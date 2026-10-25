use super::*;

/// Status als &'static str für die Vergleiche in den Tests
fn stat(s: &str) -> &'static str {
    match s {
        "ok" => "ok",
        "collision" => "collision",
        _ => "no_data",
    }
}
use crate::mbus::master::{Bauds, FrameLog, Master, MeterReading, Progress, Target};
use std::sync::Arc;

fn master() -> Master {
    Master::new(
        Box::new(Simulator::new(9600)),
        9600,
        50,
        Arc::new(FrameLog::default()),
    )
}

#[test]
fn primary_scan_finds_meters_and_collision() {
    let mut m = master();
    let mut hits = Vec::new();
    m.scan_primary(0, 5, 0, &mut |p| {
        if let Progress::Hit(h) = p {
            hits.push(h);
        }
        true
    })
    .unwrap();
    let addrs: Vec<_> = hits
        .iter()
        .map(|h| (h.primary_address.unwrap(), stat(&h.status)))
        .collect();
    assert_eq!(
        addrs,
        vec![
            (0, "collision"),
            (1, "ok"),
            (2, "ok"),
            (3, "ok"),
            (4, "no_data"),
            (5, "no_data")
        ]
    );
    assert_eq!(hits[1].id.as_deref(), Some("12345678"));
    assert_eq!(hits[1].manufacturer.as_deref(), Some("KAM"));
}

#[test]
fn secondary_scan_resolves_all() {
    let mut m = master();
    let mut hits = Vec::new();
    m.scan_secondary(&mut |p| {
        if let Progress::Hit(h) = p {
            hits.push((h.id.clone().unwrap_or_default(), stat(&h.status)));
        }
        true
    })
    .unwrap();
    hits.sort();
    assert_eq!(
        hits,
        vec![
            ("10000010".into(), "ok"),
            ("10000011".into(), "ok"),
            // meldet "beschäftigt": gefunden, ID aus der Suche, aber keine Daten
            ("11000011".into(), "no_data"),
            ("12000012".into(), "ok"),
            ("12345678".into(), "ok"),
            ("12345699".into(), "ok"),
            // Kontingent: die ersten Auslesungen klappen noch
            ("19000019".into(), "ok"),
            ("30000001".into(), "ok"),
            // fehlerhafte Telegramme: gefunden, aber ohne Daten
            ("40000004".into(), "no_data"),
            ("50000005".into(), "no_data"),
            ("55500011".into(), "ok"),
            ("70000007".into(), "ok"),
            // gleiche ID bei zwei Zählern: über die ID allein nicht trennbar
            ("77777777".into(), "collision"),
            ("87654321".into(), "ok"),
            ("88888888".into(), "collision"),
            // 60000006 und 90000009 laufen mit anderer Baudrate, 16000016 (Wackelkontakt)
            // verpasst die Selektion: alle drei fehlen
        ]
    );
}

#[test]
fn read_multi_telegram() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    assert_eq!(r.raw_frames.len(), 2);
    assert!(r.records.iter().any(|x| x.storage == 2 && x.unit == "kWh"));
    assert!(r
        .records
        .iter()
        .any(|x| x.quantity == "flow_temperature" && x.value_text == "65.23"));
}

#[test]
fn raw_telegrams_round_trip() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    let back = MeterReading::from_raw(
        &r.raw_bytes(),
        r.timestamp.clone(),
        r.utc,
        r.target.clone(),
        false,
    )
    .unwrap();
    assert_eq!(back.raw_frames, r.raw_frames);
    assert_eq!(back.primary_address, r.primary_address);
    assert_eq!(
        serde_json::to_value(&back.records).unwrap(),
        serde_json::to_value(&r.records).unwrap()
    );
    assert_eq!(
        back.header.unwrap().secondary_address,
        r.header.unwrap().secondary_address
    );
}

#[test]
fn read_electricity_values() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(2)).unwrap();
    let find = |d: &str| {
        r.records
            .iter()
            .find(|x| x.quantity == d)
            .unwrap()
            .value_text
            .clone()
    };
    assert_eq!(find("voltage"), "230.1");
    assert_eq!(find("current"), "5.123");
    assert_eq!(find("firmware_version"), "1.02");
    assert!(r.records.iter().any(|x| x.tariff == 2));
}

