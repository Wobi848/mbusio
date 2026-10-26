//! Tests der Datenpunkte: Ausgänge: MQTT, Topics, Home Assistant, Live-Werte

use super::*;

/// Home Assistant ein- oder ausschalten (mit fester Client-ID für feste Topics)
fn ha_on(db: &mut Db, on: bool) {
    let mut c = db.mqtt_config();
    c.ha = on;
    c.client_id = "test".into();
    db.set_mqtt_config(&c).unwrap();
}

#[test]
fn mqtt_queue_follows_send_mode_and_quality() {
    let (mut db, line, mid, r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    mqtt_on(&mut db, true);
    db.dp_bulk(&[id], "out:mqtt:on").unwrap();
    let topic = db.dp(id).unwrap().outputs["mqtt"].address.clone().unwrap();
    assert_eq!(topic, "schnellscan/gateway-1-linie-1/whg-1/energy");
    // Linie nicht im Dauerbetrieb: nichts geht weiter (Konzept Bedienung §13)
    assert_eq!(db.outputs_refresh().unwrap(), 0);
    db.operation_on(line);
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
    mqtt_on(&mut db, false);
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
fn home_assistant_config_follows_datapoints() {
    let (mut db, _line, mid, _r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    mqtt_on(&mut db, true);
    ha_on(&mut db, true);
    db.dp_bulk(&[id], "out:mqtt:on").unwrap();
    let topic = "homeassistant/sensor/test/p1-dp1/config";
    // Konfig-Nachrichten: vollständiges Topic, nicht unter dem Basis-Topic
    let ha = |db: &Db| -> Vec<(String, String)> {
        db.mqtt_peek(1000)
            .unwrap()
            .into_iter()
            .filter(|q| q.3)
            .map(|q| (q.1, q.2))
            .collect()
    };
    db.outputs_refresh().unwrap();
    let sent = ha(&db);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, topic);
    let c: Value = serde_json::from_str(&sent[0].1).unwrap();
    assert_eq!(c["unique_id"], "test-p1-dp1");
    assert_eq!(
        c["name"], "Energie",
        "ohne den Zähler, den Home Assistant davor setzt"
    );
    assert_eq!(
        c["state_topic"],
        "mbusio/schnellscan/gateway-1-linie-1/whg-1/energy"
    );
    assert_eq!(c["unit_of_measurement"], "kWh");
    assert_eq!(c["device_class"], "energy");
    assert_eq!(c["state_class"], "total_increasing");
    assert_eq!(c["availability"][0]["topic"], "mbusio/status");
    assert_eq!(c["device"]["identifiers"][0], format!("test-m{mid}"));
    // unverändert: nichts Neues
    db.outputs_refresh().unwrap();
    assert_eq!(ha(&db).len(), 1);
    // Name geändert: Konfig neu
    let mut d = db.dp(id).unwrap();
    d.name = "Wohnung 1 · Energie".into();
    db.dp_update(id, &d).unwrap();
    db.outputs_refresh().unwrap();
    let sent = ha(&db);
    assert_eq!(sent.len(), 2);
    assert!(sent[1].1.contains("Wohnung 1"));
    // MQTT-Häkchen weg: leere Nachricht, Home Assistant entfernt den Sensor
    db.dp_bulk(&[id], "out:mqtt:off").unwrap();
    db.outputs_refresh().unwrap();
    let sent = ha(&db);
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[2], (topic.to_string(), String::new()));
    // wieder an, dann Home Assistant aus: ebenfalls entfernt
    db.dp_bulk(&[id], "out:mqtt:on").unwrap();
    db.outputs_refresh().unwrap();
    ha_on(&mut db, false);
    db.outputs_refresh().unwrap();
    let sent = ha(&db);
    assert_eq!(sent.len(), 5);
    assert_eq!(sent[4], (topic.to_string(), String::new()));
    // ungültiges Präfix wird abgelehnt
    let mut c = db.mqtt_config();
    c.ha = true;
    c.ha_prefix = "/ha".into();
    assert!(db.set_mqtt_config(&c).is_err());
}

/// Auslesung mit Wert, `minutes` vor der Auslesung `r` (so bleibt der Wert aktuell, nicht veraltet)
fn energy_ago(r: &MeterReading, value: f64, minutes: i64) -> MeterReading {
    let mut r = with_energy(r, value);
    r.utc = r.utc.map(|t| t - minutes * 60);
    r
}

#[test]
fn live_values_follow_operation_send_mode_and_quality() {
    let (mut db, line, mid, r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let scope = Scope {
        project: 1,
        line: None,
        meter: None,
    };
    let changes = db.changes.subscribe();
    let read = |db: &mut Db, value, minutes| {
        db.record(
            line,
            &Target::Primary(1),
            Ok(&energy_ago(&r, value, minutes)),
            false,
        )
        .unwrap()
    };
    // jede Auslesung meldet sich bei den Empfängern
    read(&mut db, 200100.0, 50);
    assert!(changes.has_changed().unwrap());
    // ohne Dauerbetrieb: im Aufbau, aber keine Werte (wie bei den Ausgängen)
    let empty = LiveState::default();
    let s0 = db.live_state(scope).unwrap();
    assert!(s0.values.is_empty());
    assert!(s0.since(&empty).0, "Aufbau neu");
    let l = db.line(line).unwrap();
    db.update_line(
        line,
        Line {
            connection: "SIMULATOR".into(),
            ..l
        },
    )
    .unwrap();
    db.set_line_plan(line, Some("hourly")).unwrap();
    db.set_operation(line, true).unwrap();
    read(&mut db, 200101.0, 40);
    let s1 = db.live_state(scope).unwrap();
    let (changed, values) = s1.since(&s0);
    assert!(!changed);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, 200101.0);
    assert_eq!(values[0].quality, "good");
    assert!(values[0].time.ends_with('Z'), "UTC: {}", values[0].time);
    // nichts Neues: nichts
    assert!(db.live_state(scope).unwrap().since(&s1).1.is_empty());
    // jede Auslesung: gleicher Wert zu neuer Zeit geht raus
    read(&mut db, 200101.0, 30);
    let s2 = db.live_state(scope).unwrap();
    assert_eq!(s2.since(&s1).1.len(), 1);
    // nur bei Änderung: gleicher Wert nicht, anderer schon
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    let mut d = db.dp(id).unwrap();
    d.send = "change".into();
    db.dp_update(id, &d).unwrap();
    let s2 = db.live_state(scope).unwrap();
    read(&mut db, 200101.0, 20);
    let s3 = db.live_state(scope).unwrap();
    assert!(s3.since(&s2).1.is_empty());
    read(&mut db, 200102.0, 10);
    assert_eq!(db.live_state(scope).unwrap().since(&s3).1.len(), 1);
    // Name geändert: Aufbau geändert
    let s4 = db.live_state(scope).unwrap();
    let mut d = db.dp(id).unwrap();
    d.name = "Wohnung 1 · Energie".into();
    db.dp_update(id, &d).unwrap();
    assert!(db.live_state(scope).unwrap().since(&s4).0);
    // Baum zur Auswahl: Projekt › Gateway › Linie › Zähler › Datenpunkt mit Wert
    let tree = db.live_tree().unwrap();
    let line0 = &tree[0]["gateways"][0]["lines"][0];
    assert_eq!(line0["operation"], true);
    let dp = &line0["meters"][0]["datapoints"][0];
    assert_eq!(dp["no"], 1);
    assert_eq!(dp["name"], "Wohnung 1 · Energie");
    assert_eq!(dp["value"], 200102.0);
    // unbekanntes Projekt: Fehler
    assert!(db
        .live_state(Scope {
            project: 99,
            line: None,
            meter: None
        })
        .is_err());
}
