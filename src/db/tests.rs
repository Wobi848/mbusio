//! Tests der Datenbank

use super::*;

fn meter(address: &str) -> Meter {
    Meter {
        address: address.into(),
        ..Default::default()
    }
}

#[test]
fn update_keeps_gateway_lines_with_plan_in_operation() {
    let setup = || {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap();
        db.update_line(
            line.id,
            Line {
                connection: "COM3".into(),
                ..line.clone()
            },
        )
        .unwrap();
        db.add_meter(line.id, meter("5")).unwrap();
        db.set_project_plan(db.active().unwrap().project.id, "daily 00:05")
            .unwrap();
        // Datenbank von vor dem Schalter
        db.conn
            .execute_batch("ALTER TABLE lines DROP COLUMN operation")
            .unwrap();
        (db, line.id)
    };
    // Laptop (Fenster): alles aus
    let (mut db, id) = setup();
    crate::service::set_background_for_tests(false);
    db.migrate().unwrap();
    assert!(!db.line(id).unwrap().operation);
    // Gateway (Dienst): Linie mit wirksamem Plan liest weiter
    let (mut db, id) = setup();
    crate::service::set_background_for_tests(true);
    let r = db.migrate();
    crate::service::set_background_for_tests(false);
    r.unwrap();
    assert!(db.line(id).unwrap().operation);
}

#[test]
fn meter_list_follows_address_and_baud_changes() {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    let by_pa = db.add_meter(line, meter("5")).unwrap();
    let by_sa = db.add_meter(line, meter("12345678")).unwrap();
    // über die PA gemerkt: neue Adresse
    assert!(db
        .follow_meter_change(line, &Target::Primary(5), Some(17), None)
        .unwrap());
    let m = db.meter(by_pa.id).unwrap();
    assert_eq!((m.address.as_str(), m.primary_address), ("17", Some(17)));
    // über die ID gemerkt: Adresse bleibt, PA wird nachgetragen
    db.follow_meter_change(
        line,
        &Target::Secondary("12345678FFFFFFFF".into()),
        Some(9),
        None,
    )
    .unwrap();
    let m = db.meter(by_sa.id).unwrap();
    assert_eq!(
        (m.address.as_str(), m.primary_address),
        ("12345678", Some(9))
    );
    // Baudrate: abweichend gemerkt, gleich wie die Linie wieder ohne eigene
    db.follow_meter_change(line, &Target::Primary(17), None, Some(9600))
        .unwrap();
    assert_eq!(db.meter(by_pa.id).unwrap().baud, Some(9600));
    db.follow_meter_change(line, &Target::Primary(17), None, Some(2400))
        .unwrap();
    assert_eq!(db.meter(by_pa.id).unwrap().baud, None);
    // nicht in der Liste: nichts zu tun
    assert!(!db
        .follow_meter_change(line, &Target::Primary(99), Some(1), None)
        .unwrap());
}

#[test]
fn starts_with_default_project_and_active_line() {
    let db = Db::memory().unwrap();
    let a = db.active().unwrap();
    assert_eq!(a.gateway.name, "Gateway 1");
    assert_eq!(a.line.baud, 2400);
    assert_eq!(a.line.addressing, "primary", "neue Linien merken über PA");
    assert_eq!(db.tree().unwrap().len(), 1);
}

#[test]
fn complete_scans_are_remembered_per_kind_and_scope() {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    let all = "0-250 [2400]";
    let scan = |pa: u8| json!({ "kind": "primary", "scope": all, "complete": true, "hits": [{ "primary_address": pa }] });
    let prev = |db: &Db, scope: &str| db.previous_scan(line, "primary", scope).unwrap();
    assert!(
        prev(&db, all).is_none(),
        "erster Scan: nichts zu vergleichen"
    );
    db.set_last_scan(line, &scan(1), true).unwrap();
    // abgebrochener Scan wird nicht gemerkt
    db.set_last_scan(line, &scan(9), false).unwrap();
    assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 1);
    assert!(
        prev(&db, "0-12 [2400]").is_none(),
        "anderer Bereich: kein Vergleich"
    );
    // „PA ändern“ führt den gemerkten Scan nach: keine Änderung beim nächsten Vergleich
    db.replace_last_scan(line, &scan(5)).unwrap();
    assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 5);
    // nach einem abgebrochenen Scan verkürzt „PA ändern“ den gemerkten Scan nicht
    let mut cut = scan(7);
    cut["complete"] = json!(false);
    cut["hits"] = json!([]);
    db.replace_last_scan(line, &cut).unwrap();
    assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 5);
}

