//! Tests des Berichts: Termine, verpasst, Fehlschlag, Teile, Text

use super::*;
use crate::storage::Level;

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
}

fn settings() -> ReportSettings {
    ReportSettings {
        to: "a@example.org".into(),
        ..Default::default()
    }
}

#[test]
fn daily_starts_quietly_then_once_per_day() {
    let s = settings();
    let mut r = ReportState::default();
    // eingeschaltet um 10:00: kein Bericht sofort, der Zeitraum beginnt heute 07:00
    assert_eq!(r.due(&s, at("2026-10-27 10:00")), (true, None));
    assert_eq!(r.due(&s, at("2026-10-28 06:59")), (false, None));
    let (_, since) = r.due(&s, at("2026-10-28 07:00"));
    assert_eq!(since, Some(at("2026-10-27 07:00")));
    r.done(true, at("2026-10-28 07:00"));
    assert_eq!(r.due(&s, at("2026-10-28 12:00")), (false, None));
    // drei Tage aus: ein Bericht über die ganze Zeit, nicht drei
    let (_, since) = r.due(&s, at("2026-10-31 09:00"));
    assert_eq!(since, Some(at("2026-10-28 07:00")));
    r.done(true, at("2026-10-31 09:00"));
    assert_eq!(r.due(&s, at("2026-10-31 09:10")), (false, None));
}

#[test]
fn weekly_failed_and_switched_off() {
    let mut s = settings();
    s.frequency = "weekly".into();
    s.weekday = 2; // Mittwoch
    s.time = "08:30".into();
    let mut r = ReportState::default();
    // Dienstag, 27.10.: Zeitraum ab Mittwoch der Vorwoche
    r.due(&s, at("2026-10-27 10:00"));
    assert_eq!(r.due(&s, at("2026-10-28 08:29")), (false, None));
    let (_, since) = r.due(&s, at("2026-10-28 08:30"));
    assert_eq!(since, Some(at("2026-10-21 08:30")));
    // nicht gesendet: nach einer Stunde neu, Zeitraum bleibt
    r.done(false, at("2026-10-28 08:30"));
    assert_eq!(r.due(&s, at("2026-10-28 09:00")), (false, None));
    let (_, since) = r.due(&s, at("2026-10-28 09:30"));
    assert_eq!(since, Some(at("2026-10-21 08:30")));
    // ausgeschaltet: vergessen; wieder an, dann wieder ohne sofortigen Bericht
    s.frequency = "off".into();
    assert_eq!(r.due(&s, at("2026-10-28 10:00")), (true, None));
    assert_eq!(r, ReportState::default());
    s.frequency = "daily".into();
    s.to.clear();
    assert_eq!(r.due(&s, at("2026-10-28 10:00")), (false, None));
}

fn data() -> ReportData {
    ReportData {
        reading: vec![ReadLine {
            place: "Haus A › GW › Linie 1".into(),
            read: 12,
            total: 14,
        }],
        outputs: vec![OutputCount {
            output: "mqtt".into(),
            sent: 300,
            dropped: 2,
            failed: 0,
        }],
        problems: Vec::new(),
        billing: vec![("Haus A".into(), 1, 1)],
        disk: DiskState {
            level: Level::Ok,
            free_pct: 40.0,
            free_bytes: 8 * 1024 * 1024 * 1024,
            days_left: Some(900),
            ..Default::default()
        },
    }
}

#[test]
fn text_has_parts_and_subject() {
    crate::i18n::set(crate::i18n::Lang::De);
    let mut s = settings();
    let (subject, text) = mail(
        &s,
        &data(),
        "keller",
        at("2026-10-27 07:00"),
        at("2026-10-28 07:00"),
    );
    assert_eq!(subject, "mbusio keller: Bericht 28.10.2026");
    for part in [
        "Zeitraum 27.10.2026 07:00 – 28.10.2026 07:00",
        "- Haus A › GW › Linie 1: 12 von 14 Zählern gelesen",
        "- MQTT: 300 gesendet, 2 verworfen",
        "Offene Probleme:\r\n- keine",
        "- Haus A: 1 abgelegt, 1 per E-Mail gesendet",
        "- frei 8.0 GB (40 %), reicht noch ca. 900 Tage",
    ] {
        assert!(text.contains(part), "{part}: {text}");
    }
    s.off = vec!["outputs".into(), "disk".into()];
    s.subject = "{geraet} {datum} {x}".into();
    let (subject, text) = mail(
        &s,
        &data(),
        "keller",
        at("2026-10-27 07:00"),
        at("2026-10-28 07:00"),
    );
    assert_eq!(subject, "keller 28.10.2026 {x}");
    assert!(!text.contains("MQTT") && !text.contains("frei"), "{text}");
}

#[test]
fn settings_are_checked() {
    let mut s = ReportSettings::default();
    assert!(s.check().is_ok());
    s.time = "7 Uhr".into();
    assert!(s.check().is_err());
    s.time = "07:00".into();
    s.frequency = "hourly".into();
    assert!(s.check().is_err());
    s.frequency = "weekly".into();
    s.weekday = 7;
    assert!(s.check().is_err());
    s.weekday = 6;
    s.off = vec!["nonsense".into()];
    assert!(s.check().is_err());
    // alte Einstellungen ohne `report` laden mit Vorgaben
    let c: crate::mail::MailConfig = serde_json::from_str(r#"{"host":"x"}"#).unwrap();
    assert_eq!(c.report, ReportSettings::default());
}
