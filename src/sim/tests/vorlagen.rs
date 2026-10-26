//! Tests des Simulators: Vorlagen und eigene Simulationen

use super::*;

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

#[test]
fn every_builtin_meter_passes_the_check() {
    // eine Kopie einer Vorlage muss sich speichern lassen (Zähler mit fester Struktur ohne Hersteller)
    for s in super::scenarios::builtins() {
        for m in &s.meters {
            m.check().unwrap_or_else(|e| panic!("{}: {e:#}", s.name));
        }
    }
}