#[test]
fn corrupt_telegrams_are_rejected() {
    let mut m = master();
    let e = m.read_meter(&Target::Primary(4)).unwrap_err().to_string();
    assert!(e.contains("Kollision"), "{e}");
    assert!(m.read_meter(&Target::Primary(5)).is_err());
}

#[test]
fn set_address_via_secondary() {
    let mut m = master();
    m.set_primary(&Target::parse("12345699").unwrap(), 20)
        .unwrap();
    let r = m.read_meter(&Target::Primary(20)).unwrap();
    assert_eq!(r.header.unwrap().id, "12345699");
    // Adresse 0 ist jetzt eindeutig
    let r = m.read_meter(&Target::Primary(0)).unwrap();
    assert_eq!(r.header.unwrap().id, "55500011");
}

#[test]
fn set_address_refuses_collision() {
    let mut m = master();
    let e = m
        .set_primary(&Target::Primary(0), 9)
        .unwrap_err()
        .to_string();
    assert!(e.contains("Mehrere Zähler"), "{e}");
    // keiner der beiden wurde umgestellt
    assert!(m.read_meter(&Target::Primary(9)).is_err());
    let r = m.read_meter(&Target::parse("55500011").unwrap()).unwrap();
    assert_eq!(r.primary_address, 0);
}

#[test]
fn set_address_refuses_occupied() {
    let mut m = master();
    let e = m
        .set_primary(&Target::Primary(1), 2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("schon belegt"), "{e}");
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    assert_eq!(r.header.unwrap().id, "12345678");
    let r = m.read_meter(&Target::Primary(2)).unwrap();
    assert_eq!(r.header.unwrap().id, "87654321");
}

#[test]
fn set_address_to_own_address_is_ok() {
    let mut m = master();
    m.set_primary(&Target::Primary(1), 1).unwrap();
    m.set_primary(&Target::parse("87654321").unwrap(), 2)
        .unwrap();
}

#[test]
fn detect_and_switch_baud() {
    let mut m = master();
    // Adresse 6 läuft mit 2400 Bd, der Master mit 9600
    assert!(m.read_meter(&Target::Primary(6)).is_err());
    assert_eq!(m.detect_baud(&Target::Primary(6)).unwrap(), vec![2400]);
    assert_eq!(m.baud(), 9600);
    m.set_baud(2400).unwrap();
    m.switch_baud(&Target::parse("60000006").unwrap(), 9600)
        .unwrap();
    m.set_baud(9600).unwrap();
    let r = m.read_meter(&Target::Primary(6)).unwrap();
    assert_eq!(r.header.unwrap().id, "60000006");
    // Zähler 1 läuft weiter mit 9600
    assert_eq!(m.detect_baud(&Target::Primary(1)).unwrap(), vec![9600]);
}

#[test]
fn switch_baud_refuses_collision() {
    let mut m = master();
    assert!(m.switch_baud(&Target::Primary(0), 2400).is_err());
    assert_eq!(m.detect_baud(&Target::Primary(0)).unwrap(), vec![9600]);
}

#[test]
fn application_reset_is_acknowledged() {
    let mut m = master();
    m.application_reset(&Target::Primary(1), None).unwrap();
    m.application_reset(&Target::parse("87654321").unwrap(), Some(0x10))
        .unwrap();
    assert!(m.application_reset(&Target::Primary(99), None).is_err());
}

#[test]
fn read_fixed_structure() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(7)).unwrap();
    let h = r.header.unwrap();
    assert_eq!(
        (h.id.as_str(), h.medium.as_str()),
        ("70000007", "medium_heat_outlet")
    );
    assert_eq!(r.records.len(), 2);
    assert_eq!(
        (r.records[0].quantity.as_str(), r.records[0].unit.as_str()),
        ("energy", "kWh")
    );
    assert!(r.records[0].value_text.starts_with("123"));
    assert_eq!(
        (r.records[1].unit.as_str(), r.records[1].value_text.as_str()),
        ("m³", "678")
    );
}

#[test]
fn raw_send_returns_reply() {
    let mut m = master();
    let ack = m.raw_send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
    assert!(matches!(
        ack,
        crate::mbus::master::Reply::Frame(crate::mbus::frame::Frame::Ack)
    ));
    let none = m.raw_send(&[0x10, 0x40, 0x63, 0xA3, 0x16]).unwrap();
    assert!(matches!(none, crate::mbus::master::Reply::None));
}

