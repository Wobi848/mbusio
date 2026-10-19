use super::csv::parse_num;
use super::*;

#[test]
fn keys_and_labels() {
    assert!(is_counter("instantaneous|0|0|0|energy|kWh"));
    assert!(!is_counter("instantaneous|1|0|0|energy|kWh"));
    assert!(!is_counter("instantaneous|0|0|0|flow_temperature|°C"));
    assert_eq!(key_unit("instantaneous|0|1|0|energy|kWh"), "kWh");
    let l = key_label("instantaneous|2|1|0|energy|kWh", Lang::De);
    assert!(l.contains("Tarif 1") && l.contains("Speicher 2"), "{l}");
}

#[test]
fn plausibility() {
    let k = "instantaneous|0|0|0|energy|kWh";
    assert_eq!(check(k, 100.0, Some(99.0), 0), None);
    assert_eq!(check(k, 100.0, Some(100.0), 0), None);
    assert_eq!(check(k, 98.0, Some(99.0), 0), Some("decreased"));
    // Temperatur darf sinken
    assert_eq!(
        check(
            "instantaneous|0|0|0|flow_temperature|°C",
            20.0,
            Some(60.0),
            0
        ),
        None
    );
    // Zähler meldet Fehler im Status
    assert_eq!(check(k, 100.0, None, 0x10), Some("status"));
}

use crate::db::Meter;
use crate::mbus::master::{FrameLog, Master};
use std::sync::Arc;

const ENERGY: &str = "instantaneous|0|0|0|energy|kWh";

/// Wärmezähler 1 aus dem Simulator auslesen
fn heat_reading() -> MeterReading {
    let link = crate::link::open("SIMULATOR", 2400).unwrap();
    let mut m = Master::new(link, 2400, 60, Arc::new(FrameLog::default()));
    m.read_meter(&Target::Primary(1)).unwrap()
}

fn with_energy(r: &MeterReading, value: f64) -> MeterReading {
    let mut r = r.clone();
    for x in r.records.iter_mut().filter(|x| key_of(x) == ENERGY) {
        x.value = json!(value);
        x.value_text = value.to_string();
    }
    r
}

fn setup() -> (Db, i64, i64, MeterReading) {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    let m = db
        .add_meter(
            line,
            Meter {
                address: "1".into(),
                name: "Whg 1".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let r = heat_reading();
    db.record(line, &Target::Primary(1), Ok(&r), false).unwrap();
    (db, line, m.id, r)
}

#[test]
fn select_values_and_follow_readings() {
    let (mut db, line, mid, r) = setup();
    let c = db.dp_candidates(mid, Lang::De).unwrap();
    let values = c["values"].as_array().unwrap();
    // Hauptwert vorgeschlagen, noch nichts gewählt
    let main: Vec<_> = values.iter().filter(|v| v["main"] == true).collect();
    assert_eq!(main.len(), 1);
    assert_eq!(main[0]["key"], ENERGY);
    assert!(values.iter().all(|v| v["selected"] == false));
    assert_eq!(
        db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
            .unwrap(),
        (1, 0)
    );
    let dp = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(dp.name, "Whg 1 · Energie");
    assert_eq!(dp.no, 1);
    assert_eq!(dp.last_value, Some(123456.0));
    // neue Auslesung: Wert und Verlauf
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123460.0)),
        false,
    )
    .unwrap();
    let dp = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(dp.last_value, Some(123460.0));
    assert!(dp.invalid.is_none());
    // Zählerstand sinkt: ungültig, Meldung, letzter guter Wert bleibt
    db.record(line, &Target::Primary(1), Ok(&with_energy(&r, 5.0)), false)
        .unwrap();
    let dp = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(dp.invalid.as_deref(), Some("decreased"));
    assert_eq!(dp.last_good, Some(123460.0));
    assert!(
        db.notes.iter().any(|n| n.contains("gesunken")),
        "{:?}",
        db.notes
    );
    let m = db.meter(mid).unwrap();
    assert_eq!(quality(dp, &m, Plan::Off, crate::clock::now()), "invalid");
    // abwählen löscht, Nummer wird nicht wieder vergeben
    assert_eq!(db.dp_select(mid, &[], &[], Lang::De).unwrap(), (0, 1));
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    assert_eq!(db.dp_by_meter(mid).unwrap()[0].no, 2);
}

