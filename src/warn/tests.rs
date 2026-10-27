//! Tests der Warnregeln: neu, gebündelt, Abstand, behoben, Wartezeit, erinnern, aufgeben

use super::*;
use crate::i18n::both;

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
}

fn problem(key: &str, kind: &'static str) -> Problem {
    let key = key.to_string();
    Problem {
        text: both(|| format!("{} {key}", tr("Problem", "problem"))),
        key,
        kind,
        place: "Haus A › Linie 1".into(),
        since: None,
        line: None,
        meter: None,
        project: None,
    }
}

fn settings() -> WarnSettings {
    WarnSettings {
        to: "a@example.org".into(),
        ..Default::default()
    }
}

/// Mail fällig? Dann als gesendet verbuchen; liefert Betreff und Text
fn send(w: &mut WarnState, s: &WarnSettings, now: &str) -> Option<(String, String)> {
    let m = w.mail_due(s, "pi", at(now))?;
    w.mail_done(&m.keys, true, at(now));
    Some((m.subject, m.text))
}

#[test]
fn new_problems_are_bundled_and_not_repeated() {
    let s = settings();
    let mut w = WarnState::default();
    let now = at("2026-10-27 10:00");
    assert!(w.update(
        vec![
            problem("disturbed:1", "disturbed"),
            problem("battery:2", "battery")
        ],
        &[],
        &s,
        now
    ));
    // gleich geblieben: nichts zu speichern
    assert!(!w.update(
        vec![
            problem("disturbed:1", "disturbed"),
            problem("battery:2", "battery")
        ],
        &[],
        &s,
        now
    ));
    let (subject, text) = send(&mut w, &s, "2026-10-27 10:00").unwrap();
    assert!(subject.starts_with("mbusio pi: 2 "), "{subject}");
    assert!(text.contains("Haus A › Linie 1 · "), "{text}");
    // bleibt offen: keine zweite Mail, auch nach dem Abstand
    assert!(send(&mut w, &s, "2026-10-27 12:00").is_none());
}

#[test]
fn interval_collects_and_resolved_go_into_next_mail() {
    let s = settings();
    let mut w = WarnState::default();
    w.update(
        vec![problem("fault:1", "fault")],
        &[],
        &s,
        at("2026-10-27 10:00"),
    );
    send(&mut w, &s, "2026-10-27 10:00").unwrap();
    // behoben, dafür ein neues; innerhalb der Stunde keine Mail
    w.update(
        vec![problem("swap:3", "swap")],
        &[],
        &s,
        at("2026-10-27 10:20"),
    );
    assert!(send(&mut w, &s, "2026-10-27 10:30").is_none());
    let (subject, text) = send(&mut w, &s, "2026-10-27 11:00").unwrap();
    assert!(subject.contains(": 1 "), "{subject}");
    let resolved = text
        .split("\r\n\r\n")
        .find(|p| p.contains("(1):\r\n- ") && p.contains("fault:1"));
    assert!(resolved.is_some(), "behoben genannt: {text}");
    // danach nicht noch einmal behoben
    w.update(
        vec![problem("swap:3", "swap"), problem("disk", "disk")],
        &[],
        &s,
        at("2026-10-27 12:00"),
    );
    let (_, text) = send(&mut w, &s, "2026-10-27 12:00").unwrap();
    assert!(!text.contains("fault:1"), "{text}");
    assert!(text.contains("swap:3"), "weiterhin offen: {text}");
}

#[test]
fn waiting_time_switched_off_kinds_and_unsure() {
    let mut s = settings();
    let mut w = WarnState::default();
    w.update(
        vec![problem("line:1", "line")],
        &[],
        &s,
        at("2026-10-27 10:00"),
    );
    // erst nach 30 Minuten sichtbar und gemeldet
    assert!(w.visible(&s, at("2026-10-27 10:29")).is_empty());
    assert!(send(&mut w, &s, "2026-10-27 10:29").is_none());
    // unklar (verbindet gerade): bleibt mit seiner Zeit
    w.update(Vec::new(), &["line:1".into()], &s, at("2026-10-27 10:29"));
    assert_eq!(w.visible(&s, at("2026-10-27 10:30")).len(), 1);
    // ausgeschaltet: weg, ohne als behoben zu gelten
    s.off = vec!["line".into()];
    w.update(
        vec![problem("line:1", "line")],
        &[],
        &s,
        at("2026-10-27 10:31"),
    );
    assert!(w.open.is_empty());
    assert!(send(&mut w, &s, "2026-10-27 10:31").is_none());
}

#[test]
fn seen_flapping_remind_and_give_up() {
    let mut s = settings();
    let mut w = WarnState::default();
    w.update(
        vec![problem("fault:1", "fault")],
        &[],
        &s,
        at("2026-10-27 10:00"),
    );
    assert!(w.see(&[]));
    assert!(!w.see(&[]));
    send(&mut w, &s, "2026-10-27 10:00").unwrap();
    // kurz weg und wieder da vor der nächsten Mail: nicht neu
    w.update(Vec::new(), &[], &s, at("2026-10-27 10:10"));
    w.update(
        vec![problem("fault:1", "fault")],
        &[],
        &s,
        at("2026-10-27 10:20"),
    );
    assert!(send(&mut w, &s, "2026-10-28 09:00").is_none());
    // täglich erinnern
    s.remind = true;
    assert!(send(&mut w, &s, "2026-10-28 10:00").is_some());

    // Senden klappt nicht: alle 15 Minuten neu, nach einem Tag gilt es als gemeldet
    w.update(
        vec![problem("fault:1", "fault"), problem("battery:2", "battery")],
        &[],
        &s,
        at("2026-10-28 12:00"),
    );
    let m = w.mail_due(&s, "pi", at("2026-10-28 12:00")).unwrap();
    w.mail_done(&m.keys, false, at("2026-10-28 12:00"));
    assert!(w.mail_due(&s, "pi", at("2026-10-28 12:10")).is_none());
    let m = w.mail_due(&s, "pi", at("2026-10-28 12:15")).unwrap();
    w.mail_done(&m.keys, false, at("2026-10-29 12:15"));
    assert!(w.open.iter().all(|o| o.mailed.is_some()));
    // ohne Empfänger keine Mail
    s.to.clear();
    w.update(
        vec![problem("disk", "disk")],
        &[],
        &s,
        at("2026-10-30 12:00"),
    );
    assert!(w.mail_due(&s, "pi", at("2026-10-30 12:00")).is_none());
}

#[test]
fn settings_are_checked() {
    let mut s = WarnSettings::default();
    assert!(s.check().is_ok());
    s.off = vec!["nonsense".into()];
    assert!(s.check().is_err());
    s.off.clear();
    s.interval_minutes = 7;
    assert!(s.check().is_err());
    s.interval_minutes = 0;
    s.to = "kein Empfänger".into();
    assert!(s.check().is_err());
    // alte Einstellungen ohne `warn` laden mit Vorgaben
    let c: crate::mail::MailConfig = serde_json::from_str(r#"{"host":"x"}"#).unwrap();
    assert_eq!(c.warn, WarnSettings::default());
}
