//! Tests des Exports für die Abrechnung: Zeitraum, Herkunft des Stands, Abrechnung, Tausch, Zeitreihe

use super::*;
use crate::db::Meter;
use crate::mbus::master::{FrameLog, Master, Target};
use crate::testutil::with_energy;
use std::sync::Arc;

const ENERGY: &str = "instantaneous|0|0|0|energy|kWh";

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Wärmezähler 1 aus dem Simulator: liefert auch „Energie · Speicher 1“ zum 30.09.2026
fn heat_reading() -> MeterReading {
    let link = crate::link::open("SIMULATOR", 2400).unwrap();
    let mut m = Master::new(link, 2400, 60, Arc::new(FrameLog::default()));
    m.read_meter(&Target::Primary(1)).unwrap()
}

/// Auslesung mit Energie `value` zur Zeit `time`
fn at(r: &MeterReading, time: &str, value: f64) -> MeterReading {
    let mut r = with_energy(r, value);
    r.timestamp = time.into();
    r
}

/// Projekt mit einem Wärmezähler und den Auslesungen `(Zeit, Energie)`
fn with_readings(list: &[(&str, f64)]) -> (Db, i64, i64, MeterReading) {
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
    for (time, value) in list {
        db.record(line, &Target::Primary(1), Ok(&at(&r, time, *value)), false)
            .unwrap();
    }
    (db, line, m.id, r)
}

fn custom(from: &str, to: &str) -> Settings {
    Settings {
        period: "custom".into(),
        from: Some(from.into()),
        to: Some(to.into()),
        ..Default::default()
    }
}

#[test]
fn period_presets_and_checks() {
    let today = day("2026-10-26");
    let s = Settings::default();
    assert_eq!(
        s.range(today).unwrap(),
        (day("2026-09-01"), day("2026-09-30"))
    );
    let jan = day("2026-01-10");
    assert_eq!(
        s.range(jan).unwrap(),
        (day("2025-12-01"), day("2025-12-31"))
    );
    let year = Settings {
        period: "prev_year".into(),
        ..Default::default()
    };
    assert_eq!(
        year.range(today).unwrap(),
        (day("2025-01-01"), day("2025-12-31"))
    );
    assert!(custom("2026-09-30", "2026-09-01").range(today).is_err());
    assert!(custom("2026-09-01", "").range(today).is_err());
    assert!(Settings {
        decimal: ";".into(),
        ..Default::default()
    }
    .check()
    .is_err());
}

#[test]
fn stand_comes_from_meter_reading_or_after() {
    let (db, _, mid, _) = with_readings(&[
        ("2026-08-31 23:00:00", 1000.0),
        ("2026-09-15 12:00:00", 1500.0),
        ("2026-10-02 08:00:00", 2100.0),
    ]);
    // Ende 15.09.: letzte Auslesung am Tag
    let s = db.stand_at(mid, ENERGY, day("2026-09-15"), None).unwrap();
    assert_eq!(
        (s.value, s.source, s.time.as_str()),
        (Some(1500.0), Source::Reading, "2026-09-15 12:00:00")
    );
    // Ende 30.09.: der Zähler hat einen Stichtagswert zum 30.09. gespeichert (Auslesung 02.10.)
    let s = db.stand_at(mid, ENERGY, day("2026-09-30"), None).unwrap();
    assert_eq!((s.value, s.source), (Some(120000.0), Source::Meter));
    // vor der ersten Auslesung: nichts, ausser die erste danach ist erlaubt
    let s = db.stand_at(mid, ENERGY, day("2026-08-01"), None).unwrap();
    assert_eq!(s.source, Source::Missing);
    let s = db
        .stand_at(mid, ENERGY, day("2026-08-01"), Some(day("2026-09-30")))
        .unwrap();
    assert_eq!((s.value, s.source), (Some(1000.0), Source::After));
    // die erste danach nur im Zeitraum
    let s = db
        .stand_at(mid, ENERGY, day("2026-08-01"), Some(day("2026-08-15")))
        .unwrap();
    assert_eq!(s.source, Source::Missing);
}