#[test]
fn exchange_continues_the_counter() {
    let (mut db, line, mid, r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    // neuer Zähler beginnt bei 10 kWh
    let mut new = with_energy(&r, 10.0);
    new.header.as_mut().unwrap().id = "99999999".into();
    let lines = db.exchange_meter(mid, "5", &new).unwrap();
    assert!(
        lines[0].contains("123456") && lines[0].contains("10"),
        "{lines:?}"
    );
    let dp = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(dp.last_value, Some(123456.0));
    assert!(dp.invalid.is_none());
    let m = db.meter(mid).unwrap();
    assert_eq!(m.address, "5");
    assert_eq!(m.meter_id.as_deref(), Some("99999999"));
    // weiter: neuer Zähler bei 12 kWh → 123458
    db.record(
        line,
        &Target::Primary(5),
        Ok(&with_energy(&new, 12.0)),
        false,
    )
    .unwrap();
    assert_eq!(db.dp_by_meter(mid).unwrap()[0].last_value, Some(123458.0));
    assert!(db
        .changes(1)
        .unwrap()
        .iter()
        .any(|c| c.text.contains("getauscht")));
}

#[test]
fn csv_preview_then_apply() {
    let (mut db, _line, mid, _r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let csv = db.dp_csv(1, Lang::De).unwrap();
    assert!(csv.starts_with("Nr;Linie;Zähler"));
    // in Excel geändert: Name, Faktor (mit Komma), inaktiv; dazu ein neuer Wert ohne Nummer
    let row = csv.lines().nth(1).unwrap();
    let mut c = fields(row);
    c[5] = "Wohnung 1 Energie".into();
    c[6] = "MWh".into();
    c[7] = "0,001".into();
    c[9] = "0".into();
    let mut new = c.clone();
    new[0] = String::new();
    new[4] = "instantaneous|0|0|0|volume|m³".into();
    new[5] = "Wohnung 1 Volumen".into();
    new[6] = "m³".into();
    new[7] = "1".into();
    let text = format!(
        "{}\n{}\n{}\n",
        csv.lines().next().unwrap(),
        c.join(";"),
        new.join(";")
    );
    let preview = db.dp_import(1, &table(&text), false, Lang::De).unwrap();
    assert!(preview.errors.is_empty(), "{:?}", preview.errors);
    assert_eq!(preview.changed.len(), 4);
    assert_eq!(preview.added.len(), 1);
    assert!(!preview.applied);
    assert_eq!(
        db.dp_by_meter(mid).unwrap().len(),
        1,
        "Vorschau ändert nichts"
    );
    let done = db.dp_import(1, &table(&text), true, Lang::De).unwrap();
    assert!(done.applied);
    let dps = db.dp_by_meter(mid).unwrap();
    assert_eq!(dps.len(), 2);
    assert_eq!(dps[0].name, "Wohnung 1 Energie");
    assert_eq!(dps[0].factor, 0.001);
    assert!(!dps[0].active);
    assert_eq!(dps[0].last_value, Some(123.456));
    assert_eq!(dps[1].name, "Wohnung 1 Volumen");
    // fehlerhafte Zeile: nichts wird übernommen
    let bad = db
        .dp_import(1, &table("99;x;x;x;x;x;x;1;every;1"), true, Lang::De)
        .unwrap();
    assert!(!bad.applied && bad.errors.len() == 1);
}

#[test]
fn template_applies_to_new_meters() {
    let (mut db, line, mid, r) = setup();
    let medium = db.meter(mid).unwrap().medium.unwrap();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    db.template_save(line, &medium, &[ENERGY.to_string()], true, &[], Lang::De)
        .unwrap();
    // neuer Zähler gleicher Art: nach der ersten Auslesung hat er den Datenpunkt
    let m2 = db
        .add_meter(
            line,
            Meter {
                address: "8".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let mut r2 = r.clone();
    r2.primary_address = 8;
    db.record(line, &Target::Primary(8), Ok(&r2), false)
        .unwrap();
    assert_eq!(db.dp_by_meter(m2.id).unwrap().len(), 1);
    // gelöscht bleibt gelöscht (Vorlage gilt nur einmal je Zähler)
    db.dp_select(m2.id, &[], &[], Lang::De).unwrap();
    db.record(line, &Target::Primary(8), Ok(&r2), false)
        .unwrap();
    assert!(db.dp_by_meter(m2.id).unwrap().is_empty());
}

#[test]
fn project_export_keeps_numbers_and_templates() {
    let (mut db, line, mid, _r) = setup();
    let medium = db.meter(mid).unwrap().medium.unwrap();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    db.template_save(line, &medium, &[ENERGY.to_string()], true, &[], Lang::De)
        .unwrap();
    let exported = db.export_project(1).unwrap();
    let pid = db.import_project(&exported).unwrap();
    let list = db.dp_list(None, Some(pid)).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["no"], 1);
    assert_eq!(list[0]["key"], ENERGY);
    let new_line = db
        .tree()
        .unwrap()
        .into_iter()
        .find(|p| p.project.id == pid)
        .unwrap()
        .gateways[0]
        .lines[0]
        .line
        .id;
    assert_eq!(db.templates(new_line).unwrap().len(), 1);
}

#[test]
fn mqtt_queue_follows_send_mode_and_quality() {
    let (mut db, line, mid, r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    db.mqtt_enabled = true;
    db.dp_bulk(&[id], "out:mqtt:on").unwrap();
    let topic = db.dp(id).unwrap().outputs["mqtt"].address.clone().unwrap();
    assert_eq!(topic, "schnellscan/gateway-1-linie-1/whg-1/energy");
    // eingeschaltet: aktueller Wert einmal, dann nicht mehr
    assert_eq!(db.outputs_refresh().unwrap(), 1);
    assert_eq!(db.outputs_refresh().unwrap(), 0);
    let q = |db: &Db| db.mqtt_peek(1000).unwrap();
    let first: Value = serde_json::from_str(&q(&db)[0].2).unwrap();
    assert_eq!(first["value"], 123456.0);
    assert_eq!(first["quality"], "good");
    assert_eq!(first["unit"], "kWh");
    // jede Auslesung
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123457.0)),
        false,
    )
    .unwrap();
    assert_eq!(q(&db).len(), 2);
    // nur bei Änderung: gleicher Wert nicht, anderer schon
    let mut d = db.dp(id).unwrap();
    d.send = "change".into();
    db.dp_update(id, &d).unwrap();
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123457.0)),
        false,
    )
    .unwrap();
    assert_eq!(q(&db).len(), 2);
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123458.0)),
        false,
    )
    .unwrap();
    assert_eq!(q(&db).len(), 3);
    // gesunken: geht als ungültig raus
    db.record(line, &Target::Primary(1), Ok(&with_energy(&r, 1.0)), false)
        .unwrap();
    let last: Value = serde_json::from_str(&q(&db).last().unwrap().2).unwrap();
    assert_eq!(last["quality"], "invalid");
    // Zähler umbenannt: Topic bleibt
    let mut m = db.meter(mid).unwrap();
    m.name = "Wohnung 1".into();
    db.update_meter(mid, m).unwrap();
    assert_eq!(
        db.dp(id).unwrap().outputs["mqtt"].address.as_deref(),
        Some(topic.as_str())
    );
    // MQTT aus: nichts mehr
    db.mqtt_enabled = false;
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123460.0)),
        false,
    )
    .unwrap();
    assert_eq!(q(&db).len(), 4);
    // zweiter Datenpunkt mit gleichem Vorschlag bekommt ein eindeutiges Topic
    assert!(crate::mqtt::check_topic(&topic).is_ok());
}

