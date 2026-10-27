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

/// Spalte „Hinweis“: immer die letzte (davor stehen je nach Einstellung die Vorjahre)
fn note(row: &[String]) -> &str {
    row.last().unwrap()
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
    assert!(note(row).contains("Zeit unsicher"), "{}", note(row));
    assert_eq!(note(&t.rows[1]), "noch nie ausgelesen");
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
    assert!(note(row).contains("Zählertausch"), "{}", note(row));
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
        note(row).contains("Zählertausch erkannt 20.08.2026 10:00 (nicht bestätigt)"),
        "{}",
        note(row)
    );
    assert!(note(row).contains("meldet Fehler"), "{}", note(row));
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
        note(&t.rows[0]).starts_with("Zählertausch 20.08.2026"),
        "{}",
        note(&t.rows[0])
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
        note(&t.rows[0]).contains("im Zeitraum nicht gelesen"),
        "{}",
        note(&t.rows[0])
    );
    assert_eq!((t.summary["not_read"], t.summary["missing"]), (1, 1));
}

/// Automatisch (Konzept Export 3.5): erst nach der ersten Auslesung des neuen Zeitraums, einmal je
/// Zeitraum, verpasste Zeiträume nachholen; ruht bei eigenem Zeitraum
#[test]
fn auto_export_waits_for_reading_and_catches_up() {
    let (mut db, line, _, r) = with_readings(&[("2026-09-30 12:00:00", 100.0)]);
    let s = Settings {
        auto: true,
        ..Default::default()
    };
    db.set_billing_settings(1, &s).unwrap();
    // eigener Zeitraum: ruht
    let mut own = custom("2026-09-01", "2026-09-30");
    own.auto = true;
    db.set_billing_settings(1, &own).unwrap();
    assert!(db.billing_auto_projects().unwrap().is_empty());
    db.set_billing_settings(1, &s).unwrap();
    let at_time = |t: &str| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M").unwrap();
    let dir = std::env::temp_dir().join(format!("mbusio-billing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // 1.10. kurz nach Mitternacht, noch nicht gelesen: warten
    assert!(db
        .billing_auto(&dir, at_time("2026-10-01 00:01"))
        .unwrap()
        .is_empty());
    db.record(
        line,
        &Target::Primary(1),
        Ok(&at(&r, "2026-10-01 00:05:00", 110.0)),
        false,
    )
    .unwrap();
    let done = db.billing_auto(&dir, at_time("2026-10-01 00:10")).unwrap();
    assert_eq!(done.len(), 1);
    let files = auto::export_files(&dir, 1).unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        files[0].name.ends_with("-2026-09-01-2026-09-30.csv"),
        "{}",
        files[0].name
    );
    let text = std::fs::read_to_string(auto::export_dir(&dir, 1).join(&files[0].name)).unwrap();
    assert!(text.starts_with('\u{feff}'));
    // einmal je Zeitraum
    assert!(db
        .billing_auto(&dir, at_time("2026-10-05 00:00"))
        .unwrap()
        .is_empty());

    // Gerät aus bis Mitte Januar: Oktober bis Dezember nachholen
    assert_eq!(
        db.billing_due(1, &s, at_time("2027-01-15 08:00")).unwrap(),
        [
            (day("2026-10-01"), day("2026-10-31")),
            (day("2026-11-01"), day("2026-11-30")),
            (day("2026-12-01"), day("2026-12-31")),
        ]
    );
    db.billing_auto(&dir, at_time("2027-01-15 08:00")).unwrap();
    assert_eq!(auto::export_files(&dir, 1).unwrap().len(), 4);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn auto_export_switched_on_again_only_last_period() {
    let (mut db, ..) = with_readings(&[("2026-09-30 12:00:00", 100.0)]);
    let on = Settings {
        auto: true,
        ..Default::default()
    };
    db.set_billing_settings(1, &on).unwrap();
    db.set_meta("billing_done:1", "2026-01-31").unwrap();
    let now = chrono::NaiveDateTime::parse_from_str("2026-10-26 08:00", "%Y-%m-%d %H:%M").unwrap();
    assert_eq!(db.billing_due(1, &on, now).unwrap().len(), 8);
    db.set_billing_settings(1, &Settings::default()).unwrap();
    db.set_billing_settings(1, &on).unwrap();
    assert_eq!(
        db.billing_due(1, &on, now).unwrap(),
        [(day("2026-09-01"), day("2026-09-30"))]
    );
}

/// E-Mail (Teil C): neue Datei geht an die Empfänger, fehlgeschlagene wird erneut versucht, ins
/// Protokoll nur erster Fehler, geänderter Grund und Erfolg
#[test]
fn auto_export_mails_file_and_retries() {
    let (mut db, line, _, r) = with_readings(&[("2026-09-30 12:00:00", 100.0)]);
    db.record(
        line,
        &Target::Primary(1),
        Ok(&at(&r, "2026-10-01 00:05:00", 110.0)),
        false,
    )
    .unwrap();
    let s = Settings {
        auto: true,
        mail_to: "a@example.org, b@example.org".into(),
        ..Default::default()
    };
    db.set_billing_settings(1, &s).unwrap();
    let dir = std::env::temp_dir().join(format!("mbusio-billing-mail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let now = chrono::NaiveDateTime::parse_from_str("2026-10-01 00:10", "%Y-%m-%d %H:%M").unwrap();
    db.billing_auto(&dir, now).unwrap();

    // E-Mail ausgeschaltet: nichts versuchen, kein Fehler an der Datei
    db.set_mail_config(&crate::mail::MailConfig {
        enabled: false,
        ..Default::default()
    })
    .unwrap();
    assert!(db.billing_mail_pending(&dir, now).unwrap().0.is_empty());
    assert!(db.billing_files(&dir, 1).unwrap()[0].mail.is_none());
    db.set_mail_config(&crate::mail::MailConfig::default())
        .unwrap();

    // ohne Mailserver: Fehler an der Datei, einmal gemeldet
    let (out, notes) = db.billing_mail_pending(&dir, now).unwrap();
    assert!(out.is_empty());
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].get().contains("E-Mail-Server fehlt"),
        "{}",
        notes[0].get()
    );
    assert_eq!(
        db.billing_mail_pending(&dir, now).unwrap().1.len(),
        0,
        "derselbe Grund nicht nochmal"
    );
    let f = &db.billing_files(&dir, 1).unwrap()[0];
    assert!(f.mail.as_ref().unwrap().error.is_some());

    // mit Server: gesendet, danach nichts mehr offen
    let (port, server) = crate::mail::tests::fake_server("PLAIN");
    db.set_mail_config(&crate::mail::tests::config(port, "geheim"))
        .unwrap();
    let (out, _) = db.billing_mail_pending(&dir, now).unwrap();
    assert_eq!(out.len(), 1);
    let o = &out[0];
    let result = crate::mail::send(&o.config, &o.message);
    let note = db
        .billing_mail_done(1, &o.name, result, now)
        .unwrap()
        .unwrap();
    assert!(note.get().contains("E-Mail gesendet"));
    let seen = server.join().unwrap();
    assert!(seen.contains("RCPT TO:<b@example.org>"));
    assert!(seen.contains("2026-09-01-2026-09-30.csv"));
    assert!(db.billing_mail_pending(&dir, now).unwrap().0.is_empty());
    assert!(db.billing_files(&dir, 1).unwrap()[0]
        .mail
        .as_ref()
        .unwrap()
        .sent
        .is_some());

    // Datei älter als 7 Tage (Zeit der Datei: jetzt): nicht mehr erneut versuchen
    db.billing_mail_done(1, &o.name, Err(anyhow::anyhow!("weg")), now)
        .unwrap();
    let later = chrono::Local::now().naive_local() + Duration::days(30);
    assert!(db.billing_mail_pending(&dir, later).unwrap().0.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Gestört im Zeitraum: von–bis aus dem Verlauf der Störungen, auch wenn der Zähler heute wieder
/// antwortet; nur gewählte Linien
#[test]
fn disturbed_in_period_and_chosen_lines() {
    use crate::db::{Gateway, Line, PlanOutcome};
    let today = crate::clock::now().date();
    let (mut db, _, mid, _) = with_readings(&[(&format!("{today} 00:00:01"), 100.0)]);
    for _ in 0..crate::db::DISTURBED_AFTER {
        db.plan_done(mid, "2026-01-01 00:00:00", PlanOutcome::Failed)
            .unwrap();
    }
    db.plan_done(mid, "2026-01-01 00:00:00", PlanOutcome::Read { ms: 100.0 })
        .unwrap();
    let period = custom(&(today - Duration::days(1)).to_string(), &today.to_string());
    let t = db.billing_table(1, &period, today, None).unwrap();
    assert!(
        note(&t.rows[0]).contains("gestört ") && note(&t.rows[0]).contains(" – "),
        "{}",
        note(&t.rows[0])
    );
    assert_eq!(t.summary["disturbed"], 1);
    // vor der Störung: kein Hinweis
    let before = custom("2026-01-01", "2026-01-31");
    assert!(!db.billing_table(1, &before, today, None).unwrap().rows[0]
        .last()
        .unwrap()
        .contains("gestört"));

    // zweite Linie mit einem Zähler: alle, nur eine, gelöschte Wahl = alle
    let gw = db
        .create_gateway(
            1,
            Gateway {
                name: "GW 2".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let line2 = db
        .create_line(
            gw.id,
            Line {
                name: "L2".into(),
                baud: 2400,
                ..Default::default()
            },
        )
        .unwrap();
    db.add_meter(
        line2.id,
        Meter {
            address: "7".into(),
            name: "Whg 7".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(db.billing_lines(1).unwrap().len(), 2);
    let mut s = period.clone();
    assert_eq!(db.billing_table(1, &s, today, None).unwrap().total, 2);
    s.lines = vec![line2.id];
    let t = db.billing_table(1, &s, today, None).unwrap();
    assert_eq!((t.total, t.rows[0][2].as_str()), (1, "Whg 7"));
    s.lines = vec![9999];
    assert_eq!(db.billing_table(1, &s, today, None).unwrap().total, 2);
}

/// Quartal, Halbjahr, Bezeichnungen, Jahre zurück (Konzept Export 3.7)
#[test]
fn quarter_half_labels_and_years_back() {
    let with = |p: &str| Settings {
        period: p.into(),
        ..Default::default()
    };
    let cases = [
        (
            "prev_quarter",
            "2026-10-26",
            "2026-07-01",
            "2026-09-30",
            "Q3/2026",
        ),
        (
            "prev_quarter",
            "2026-02-10",
            "2025-10-01",
            "2025-12-31",
            "Q4/2025",
        ),
        (
            "prev_half",
            "2026-10-26",
            "2026-01-01",
            "2026-06-30",
            "H1/2026",
        ),
        (
            "prev_half",
            "2026-03-01",
            "2025-07-01",
            "2025-12-31",
            "H2/2025",
        ),
        (
            "prev_month",
            "2026-03-15",
            "2026-02-01",
            "2026-02-28",
            "02/2026",
        ),
        (
            "prev_year",
            "2026-03-15",
            "2025-01-01",
            "2025-12-31",
            "2025",
        ),
    ];
    for (p, today, from, to, label) in cases {
        let s = with(p);
        let (f, t) = s.range(day(today)).unwrap();
        assert_eq!((f, t), (day(from), day(to)), "{p} {today}");
        assert_eq!(s.label(f, t), label);
    }
    // Monatsende bleibt Monatsende, auch im Schaltjahr
    assert_eq!(years_back(day("2025-02-28"), 1), day("2024-02-29"));
    assert_eq!(years_back(day("2024-02-29"), 1), day("2023-02-28"));
    assert_eq!(years_back(day("2026-09-15"), 2), day("2024-09-15"));
    assert!(Settings {
        compare_years: 11,
        ..Default::default()
    }
    .check()
    .is_err());
}

/// Vorjahre: je Jahr eine Spalte „Verbrauch <Zeitraum>“ vor dem Hinweis; fehlt ein Jahr, leer
/// mit Hinweis
#[test]
fn previous_years_as_columns() {
    let (db, ..) = with_readings(&[
        ("2025-07-31 12:00:00", 100.0),
        ("2025-08-31 12:00:00", 160.0),
        ("2026-07-31 12:00:00", 500.0),
        ("2026-08-31 12:00:00", 580.0),
    ]);
    let s = Settings {
        compare_years: 2,
        ..Default::default()
    };
    let t = db.billing_table(1, &s, day("2026-09-15"), None).unwrap();
    let n = t.header.len();
    assert_eq!(
        &t.header[n - 4..],
        [
            "Verbrauch",
            "Verbrauch 08/2025",
            "Verbrauch 08/2024",
            "Hinweis"
        ]
    );
    let row = &t.rows[0];
    assert_eq!(&row[n - 4..n - 1], ["80", "60", ""], "{row:?}");
    assert!(
        note(row).contains("Vorjahr unvollständig: 08/2024"),
        "{}",
        note(row)
    );
    assert_eq!(t.summary["prev_missing"], 1);
    // ohne Vorjahre: Verbrauch direkt vor dem Hinweis
    let t = db
        .billing_table(
            1,
            &Settings {
                compare_years: 0,
                ..Default::default()
            },
            day("2026-09-15"),
            None,
        )
        .unwrap();
    assert_eq!(&t.header[t.header.len() - 2..], ["Verbrauch", "Hinweis"]);
}

/// Betreff mit Platzhaltern (unbekannte bleiben stehen), „jetzt senden“ mit Anhang
#[test]
fn subject_placeholders_and_mail_now() {
    let (mut db, ..) = with_readings(&[("2026-09-30 12:00:00", 100.0)]);
    db.set_mail_config(&crate::mail::MailConfig {
        host: "smtp.example.org".into(),
        from: "mbusio@example.org".into(),
        device: "Keller-Pi".into(),
        ..Default::default()
    })
    .unwrap();
    let mut s = Settings {
        period: "prev_quarter".into(),
        mail_to: "a@example.org".into(),
        ..Default::default()
    };
    db.set_billing_settings(1, &s).unwrap();
    let (f, t) = s.range(day("2026-10-26")).unwrap();
    assert_eq!(
        db.billing_subject(1, &s, f, t).unwrap(),
        format!("Abrechnung {} Q3/2026", db.project(1).unwrap().name)
    );
    s.mail_subject = "{geraet}: {von}–{bis} {tippfehler}\r\nBcc: x@y.de".into();
    assert_eq!(
        db.billing_subject(1, &s, f, t).unwrap(),
        "Keller-Pi: 01.07.2026–30.09.2026 {tippfehler}Bcc: x@y.de",
        "Zeilenumbruch weg (Kopfzeile)"
    );
    db.set_billing_settings(1, &s).unwrap();
    let o = db.billing_mail_now(1, day("2026-10-26")).unwrap();
    assert_eq!(o.message.to, ["a@example.org"]);
    assert!(
        o.name.ends_with("-abrechnung-2026-07-01-2026-09-30.csv"),
        "{}",
        o.name
    );
    assert!(String::from_utf8_lossy(&o.message.attachments[0].data).contains("Verbrauch Q3/2025"));
    // ohne Empfänger: sagt es
    s.mail_to.clear();
    db.set_billing_settings(1, &s).unwrap();
    assert!(format!(
        "{:#}",
        db.billing_mail_now(1, day("2026-10-26")).err().unwrap()
    )
    .contains("Keine Empfänger"));
}

/// Warten auf eine Auslesung nach dem Ende ist einstellbar
#[test]
fn auto_export_wait_days_setting() {
    let (mut db, ..) = with_readings(&[("2026-09-30 12:00:00", 100.0)]);
    let at = |t: &str| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M").unwrap();
    let mut s = Settings {
        auto: true,
        ..Default::default()
    };
    db.set_billing_settings(1, &s).unwrap();
    assert!(
        db.billing_due(1, &s, at("2026-10-01 08:00"))
            .unwrap()
            .is_empty(),
        "Vorgabe 2 Tage"
    );
    s.wait_days = 0;
    assert_eq!(
        db.billing_due(1, &s, at("2026-10-01 00:10")).unwrap().len(),
        1
    );
    s.wait_days = 40;
    assert!(db.set_billing_settings(1, &s).is_err());
}
