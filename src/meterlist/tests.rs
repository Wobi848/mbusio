//! Tests der Zählerliste vom Kunden

use super::*;
use crate::db::Line;

fn row(id: &str, name: &str, loc: &str) -> ListRow {
    ListRow {
        id: id.into(),
        name: name.into(),
        location: loc.into(),
        ..Default::default()
    }
}

#[test]
fn ids_are_normalized() {
    assert_eq!(normalize_id("71000001").as_deref(), Some("71000001"));
    assert_eq!(normalize_id("KAM 1234").as_deref(), Some("00001234"));
    assert_eq!(normalize_id("123456789"), None);
    assert_eq!(normalize_id("abc"), None);
}

#[test]
fn list_creates_and_renames_with_preview() {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    db.add_meter(
        line,
        Meter {
            address: "71000001".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let rows = vec![
        row("71000001", "Whg 1 Wärme", "Haus A · Whg 1"),
        row("72000001", "Whg 1 Kaltwasser", "Haus A · Whg 1"),
        row("", "", ""),
    ];
    let p = db.import_list(line, &rows, false).unwrap();
    assert_eq!((p.added.len(), p.changed.len(), p.errors.len()), (1, 2, 0));
    assert!(!p.applied);
    assert_eq!(db.meters(line).unwrap().len(), 1, "Vorschau ändert nichts");
    let done = db.import_list(line, &rows, true).unwrap();
    assert!(done.applied);
    let ms = db.meters(line).unwrap();
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].name, "Whg 1 Wärme");
    assert_eq!(ms[1].address, "72000001");
    assert_eq!(ms[1].location, "Haus A · Whg 1");
    // nochmal: nichts mehr zu tun; leere Zellen überschreiben nichts
    let again = db
        .import_list(line, &[row("72000001", "", "")], false)
        .unwrap();
    assert_eq!(
        (again.added.len(), again.changed.len(), again.unchanged),
        (0, 0, 1)
    );
    // Fehler: nichts wird übernommen
    let bad = db
        .import_list(
            line,
            &[row("1234567890", "x", ""), row("73000001", "y", "")],
            true,
        )
        .unwrap();
    assert!(!bad.applied && bad.errors.len() == 1);
    assert_eq!(db.meters(line).unwrap().len(), 2);
}

#[test]
fn primary_line_uses_pa_from_list() {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap();
    db.set_addressing(line.id, "primary").unwrap();
    let mut r = row("71000001", "A", "");
    r.pa = "5".into();
    db.import_list(line.id, &[r], true).unwrap();
    assert_eq!(db.meters(line.id).unwrap()[0].address, "5");
    let _ = Line::default();
}

#[test]
fn names_from_pattern() {
    let m = Meter {
        location: "Whg 3".into(),
        medium: Some("medium_water".into()),
        address: "71000003".into(),
        ..Default::default()
    };
    assert_eq!(
        render_name("{ort} {medium}", &m, 1, Lang::De),
        "Whg 3 Wasser"
    );
    // leerer Ort: kein Trenner am Anfang
    let m2 = Meter {
        location: String::new(),
        ..m.clone()
    };
    assert_eq!(render_name("{ort} · {medium}", &m2, 1, Lang::De), "Wasser");
    assert_eq!(render_name("Z{nr} {id}", &m, 7, Lang::De), "Z7 71000003");
}