#[test]
fn bus_bauds_finds_mixed_bus() {
    let mut m = master();
    // 300: uralter Zähler auf Adresse 9
    assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 2400, 9600]);
    // einzeln abfragen wie die Oberfläche
    assert_eq!(m.bus_bauds(&[2400]).unwrap(), vec![2400]);
    assert_eq!(m.bus_bauds(&[4800]).unwrap(), Vec::<u32>::new());
    assert!(m.bus_bauds(&[1234]).is_err());
    assert_eq!(m.baud(), 9600);
}

#[test]
fn primary_scan_on_all_bauds() {
    let mut m = master();
    let mut hits = Vec::new();
    m.scan_with(
        Bauds::Probe,
        &mut |p| {
            if let Progress::Hit(h) = p {
                hits.push((h.primary_address.unwrap(), h.baud.unwrap(), stat(&h.status)));
            }
            true
        },
        &mut |m, p| m.scan_primary(5, 7, 0, p),
    )
    .unwrap();
    assert_eq!(
        hits,
        // eingestellte Baudrate (9600) zuerst
        vec![(5, 9600, "no_data"), (7, 9600, "ok"), (6, 2400, "ok")]
    );
    assert_eq!(m.baud(), 9600);
    // nur eingestellte Baudrate: Zähler 6 bleibt unsichtbar, Treffer tragen trotzdem die Baudrate
    let mut bauds = Vec::new();
    m.scan_with(
        Bauds::Current,
        &mut |p| {
            if let Progress::Hit(h) = p {
                bauds.push((h.primary_address.unwrap(), h.baud.unwrap()));
            }
            true
        },
        &mut |m, p| m.scan_primary(5, 7, 0, p),
    )
    .unwrap();
    assert_eq!(bauds, vec![(5, 9600), (7, 9600)]);
    // gewählte Liste ohne Prüfung, eingestellte Baudrate zuerst
    let mut order = Vec::new();
    m.scan_with(
        Bauds::List(vec![2400, 9600, 2400]),
        &mut |p| {
            if let Progress::Hit(h) = p {
                order.push((h.primary_address.unwrap(), h.baud.unwrap()));
            }
            true
        },
        &mut |m, p| m.scan_primary(6, 6, 0, p),
    )
    .unwrap();
    assert_eq!(order, vec![(6, 2400)]);
    assert!(m
        .scan_with(Bauds::List(vec![1234]), &mut |_| true, &mut |m, p| m
            .scan_primary(6, 6, 0, p))
        .is_err());
}

#[test]
fn free_primary_skips_meters_without_data() {
    let mut m = master();
    // 1..5 und 7 belegt (4 und 5 liefern keine Daten, antworten aber), 6 läuft mit anderer Baudrate
    assert_eq!(m.free_primary(1, &[]).unwrap(), 6);
    assert_eq!(m.free_primary(1, &[6]).unwrap(), 8);
}

#[test]
fn switch_meter_on_other_baud_to_bus_baud() {
    let mut m = master();
    // Zähler 6 läuft mit 2400, Bus mit 9600: Befehl mit 2400 senden, Kontrolle mit 9600
    m.set_baud(2400).unwrap();
    m.switch_baud(&Target::Primary(6), 9600).unwrap();
    m.set_baud(9600).unwrap();
    assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 9600]);
}