/// Zeile in Felder, wie es die Oberfläche tut (hier ohne Anführungszeichen)
fn fields(line: &str) -> Vec<String> {
    line.split(';').map(String::from).collect()
}

fn table(text: &str) -> Vec<Vec<String>> {
    text.lines().filter(|l| !l.is_empty()).map(fields).collect()
}

#[test]
fn csv_numbers_from_excel() {
    assert_eq!(parse_num("0,001"), Some(0.001));
    assert_eq!(parse_num("1'234.5"), Some(1234.5));
}

#[test]
fn old_export_with_mqtt_fields_is_understood() {
    let d = from_export(&json!({
        "key": ENERGY, "name": "E", "no": 4, "mqtt": true, "mqtt_topic": "a/b"
    }))
    .unwrap();
    assert_eq!(
        d.outputs["mqtt"],
        outputs::DpOutput {
            enabled: true,
            address: Some("a/b".into())
        }
    );
    let t =
        template_from_export(&json!({ "medium": "m", "keys": [ENERGY], "mqtt": true })).unwrap();
    assert_eq!(t.outputs, vec!["mqtt".to_string()]);
}

#[test]
fn outputs_in_csv_round_trip() {
    let (mut db, _line, mid, _r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &["mqtt".to_string()], Lang::De)
        .unwrap();
    let csv = db.dp_csv(1, Lang::De).unwrap();
    assert!(
        csv.lines().next().unwrap().ends_with(";MQTT;Topic"),
        "{csv}"
    );
    let row = csv.lines().nth(1).unwrap().to_string();
    assert!(
        row.contains(";1;schnellscan/gateway-1-linie-1/whg-1/energy"),
        "{row}"
    );
    // Topic in Excel geändert, MQTT aus
    let mut c = fields(&row);
    c[10] = "0".into();
    c[11] = "haus/whg1/energie".into();
    let text = format!("{}\n{}\n", csv.lines().next().unwrap(), c.join(";"));
    let p = db.dp_import(1, &table(&text), true, Lang::De).unwrap();
    assert!(p.applied && p.changed.len() == 2, "{:?}", p.changed);
    let d = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(
        d.outputs["mqtt"],
        outputs::DpOutput {
            enabled: false,
            address: Some("haus/whg1/energie".into())
        }
    );
}

