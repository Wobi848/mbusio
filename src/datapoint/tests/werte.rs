//! Tests der Datenpunkte: Werte, Qualität, Zählertausch, Dauerbetrieb

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
    // Zähler meldet Fehler im Status: Messwerte ungültig, Zählerstände bleiben gültig
    assert_eq!(
        check("instantaneous|0|0|0|flow_temperature|°C", 22.4, None, 0x10),
        Some("status")
    );
    assert_eq!(check(k, 100.0, None, 0x10), None);
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
        db.notes
            .iter()
            .any(|n| n.de.contains("gesunken") && n.en.contains("decreased")),
        "{:?}",
        db.notes
    );
    let m = db.meter(mid).unwrap();
    assert_eq!(
        quality(dp, &m, Plan::Off, false, crate::clock::now()),
        "invalid"
    );
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
        lines[0].de.contains("123456") && lines[0].en.contains("10"),
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

#[test]
fn line_offline_at_a_term_marks_values_disturbed_at_once() {
    let (mut db, line, mid, _) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let project = db.active().unwrap().project.id;
    db.set_project_plan(project, "daily 00:05").unwrap();
    let q = |db: &Db| db.dp_list(Some(line), None).unwrap()[0]["quality"].clone();
    assert_eq!(q(&db), "good");
    // Wandler abgezogen: sofort gestört, nicht erst nach 3 Tagen
    db.set_plan_result(line, &json!({ "offline": true, "ok": 0, "total": 0 }))
        .unwrap();
    assert_eq!(q(&db), "disturbed");
    // wieder da
    db.set_plan_result(line, &json!({ "offline": false, "ok": 1, "total": 1 }))
        .unwrap();
    assert_eq!(q(&db), "good");
    // kein Zähler hat geantwortet (Leitung unterbrochen): ebenso
    db.set_plan_result(line, &json!({ "line_down": true, "ok": 0, "total": 2 }))
        .unwrap();
    assert_eq!(q(&db), "disturbed");
}

#[test]
fn operation_needs_connection_and_plan_and_gates_forwarding() {
    let (mut db, line, mid, r) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    mqtt_on(&mut db, true);
    db.dp_bulk(&[id], "out:mqtt:on").unwrap();
    let queued = |db: &Db| db.mqtt_peek(1000).unwrap().len();
    // ohne Anschluss und ohne Plan nicht einschaltbar
    assert!(db.set_operation(line, true).is_err());
    let l = db.line(line).unwrap();
    db.update_line(
        line,
        Line {
            connection: "SIMULATOR".into(),
            ..l
        },
    )
    .unwrap();
    assert!(db.set_operation(line, true).is_err(), "Plan fehlt");
    db.set_line_plan(line, Some("hourly")).unwrap();
    // Auslesen von Hand ohne Dauerbetrieb: Wert nachgeführt, aber nicht weitergegeben
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123470.0)),
        false,
    )
    .unwrap();
    assert_eq!(db.dp(id).unwrap().last_value, Some(123470.0));
    assert_eq!(queued(&db), 0);
    db.set_operation(line, true).unwrap();
    db.record(
        line,
        &Target::Primary(1),
        Ok(&with_energy(&r, 123480.0)),
        false,
    )
    .unwrap();
    assert_eq!(queued(&db), 1);
    db.set_operation(line, false).unwrap();
    assert!(!db.line(line).unwrap().operation);
}

#[test]
fn error_value_from_meter_is_invalid_at_once() {
    let (mut db, line, mid, r) = setup();
    let power = r
        .records
        .iter()
        .find(|x| x.function == "instantaneous" && x.quantity == "power")
        .map(key_of)
        .expect("Leistung im Simulator");
    db.dp_select(mid, std::slice::from_ref(&power), &[], Lang::De)
        .unwrap();
    db.record(line, &Target::Primary(1), Ok(&r), false).unwrap();
    assert_eq!(db.dp_by_meter(mid).unwrap()[0].invalid, None);
    // Zähler ohne Durchfluss: Leistung als Fehlerwert gemeldet (wie Itron auf dem Tisch)
    let mut e = r.clone();
    for x in e.records.iter_mut().filter(|x| key_of(x) == power) {
        x.function = "error_value".into();
        x.value = json!(99999900.0);
    }
    db.record(line, &Target::Primary(1), Ok(&e), false).unwrap();
    let dp = &db.dp_by_meter(mid).unwrap()[0];
    assert_eq!(dp.invalid.as_deref(), Some("error_value"));
    assert_eq!(
        quality(
            dp,
            &db.meter(mid).unwrap(),
            Plan::Off,
            false,
            crate::clock::now()
        ),
        "invalid"
    );
}

#[test]
fn meter_clock_offset_is_noted_from_five_minutes() {
    let (mut db, line, mid, r) = setup();
    let at = |r: &MeterReading, clock: &str| {
        let mut r = r.clone();
        r.timestamp = "2026-10-20 19:55:38".into();
        r.time_unsure = false;
        let mut d = r.records[0].clone();
        d.quantity = "date_time".into();
        d.function = "instantaneous".into();
        d.storage = 0;
        d.value_text = clock.into();
        r.records.retain(|x| x.quantity != "date_time");
        r.records.push(d);
        r
    };
    let off = |db: &Db| db.meter(mid).unwrap().clock_off;
    // Itron auf dem Tisch: Uhr gut eine Stunde vor (20:59 gegen 19:55:38)
    db.record(
        line,
        &Target::Primary(1),
        Ok(&at(&r, "2026-10-20 20:59")),
        false,
    )
    .unwrap();
    assert_eq!(off(&db), Some(63));
    // innerhalb von 5 min: kein Hinweis
    db.record(
        line,
        &Target::Primary(1),
        Ok(&at(&r, "2026-10-20 19:58")),
        false,
    )
    .unwrap();
    assert_eq!(off(&db), None);
    // eigene Uhr unsicher: nichts behaupten
    let mut u = at(&r, "2026-10-20 18:00");
    u.time_unsure = true;
    db.record(line, &Target::Primary(1), Ok(&u), false).unwrap();
    assert_eq!(off(&db), None);
}

/// Speicher-Vorschau: Zuwachs je Tag nur aus Linien im Dauerbetrieb, je Termin Auslesung und Werte
#[test]
fn growth_per_day_follows_plan_and_operation() {
    let (mut db, line, mid, _) = setup();
    db.dp_select(mid, &[ENERGY.to_string()], &[], Lang::De)
        .unwrap();
    db.set_line_plan(line, Some("hourly")).unwrap();
    assert_eq!(
        db.growth_per_day().unwrap(),
        0,
        "ohne Dauerbetrieb liest niemand"
    );
    db.operation_on(line);
    assert_eq!(db.growth_per_day().unwrap(), 24 * (370 + 70));
    // Zähler aus dem Plan: nichts
    db.set_meter_plan(mid, Some("off")).unwrap();
    assert_eq!(db.growth_per_day().unwrap(), 0);
}