#[test]
fn faulty_meters_are_reported() {
    let mut m = master();
    let e = m.read_meter(&Target::Primary(11)).unwrap_err().to_string();
    assert!(e.contains("beschäftigt") && !e.contains("Decodier"), "{e}");
    let mut st = Vec::new();
    m.scan_primary(11, 11, 0, &mut |p| {
        if let Progress::Hit(h) = p {
            st.push(stat(&h.status));
        }
        true
    })
    .unwrap();
    assert_eq!(st, vec!["no_data"]);
    let e = m.read_meter(&Target::Primary(12)).unwrap_err().to_string();
    assert!(e.contains("13"), "{e}");
    // zwei Zähler auf Adresse 10: Kollision. Nicht über read_meter prüfen: überlagerte
    // Telegramme ergeben je nach Zählerstand zufällig ein gültiges (wie am echten Bus)
    let mut st = Vec::new();
    m.scan_primary(10, 10, 0, &mut |p| {
        if let Progress::Hit(h) = p {
            st.push(stat(&h.status));
        }
        true
    })
    .unwrap();
    assert_eq!(st, vec!["collision"]);
    let e = m.read_meter(&Target::Primary(10)).unwrap_err().to_string();
    assert!(e.contains("Mehrere Zähler"), "{e}");
    // über die ID geht es
    let r = m.read_meter(&Target::parse("10000011").unwrap()).unwrap();
    assert_eq!(r.header.unwrap().id, "10000011");
    // gleiche ID, verschiedene Hersteller: nur ID ist mehrdeutig, volle Adresse eindeutig
    let e = m
        .read_meter(&Target::parse("77777777").unwrap())
        .unwrap_err()
        .to_string();
    assert!(e.contains("16-stellig"), "{e}");
    let r = m
        .read_meter(&Target::parse("777777776A490107").unwrap())
        .unwrap();
    assert_eq!(r.header.unwrap().manufacturer, "ZRI");
    // Wackelkontakt: ohne Wiederholung übersehen, mit Wiederholung gefunden
    let mut found = |retries| {
        let mut n = 0;
        m.scan_primary(16, 16, retries, &mut |p| {
            n += matches!(p, Progress::Hit(_)) as u32;
            true
        })
        .unwrap();
        n
    };
    assert_eq!(found(0), 0);
    assert_eq!(found(1), 1);
}

#[test]
fn scan_explains_why_there_is_no_data() {
    let mut m = master();
    let mut why = Vec::new();
    m.scan_primary(0, 12, 0, &mut |p| {
        if let Progress::Hit(h) = p {
            if h.status != "ok" {
                why.push((
                    h.primary_address.unwrap(),
                    h.reason.unwrap_or_default(),
                    h.raw.is_some(),
                ));
            }
        }
        true
    })
    .unwrap();
    assert_eq!(
        why,
        vec![
            (0, "collision".into(), true),
            (4, "checksum".into(), true),
            (5, "truncated".into(), true),
            (10, "collision".into(), true),
            (11, "app_error:08".into(), true),
            (12, "wrong_address:13".into(), true),
        ]
    );
}

fn master_for(key: &str, baud: u32) -> Master {
    Master::new(
        Box::new(super::open(key, baud).unwrap()),
        baud,
        50,
        Arc::new(FrameLog::default()),
    )
}

#[test]
fn template_all_ok_has_ten_clean_meters() {
    let mut m = master_for("ok10", 2400);
    let mut hits = Vec::new();
    m.scan_primary(0, 15, 0, &mut |p| {
        if let Progress::Hit(h) = p {
            hits.push((h.primary_address, stat(&h.status)));
        }
        true
    })
    .unwrap();
    assert_eq!(hits.len(), 10);
    assert!(hits.iter().all(|(_, s)| *s == "ok"), "{hits:?}");
    // Werte je Zähler verschieden
    let a = m.read_meter(&Target::Primary(1)).unwrap();
    let b = m.read_meter(&Target::Primary(2)).unwrap();
    assert_ne!(a.records[0].value, b.records[0].value);
}

#[test]
fn template_apartment_block_secondary_scan_finds_all() {
    let mut m = master_for("mfh30", 2400);
    let mut n = 0;
    m.scan_secondary(&mut |p| {
        if let Progress::Hit(h) = p {
            assert_eq!(h.status, "ok");
            n += 1;
        }
        true
    })
    .unwrap();
    assert_eq!(n, 30);
}

#[test]
fn template_bauds_answers_on_three_rates() {
    let mut m = master_for("bauds", 2400);
    assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 2400, 9600]);
}

#[test]
fn unknown_simulation_and_bad_meters_are_refused() {
    assert!(super::open("gibtsnicht", 2400).is_err());
    let ok = SimMeter::new(1, "12345678", "KAM", "heat");
    assert!(ok.check().is_ok());
    for bad in [
        SimMeter {
            id: "1234".into(),
            ..ok.clone()
        },
        SimMeter {
            manufacturer: "ka".into(),
            ..ok.clone()
        },
        SimMeter {
            primary: 251,
            ..ok.clone()
        },
        SimMeter {
            medium: "lava".into(),
            ..ok.clone()
        },
        SimMeter {
            baud: Some(1234),
            ..ok.clone()
        },
        SimMeter {
            fault: "kaputt".into(),
            ..ok.clone()
        },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
    }
}