#[test]
fn topic_pattern_renders_and_drops_empty_parts() {
    use crate::mqtt::{render_topic, DEFAULT_TOPIC};
    let v = |k: &'static str, s: &str| (k, s.to_string());
    let vars = [
        v("projekt", "Standard"),
        v("gateway", "Gateway 1"),
        v("linie", "Linie 1"),
        v("zaehler", "Whg 1 Wärme"),
        v("ort", ""),
        v("wert", "energy-t1"),
    ];
    assert_eq!(
        render_topic(DEFAULT_TOPIC, &vars),
        "standard/gateway-1-linie-1/whg-1-waerme/energy-t1"
    );
    assert_eq!(
        render_topic("{ort}/{zaehler}-{ort}/{wert}", &vars),
        "whg-1-waerme/energy-t1"
    );
}

#[test]
fn topics_follow_pattern_with_preview_and_list_for_integrator() {
    let (mut db, _line, mid, _r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &["mqtt".to_string()], Lang::De)
        .unwrap();
    let before = db.dp_by_meter(mid).unwrap()[0].outputs["mqtt"]
        .address
        .clone();
    assert_eq!(
        before.as_deref(),
        Some("schnellscan/gateway-1-linie-1/whg-1/energy")
    );
    // anderes Muster: Vorschau ändert nichts, Übernehmen setzt das neue Topic
    let mut c = db.mqtt_config();
    c.topic = "{linie}/{pa}/{wert}".into();
    db.set_mqtt_config(&c).unwrap();
    let p = db.dp_readdress(1, "mqtt", false).unwrap();
    assert!(!p.applied && p.changed.len() == 1, "{:?}", p.changed);
    assert_eq!(
        db.dp_by_meter(mid).unwrap()[0].outputs["mqtt"].address,
        before
    );
    assert!(db.dp_readdress(1, "mqtt", true).unwrap().applied);
    assert_eq!(
        db.dp_by_meter(mid).unwrap()[0].outputs["mqtt"]
            .address
            .as_deref(),
        Some("linie-1/1/energy")
    );
    // nochmals: nichts mehr zu ändern
    assert!(db
        .dp_readdress(1, "mqtt", false)
        .unwrap()
        .changed
        .is_empty());
    // ungültige Muster
    c.topic = "{linie}/{zaehler}".into();
    assert!(db.set_mqtt_config(&c).is_err(), "ohne {{wert}} oder {{nr}}");
    c.topic = "{linie}/{foo}/{wert}".into();
    assert!(db.set_mqtt_config(&c).is_err(), "unbekannter Platzhalter");
    // Liste für den Integrator: volles Topic mit Basis
    let list = db.output_list(1, "mqtt", Lang::De).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["address"], "mbusio/linie-1/1/energy");
    assert_eq!(list[0]["meter"], "Whg 1");
}

#[test]
fn clear_project_keeps_lines_and_removes_meters_readings_and_last_scan() {
    let (mut db, line, mid, _) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let scan = json!({ "kind": "primary", "scope": "0-250 [2400]", "complete": true, "hits": [{ "primary_address": 1 }] });
    db.set_last_scan(line, &scan, true).unwrap();
    db.add_change(line, "PA 1 → 2").unwrap();
    let project = db.active().unwrap().project.id;
    assert_eq!(db.clear_project(project).unwrap(), 1);
    assert!(db.meters(line).unwrap().is_empty());
    assert!(db.dp_by_meter(mid).unwrap().is_empty());
    assert!(db.readings(line, 10).unwrap().is_empty());
    assert!(db.last_scan(line).unwrap().is_none());
    assert!(db
        .previous_scan(line, "primary", "0-250 [2400]")
        .unwrap()
        .is_none());
    assert!(db.changes(project).unwrap().is_empty());
    // Projekt und Linie bleiben
    assert_eq!(db.active().unwrap().line.id, line);
}