#[test]
fn meters_are_per_line_and_reject_duplicates() {
    let mut db = Db::memory().unwrap();
    let l1 = db.active_line().unwrap().id;
    let gw = db.active().unwrap().gateway.id;
    let l2 = db
        .create_line(
            gw,
            Line {
                name: "Linie 2".into(),
                baud: 9600,
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    db.add_meter(l1, meter("12345678")).unwrap();
    // ID, Maske und volle Sekundäradresse sind derselbe Zähler
    assert!(db.add_meter(l1, meter("12345678ffffffff")).is_err());
    assert!(db.add_meter(l1, meter("123456782C2D1B04")).is_err());
    assert!(db.add_meter(l1, meter("x")).is_err());
    // auf einer anderen Linie erlaubt
    db.add_meter(l2, meter("12345678")).unwrap();
    assert_eq!(
        db.import_meters(l1, vec![meter("5"), meter("6"), meter("5")])
            .unwrap(),
        2
    );
    assert_eq!(db.meters(l1).unwrap().len(), 3);
    assert_eq!(db.clear_meters(l1).unwrap(), 3);
    assert_eq!(db.meters(l2).unwrap().len(), 1);
}

#[test]
fn gateways_and_lines_can_be_reordered() {
    let mut db = Db::memory().unwrap();
    let p = db.active().unwrap().project.id;
    let g2 = db
        .create_gateway(
            p,
            Gateway {
                name: "B".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let g3 = db
        .create_gateway(
            p,
            Gateway {
                name: "C".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let names = |db: &Db| -> Vec<String> {
        db.tree().unwrap()[0]
            .gateways
            .iter()
            .map(|g| g.gateway.name.clone())
            .collect()
    };
    assert_eq!(names(&db), ["Gateway 1", "B", "C"]);
    db.move_gateway(g3.id, -1).unwrap();
    assert_eq!(names(&db), ["Gateway 1", "C", "B"]);
    db.move_gateway(g3.id, -1).unwrap();
    db.move_gateway(g3.id, -1).unwrap(); // schon oben: nichts passiert
    assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
    db.move_gateway(g2.id, 1).unwrap(); // schon unten
    assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
    let l2 = db
        .create_line(
            g2.id,
            Line {
                name: "L2".into(),
                baud: 2400,
                ..Default::default()
            },
        )
        .unwrap();
    db.move_line(l2.id, -1).unwrap();
    let lines: Vec<String> = db.tree().unwrap()[0].gateways[2]
        .lines
        .iter()
        .map(|l| l.line.name.clone())
        .collect();
    assert_eq!(lines[0], "L2");
}

#[test]
fn deleting_the_active_project_keeps_a_line_active() {
    let mut db = Db::memory().unwrap();
    let p2 = db
        .create_project(Project {
            name: "B".into(),
            ..Default::default()
        })
        .unwrap();
    let first = db.active().unwrap().project.id;
    db.delete_project(first).unwrap();
    assert_eq!(db.active().unwrap().project.id, p2.id);
    // letztes Projekt gelöscht: neues Standardprojekt
    db.delete_project(p2.id).unwrap();
    assert_eq!(db.tree().unwrap().len(), 1);
}

#[test]
fn export_and_import_round_trip() {
    let mut db = Db::memory().unwrap();
    let a = db.active().unwrap();
    db.update_project(
        a.project.id,
        Project {
            name: "MFH".into(),
            customer: "Muster AG".into(),
            ..Default::default()
        },
    )
    .unwrap();
    db.add_meter(
        a.line.id,
        Meter {
            name: "Whg 1".into(),
            ..meter("3")
        },
    )
    .unwrap();
    db.add_change(a.line.id, "3: PA 0 → 3").unwrap();
    let file = db.export_project(a.project.id).unwrap();
    let id = db.import_project(&file).unwrap();
    let r = db.report(id).unwrap();
    assert_eq!(r["project"]["customer"], "Muster AG");
    assert_eq!(
        r["gateways"][0]["lines"][0]["meters"][0]["meter"]["name"],
        "Whg 1"
    );
    assert_eq!(r["changes"][0]["text"], "3: PA 0 → 3");
    assert!(db.import_project(&json!({ "format": "x" })).is_err());
}

#[test]
fn thinning_keeps_one_reading_per_day() {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    let m = db.add_meter(line, meter("1")).unwrap();
    for day in 1..=10 {
        for hour in [0, 6, 12, 18] {
            db.conn
                .execute(
                    "INSERT INTO readings (line_id, meter, timestamp, data) VALUES (?1, ?2, ?3, '{}')",
                    params![line, m.id, format!("2026-10-{day:02} {hour:02}:00:00")],
                )
                .unwrap();
        }
    }
    let now = chrono::NaiveDateTime::parse_from_str("2026-10-10 12:00", "%Y-%m-%d %H:%M").unwrap();
    // ausgeschaltet: nichts löschen
    assert_eq!(db.thin_readings(now).unwrap(), 0);
    db.set_thin_days(1, Some(5)).unwrap();
    // älter als 5 Tage (vor dem 5.): Tag 1–4 je 3 von 4 weg
    assert_eq!(db.thin_readings(now).unwrap(), 12);
    let left: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 40 - 12);
    let first: String = db
        .conn
        .query_row(
            "SELECT timestamp FROM readings ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(first, "2026-10-01 00:00:00");
}

#[test]
fn imports_old_meter_list() {
    let dir = std::env::temp_dir().join(format!("mbusio-db-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("meters.json"),
        r#"{"meters":[{"address":"5","name":"Alt"}]}"#,
    )
    .unwrap();
    let mut db = Db::open(&dir).unwrap();
    let line = db.active_line().unwrap().id;
    assert_eq!(db.meters(line).unwrap()[0].name, "Alt");
    db.set_issues(line, &json!({ "count": 2, "items": ["a", "b"] }))
        .unwrap();
    assert_eq!(
        db.tree().unwrap()[0].gateways[0].lines[0]
            .issues
            .as_ref()
            .unwrap()["count"],
        2
    );
    assert!(dir.join("meters.json.imported").exists());
    drop(db);
    // zweiter Start: nichts doppelt
    let db = Db::open(&dir).unwrap();
    assert_eq!(db.meters(line).unwrap().len(), 1);
    drop(db);
    std::fs::remove_dir_all(&dir).unwrap();
}
