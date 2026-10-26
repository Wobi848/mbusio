//! Tests der Datenpunkte: CSV, Export und Import

use super::*;
use crate::datapoint::csv::parse_num;

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
        csv.lines()
            .next()
            .unwrap()
            .ends_with(";MQTT;Topic;Modbus;Register"),
        "{csv}"
    );
    let row = csv.lines().nth(1).unwrap().to_string();
    assert!(
        row.contains(";1;schnellscan/gateway-1-linie-1/whg-1/energy"),
        "{row}"
    );
    // Topic in Excel geändert, MQTT aus; Datei von vor Modbus (ohne die zwei Spalten)
    let mut c = fields(&row);
    c[10] = "0".into();
    c[11] = "haus/whg1/energie".into();
    c.truncate(12);
    let head = csv
        .lines()
        .next()
        .unwrap()
        .trim_end_matches(";Modbus;Register");
    let text = format!("{head}\n{}\n", c.join(";"));
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
fn duplicate_addresses_are_refused_but_csv_may_swap() {
    const VOLUME: &str = "instantaneous|0|0|0|volume|m³";
    let (mut db, _line, mid, _r) = setup();
    db.dp_select(
        mid,
        &[ENERGY.to_string(), VOLUME.to_string()],
        &[],
        Lang::De,
    )
    .unwrap();
    let dps = db.dp_by_meter(mid).unwrap();
    let (a, b) = (dps[0].id, dps[1].id);
    db.dp_bulk(&[a, b], "out:mqtt:on").unwrap();
    db.dp_bulk(&[a, b], "out:modbus:on").unwrap();
    let topic = |db: &Db, id| db.dp(id).unwrap().outputs["mqtt"].address.clone().unwrap();
    let (ta, tb) = (topic(&db, a), topic(&db, b));
    assert_ne!(ta, tb);
    // Bearbeiten: Topic des anderen abgelehnt, nichts geändert
    let mut d = db.dp(b).unwrap();
    d.outputs.get_mut("mqtt").unwrap().address = Some(ta.clone());
    d.name = "neu".into();
    let e = db.dp_update(b, &d).unwrap_err().to_string();
    assert!(e.contains("hat schon Datenpunkt 1"), "{e}");
    assert_eq!(topic(&db, b), tb);
    assert_ne!(db.dp(b).unwrap().name, "neu", "auch der Name nicht");
    // Modbus: Register im Projekt doppelt abgelehnt
    let mut d = db.dp(b).unwrap();
    d.outputs.get_mut("modbus").unwrap().address = Some("0".into());
    assert!(db.dp_update(b, &d).is_err());
    // CSV: zwei Topics tauschen geht (erst am Ende eindeutig)
    let csv = db.dp_csv(1, Lang::De).unwrap();
    let head = csv.lines().next().unwrap().to_string();
    let rows: Vec<Vec<String>> = csv.lines().skip(1).map(fields).collect();
    let swap = |rows: &[Vec<String>], x: &str, y: &str| {
        let mut out = format!("{head}\n");
        for r in rows {
            let mut r = r.clone();
            r[11] = if r[11] == ta { x.into() } else { y.into() };
            out += &(r.join(";") + "\n");
        }
        out
    };
    let p = db
        .dp_import(1, &table(&swap(&rows, &tb, &ta)), true, Lang::De)
        .unwrap();
    assert!(p.applied, "{:?}", p);
    assert_eq!((topic(&db, a), topic(&db, b)), (tb.clone(), ta.clone()));
    // CSV: beide dasselbe Topic → Fehler, nichts übernommen
    let rows: Vec<Vec<String>> = db
        .dp_csv(1, Lang::De)
        .unwrap()
        .lines()
        .skip(1)
        .map(fields)
        .collect();
    assert!(db
        .dp_import(1, &table(&swap(&rows, &ta, &ta)), true, Lang::De)
        .is_err());
    assert_eq!((topic(&db, a), topic(&db, b)), (tb, ta));
}
