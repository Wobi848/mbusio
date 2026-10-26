//! Tests des Simulators: Primär- und Sekundärscan, Baudraten, Auffälligkeiten

use super::*;

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