#[test]
fn billing_rows_with_consumption_scale_and_notes() {
    let (mut db, line, mid, r) = with_readings(&[
        ("2026-07-31 23:00:00", 900.0),
        ("2026-08-31 23:00:00", 1000.0),
    ]);
    // Ende ohne Stichtagswert (August), Zeit unsicher am Ende
    let mut end = at(&r, "2026-08-31 23:30:00", 1234.5);
    end.time_unsure = true;
    db.record(line, &Target::Primary(1), Ok(&end), false)
        .unwrap();
    // ein zweiter Zähler, nie ausgelesen
    db.add_meter(
        line,
        Meter {
            address: "2".into(),
            name: "Whg 2".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let t = db
        .billing_table(
            1,
            &custom("2026-08-01", "2026-08-31"),
            day("2026-10-26"),
            None,
        )
        .unwrap();
    assert_eq!(t.total, 2);
    assert_eq!(t.header[9], "Stand Anfang");
    let row = &t.rows[0];
    assert_eq!(&row[2], "Whg 1");
    assert_eq!((row[7].as_str(), row[8].as_str()), ("Energie", "kWh"));
    assert_eq!(row[9], "900", "Anfang: Stand Ende Juli");
    assert!(row[10].starts_with("Auslesung 31.07.2026 23:00"));
    assert_eq!(row[11], "1234,5", "Dezimalkomma auf Deutsch");
    assert_eq!(row[13], "334,5");
    assert!(row[14].contains("Zeit unsicher"), "{}", row[14]);
    assert_eq!(t.rows[1][14], "noch nie ausgelesen");
    assert_eq!(t.notes, 2);

    // Punkt einstellbar; Faktor und Einheit des Datenpunkts
    db.dp_select(mid, &[ENERGY.to_string()], &[], crate::i18n::Lang::De)
        .unwrap();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    let mut d = db.dp(id).unwrap();
    d.factor = 0.001;
    d.unit = "MWh".into();
    db.dp_update(id, &d).unwrap();
    let mut s = custom("2026-08-01", "2026-08-31");
    s.decimal = ".".into();
    let t = db.billing_table(1, &s, day("2026-10-26"), Some(1)).unwrap();
    assert_eq!(
        (t.rows.len(), t.total),
        (1, 2),
        "Vorschau rechnet nur die ersten Zeilen"
    );
    assert_eq!(
        (t.rows[0][8].as_str(), t.rows[0][13].as_str()),
        ("MWh", "0.3345")
    );
}

#[test]
fn consumption_continues_over_meter_replacement() {
    let today = crate::clock::now().date();
    let ts = |d: NaiveDate, t: &str| format!("{d} {t}");
    let (mut db, _, mid, r) = with_readings(&[
        (&ts(today - Duration::days(5), "10:00:00"), 100.0),
        (&ts(today - Duration::days(1), "10:00:00"), 150.0),
    ]);
    // neuer Zähler: gelesen kurz vor dem Eintrag des Tauschs
    let mut new = with_energy(&r, 10.0);
    new.header.as_mut().unwrap().id = "99999999".into();
    new.timestamp = (crate::clock::now() - Duration::minutes(1))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    db.exchange_meter(mid, "5", &new).unwrap();
    let mut later = at(&new, &ts(today, "23:59:59"), 30.0);
    later.header.as_mut().unwrap().id = "99999999".into();
    let line = db.meter(mid).unwrap().line_id;
    db.record(line, &Target::Primary(5), Ok(&later), false)
        .unwrap();
    let from = (today - Duration::days(10)).to_string();
    let t = db
        .billing_table(1, &custom(&from, &today.to_string()), today, None)
        .unwrap();
    let row = &t.rows[0];
    // (150 − 100) alter Zähler + (30 − 10) neuer Zähler
    assert_eq!(row[13], "70", "{row:?}");
    assert!(row[10].starts_with("erste Auslesung danach"), "{}", row[10]);
    assert!(row[14].contains("Zählertausch"), "{}", row[14]);
}

#[test]
fn series_has_month_ends_and_end_of_period() {
    let (db, ..) = with_readings(&[("2026-01-31 12:00:00", 10.0), ("2026-02-28 12:00:00", 20.0)]);
    let mut s = custom("2026-01-01", "2026-03-10");
    s.form = "series".into();
    let t = db.billing_table(1, &s, day("2026-10-26"), None).unwrap();
    let dates: Vec<&str> = t.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(dates, ["2026-01-31", "2026-02-28", "2026-03-10"]);
    assert_eq!(
        (t.rows[0][10].as_str(), t.rows[2][10].as_str()),
        ("10", "20")
    );
    s.step = "day".into();
    assert_eq!(
        db.billing_table(1, &s, day("2026-10-26"), Some(3))
            .unwrap()
            .total,
        69
    );
}

/// Erkannter Tausch (nicht bestätigt), im Zeitraum nicht gelesen, Fehler des Zählers: Hinweis je
/// Zeile und Zusammenfassung (Konzept Export 3.6)
#[test]
fn notes_for_detected_swap_not_read_and_fault() {
    let (mut db, line, mid, r) = with_readings(&[("2026-08-10 10:00:00", 100.0)]);
    // anderer Zähler unter derselben Adresse: erkannt, nicht bestätigt
    let mut new = at(&r, "2026-08-20 10:00:00", 10.0);
    new.header.as_mut().unwrap().id = "99999999".into();
    db.record(line, &Target::Primary(1), Ok(&new), false)
        .unwrap();
    // Endstand: der neue Zähler meldet einen Fehler im Statusbyte
    let mut last = at(&new, "2026-08-30 10:00:00", 30.0);
    last.header.as_mut().unwrap().status = 0x08;
    db.record(line, &Target::Primary(1), Ok(&last), false)
        .unwrap();
    let t = db
        .billing_table(
            1,
            &custom("2026-08-01", "2026-08-31"),
            day("2026-10-26"),
            None,
        )
        .unwrap();
    let row = &t.rows[0];
    assert_eq!(
        row[13], "20",
        "(100 − 100) bis zum Tausch, (30 − 10) danach: {row:?}"
    );
    assert!(
        row[14].contains("Zählertausch erkannt 20.08.2026 10:00 (nicht bestätigt)"),
        "{}",
        row[14]
    );
    assert!(row[14].contains("meldet Fehler"), "{}", row[14]);
    assert!(
        row[12].contains("meldet Fehler"),
        "Quelle Ende: {}",
        row[12]
    );
    assert_eq!((t.summary["swap_open"], t.summary["fault"]), (1, 1));
    // bestätigt: nur noch „Zählertausch“
    let ex = db.open_swaps(line).unwrap()[&mid]["id"].as_i64().unwrap();
    db.confirm_swap(ex).unwrap();
    let t = db
        .billing_table(
            1,
            &custom("2026-08-01", "2026-08-31"),
            day("2026-10-26"),
            None,
        )
        .unwrap();
    assert!(
        t.rows[0][14].starts_with("Zählertausch 20.08.2026"),
        "{}",
        t.rows[0][14]
    );
    assert!(!t.summary.contains_key("swap_open"));
    // Zeitraum ohne Auslesung
    let t = db
        .billing_table(
            1,
            &custom("2026-07-01", "2026-07-31"),
            day("2026-10-26"),
            None,
        )
        .unwrap();
    assert!(
        t.rows[0][14].contains("im Zeitraum nicht gelesen"),
        "{}",
        t.rows[0][14]
    );
    assert_eq!((t.summary["not_read"], t.summary["missing"]), (1, 1));
}