#[test]
fn own_simulation_is_stored_and_used() {
    let dir = std::env::temp_dir().join(format!("mbusio-sim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let mut db = crate::db::Db::open(&dir).unwrap();
        let key = db.sim_create("Test Anlage", Some("ok10")).unwrap();
        assert_eq!(key, "test-anlage");
        let mut meters = super::all()
            .into_iter()
            .find(|s| s.key == key)
            .unwrap()
            .meters;
        meters.truncate(3);
        meters[2].fault = "busy".into();
        db.sim_save(&key, "Test Anlage", &meters).unwrap();
        // Vorlagen bleiben unverändert
        assert!(db.sim_save("ok10", "x", &meters).is_err());
    }
    // nach einem Neustart wieder da
    crate::sim::set_user(Vec::new());
    let db = crate::db::Db::open(&dir).unwrap();
    assert_eq!(db.sim_list().unwrap()[0].meters.len(), 3);
    let mut m = master_for("test-anlage", 2400);
    assert!(m.read_meter(&Target::Primary(1)).is_ok());
    assert!(format!("{:#}", m.read_meter(&Target::Primary(3)).unwrap_err()).contains("08"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// IDs, die ein Sekundärscan findet (sortiert); `stop_after` = nach so vielen Masken abbrechen
fn secondary_ids(
    m: &mut Master,
    prefix: &str,
    resume: Option<&str>,
    stop_after: Option<usize>,
) -> (Vec<String>, Option<String>) {
    let (mut ids, mut stopped, mut steps) = (Vec::new(), None, 0);
    m.scan_secondary_from(prefix, resume, &mut |p| match p {
        Progress::Hit(h) => {
            ids.push(h.id.unwrap_or_default());
            true
        }
        Progress::Stopped { mask, .. } => {
            stopped = Some(mask);
            false
        }
        Progress::Step { .. } => {
            steps += 1;
            stop_after.is_none_or(|n| steps <= n)
        }
    })
    .unwrap();
    ids.sort();
    (ids, stopped)
}

#[test]
fn secondary_scan_with_prefix_finds_only_those_ids() {
    let mut m = master_for("mfh30", 2400);
    let (all, _) = secondary_ids(&mut m, "", None, None);
    // Präfix aus einer gefundenen ID: genau die IDs mit diesem Anfang
    let prefix = &all[0][..3];
    let (some, _) = secondary_ids(&mut m, prefix, None, None);
    let expected: Vec<String> = all
        .iter()
        .filter(|id| id.starts_with(prefix))
        .cloned()
        .collect();
    assert!(
        !some.is_empty() && some.len() < all.len(),
        "{prefix}: {some:?} von {all:?}"
    );
    assert_eq!(some, expected);
    assert!(
        m.scan_secondary_from("12a", None, &mut |_| true).is_err(),
        "nur Ziffern"
    );
}

#[test]
fn secondary_scan_resumes_where_it_stopped() {
    let mut m = master_for("mfh30", 2400);
    let (all, _) = secondary_ids(&mut m, "", None, None);
    let (first, stopped) = secondary_ids(&mut m, "", None, Some(15));
    let mask = stopped.expect("Abbruch meldet die Stelle");
    let (rest, _) = secondary_ids(&mut m, "", Some(&mask), None);
    let mut both = [first, rest].concat();
    both.sort();
    assert_eq!(both, all, "zusammen alle, keiner doppelt");
}

#[test]
fn counters_keep_rising_after_a_restart() {
    // Energie des Wärmezählers an Adresse 1, gelesen von einem frischen Simulator (= Neustart)
    let energy = || {
        let mut m = Master::new(
            Box::new(Simulator::new(9600).with_counter_origin()),
            9600,
            50,
            Arc::new(FrameLog::default()),
        );
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        r.records
            .iter()
            .find(|x| x.quantity == "energy" && x.storage == 0)
            .and_then(|x| x.value.as_f64())
            .unwrap()
    };
    let before = energy();
    // ab einem festen Datum: deutlich über dem Anfangswert, und nach dem „Neustart“ nicht kleiner
    assert!(before > 123_456.0, "{before}");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(energy() >= before);
}
