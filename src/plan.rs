//! Ausleseplan: wann ein Zähler geplant ausgelesen wird (Konzept Dauerbetrieb, 4.2–4.4).
//!
//! Termine liegen im Raster der Uhr (15 min: :00/:15/:30/:45, stündlich :00, täglich und monatlich
//! zu einer festen Uhrzeit), damit Werte verschiedener Zähler und Tage vergleichbar sind.
//! Alles hier ist reine Rechnung ohne Datenbank und Bus, damit es sich gut testen lässt.

use anyhow::{bail, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

use crate::i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Off,
    /// am 1. des Monats um diese Uhrzeit
    Monthly(NaiveTime),
    Daily(NaiveTime),
    Hourly,
    Quarter,
}

/// Format der Zeitstempel und Termine in der Datenbank
pub const SLOT_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Wird der Zeitpunkt des letzten Termins um mehr als das von der Uhr überholt (rückwärts),
/// richten sich die Termine neu aus. Kleinere Rücksprünge (Sommerzeit-Ende) werden ausgesessen,
/// damit es keine doppelten Zeitstempel gibt.
const REALIGN_AFTER: Duration = Duration::hours(2);

impl Plan {
    /// "off", "monthly 00:05", "daily 00:05", "hourly", "15min"
    pub fn parse(s: &str) -> Result<Plan> {
        let s = s.trim();
        let time = |t: &str| {
            NaiveTime::parse_from_str(t.trim(), "%H:%M").map_err(|_| {
                anyhow::anyhow!("{}: {t}", tr("Ungültige Uhrzeit", "Invalid time"))
            })
        };
        Ok(match s.split_once(' ') {
            Some(("daily", t)) => Plan::Daily(time(t)?),
            Some(("monthly", t)) => Plan::Monthly(time(t)?),
            None if s == "off" => Plan::Off,
            None if s == "hourly" => Plan::Hourly,
            None if s == "15min" => Plan::Quarter,
            _ => bail!("{}: {s}", tr("Unbekannter Ausleseplan", "Unknown read plan")),
        })
    }

    /// Gegenstück zu `parse`, so wird der Plan gespeichert
    pub fn key(&self) -> String {
        match self {
            Plan::Off => "off".into(),
            Plan::Monthly(t) => format!("monthly {}", t.format("%H:%M")),
            Plan::Daily(t) => format!("daily {}", t.format("%H:%M")),
            Plan::Hourly => "hourly".into(),
            Plan::Quarter => "15min".into(),
        }
    }

    /// Mittlerer Abstand der Termine in Sekunden (für die Buslast)
    pub fn interval_s(&self) -> Option<f64> {
        match self {
            Plan::Off => None,
            Plan::Monthly(_) => Some(30.0 * 86400.0),
            Plan::Daily(_) => Some(86400.0),
            Plan::Hourly => Some(3600.0),
            Plan::Quarter => Some(900.0),
        }
    }

    /// Letzter Termin bis einschliesslich `now`
    pub fn prev_slot(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        let date = now.date();
        Some(match self {
            Plan::Off => return None,
            Plan::Quarter => date.and_hms_opt(now.hour(), now.minute() / 15 * 15, 0)?,
            Plan::Hourly => date.and_hms_opt(now.hour(), 0, 0)?,
            Plan::Daily(t) => {
                let today = date.and_time(*t);
                if today <= now {
                    today
                } else {
                    today - Duration::days(1)
                }
            }
            Plan::Monthly(t) => {
                let this = first_of_month(date.year(), date.month())?.and_time(*t);
                if this <= now {
                    this
                } else {
                    let (y, m) = if date.month() == 1 {
                        (date.year() - 1, 12)
                    } else {
                        (date.year(), date.month() - 1)
                    };
                    first_of_month(y, m)?.and_time(*t)
                }
            }
        })
    }

    /// Nächster Termin nach `now`
    pub fn next_slot(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        let prev = self.prev_slot(now)?;
        Some(match self {
            Plan::Off => return None,
            Plan::Quarter => prev + Duration::minutes(15),
            Plan::Hourly => prev + Duration::hours(1),
            Plan::Daily(_) => prev + Duration::days(1),
            Plan::Monthly(t) => {
                let d = prev.date();
                let (y, m) = if d.month() == 12 {
                    (d.year() + 1, 1)
                } else {
                    (d.year(), d.month() + 1)
                };
                first_of_month(y, m)?.and_time(*t)
            }
        })
    }
}

fn first_of_month(y: i32, m: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y, m, 1)
}

/// Plan eines Zählers: eigener, sonst der der Linie, sonst der des Projekts.
/// Liefert den Plan und woher er kommt ("meter", "line", "project").
pub fn effective(meter: Option<&str>, line: Option<&str>, project: &str) -> (Plan, &'static str) {
    let parsed = |s: Option<&str>| s.and_then(|s| Plan::parse(s).ok());
    if let Some(p) = parsed(meter) {
        return (p, "meter");
    }
    if let Some(p) = parsed(line) {
        return (p, "line");
    }
    (parsed(Some(project)).unwrap_or(Plan::Off), "project")
}

/// Was der Plan für einen Zähler gerade verlangt
#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    No,
    /// auslesen; danach gilt dieser Termin als erledigt
    Read(NaiveDateTime),
    /// Plan neu oder Uhr weit zurückgesprungen: ohne Auslesen auf diesen Termin setzen,
    /// gelesen wird ab dem nächsten
    Start(NaiveDateTime),
    /// Plan ist aus, der gemerkte Zustand gehört weg (sonst würde beim Wiedereinschalten
    /// gleich ein alter Termin nachgeholt)
    Reset,
}

/// Gemerkter Zustand eines Zählers im Plan
#[derive(Debug, Default, Clone)]
pub struct State {
    /// Plan, zu dem `slot` gehört
    pub key: Option<String>,
    /// zuletzt erledigter Termin
    pub slot: Option<NaiveDateTime>,
    /// ausgesetzt bis (Zähler hat abgelehnt)
    pub pause: Option<NaiveDateTime>,
}

pub fn parse_time(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, SLOT_FORMAT).ok()
}

pub fn due(plan: Plan, state: &State, now: NaiveDateTime) -> Due {
    let Some(prev) = plan.prev_slot(now) else {
        return if state.key.is_some() { Due::Reset } else { Due::No };
    };
    if state.key.as_deref() != Some(plan.key().as_str()) {
        return Due::Start(prev);
    }
    if state.pause.is_some_and(|p| now < p) {
        return Due::No;
    }
    match state.slot {
        None => Due::Start(prev),
        Some(s) if s > now + REALIGN_AFTER => Due::Start(prev),
        // verpasste Termine (aus, getrennt) werden einmal nachgeholt, nicht jeder einzeln
        Some(s) if s < prev => Due::Read(prev),
        Some(_) => Due::No,
    }
}

/// Ausgesetzt nach Ablehnung durch den Zähler: bis zum Beginn des nächsten Tages
pub fn pause_until(now: NaiveDateTime) -> NaiveDateTime {
    (now.date() + Duration::days(1)).and_time(NaiveTime::MIN)
}

/// Geschätzte Dauer einer Auslesung, solange noch keine gemessen ist: ~250 Byte Antwort
/// mit 11 Bit je Byte plus Pausen
pub fn estimate_ms(baud: u32) -> f64 {
    300.0 + 250.0 * 11.0 * 1000.0 / baud.max(300) as f64
}

/// Anteil der Buszeit, den die geplanten Auslesungen einer Linie brauchen (1.0 = voll)
pub fn bus_load(items: impl IntoIterator<Item = (Plan, f64)>) -> f64 {
    items
        .into_iter()
        .filter_map(|(p, ms)| Some(ms / 1000.0 / p.interval_s()?))
        .sum()
}

/// Ab diesem Anteil warnt die Oberfläche: Bedienung und Wiederholungen brauchen auch Buszeit
pub const LOAD_WARN: f64 = 0.7;

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn st(key: &str, slot: &str) -> State {
        State {
            key: Some(key.into()),
            slot: Some(t(slot)),
            pause: None,
        }
    }

    #[test]
    fn parses_and_formats() {
        for k in ["off", "monthly 06:00", "daily 00:05", "hourly", "15min"] {
            assert_eq!(Plan::parse(k).unwrap().key(), k);
        }
        assert!(Plan::parse("daily 25:00").is_err());
        assert!(Plan::parse("weekly").is_err());
    }

    #[test]
    fn slots_follow_the_clock_raster() {
        let now = t("2026-10-17 14:37");
        assert_eq!(Plan::Quarter.prev_slot(now), Some(t("2026-10-17 14:30")));
        assert_eq!(Plan::Quarter.next_slot(now), Some(t("2026-10-17 14:45")));
        assert_eq!(Plan::Hourly.prev_slot(now), Some(t("2026-10-17 14:00")));
        let daily = Plan::parse("daily 00:05").unwrap();
        assert_eq!(daily.prev_slot(now), Some(t("2026-10-17 00:05")));
        assert_eq!(daily.next_slot(now), Some(t("2026-10-18 00:05")));
        assert_eq!(daily.prev_slot(t("2026-10-17 00:04")), Some(t("2026-10-16 00:05")));
        let monthly = Plan::parse("monthly 00:05").unwrap();
        assert_eq!(monthly.prev_slot(now), Some(t("2026-10-01 00:05")));
        assert_eq!(monthly.next_slot(now), Some(t("2026-11-01 00:05")));
        assert_eq!(monthly.prev_slot(t("2026-01-01 00:00")), Some(t("2025-12-01 00:05")));
        assert_eq!(monthly.next_slot(t("2026-12-05 00:00")), Some(t("2027-01-01 00:05")));
        assert_eq!(Plan::Off.prev_slot(now), None);
    }

    #[test]
    fn new_plan_starts_with_the_next_term() {
        let daily = Plan::parse("daily 00:05").unwrap();
        let now = t("2026-10-17 14:00");
        assert_eq!(due(daily, &State::default(), now), Due::Start(t("2026-10-17 00:05")));
        // anderer Plan als gemerkt: ebenfalls neu beginnen
        assert_eq!(
            due(daily, &st("hourly", "2026-10-17 14:00"), now),
            Due::Start(t("2026-10-17 00:05"))
        );
    }

    #[test]
    fn reads_when_term_reached_and_only_once() {
        let daily = Plan::parse("daily 00:05").unwrap();
        let s = st("daily 00:05", "2026-10-17 00:05");
        assert_eq!(due(daily, &s, t("2026-10-17 23:59")), Due::No);
        assert_eq!(due(daily, &s, t("2026-10-18 00:05")), Due::Read(t("2026-10-18 00:05")));
        let done = st("daily 00:05", "2026-10-18 00:05");
        assert_eq!(due(daily, &done, t("2026-10-18 00:06")), Due::No);
    }

    #[test]
    fn missed_terms_are_caught_up_once() {
        // über Nacht aus, 3 Tage verpasst: einmal lesen, Termin = letzter fälliger
        let daily = Plan::parse("daily 00:05").unwrap();
        let s = st("daily 00:05", "2026-10-14 00:05");
        assert_eq!(due(daily, &s, t("2026-10-17 08:00")), Due::Read(t("2026-10-17 00:05")));
        let q = st("15min", "2026-10-17 06:00");
        assert_eq!(due(Plan::Quarter, &q, t("2026-10-17 08:07")), Due::Read(t("2026-10-17 08:00")));
    }

    #[test]
    fn paused_meter_waits_for_next_day() {
        let mut s = st("hourly", "2026-10-17 10:00");
        s.pause = Some(pause_until(t("2026-10-17 10:00")));
        assert_eq!(due(Plan::Hourly, &s, t("2026-10-17 23:00")), Due::No);
        assert_eq!(due(Plan::Hourly, &s, t("2026-10-18 00:00")), Due::Read(t("2026-10-18 00:00")));
    }

    #[test]
    fn clock_jumps() {
        // Sommerzeit-Ende: eine Stunde zurück, kein doppelter Termin
        let s = st("15min", "2026-10-25 02:45");
        assert_eq!(due(Plan::Quarter, &s, t("2026-10-25 02:05")), Due::No);
        assert_eq!(due(Plan::Quarter, &s, t("2026-10-25 02:50")), Due::No);
        assert_eq!(due(Plan::Quarter, &s, t("2026-10-25 03:00")), Due::Read(t("2026-10-25 03:00")));
        // Uhr weit zurück (z.B. falsche Zeit korrigiert): Termine neu ausrichten statt warten
        let s = st("hourly", "2026-10-17 12:00");
        assert_eq!(due(Plan::Hourly, &s, t("2026-10-16 08:30")), Due::Start(t("2026-10-16 08:00")));
    }

    #[test]
    fn turned_off_forgets_state() {
        assert_eq!(due(Plan::Off, &st("hourly", "2026-10-17 12:00"), t("2026-10-17 13:00")), Due::Reset);
        assert_eq!(due(Plan::Off, &State::default(), t("2026-10-17 13:00")), Due::No);
    }

    #[test]
    fn inheritance() {
        assert_eq!(effective(None, None, "off"), (Plan::Off, "project"));
        assert_eq!(effective(None, Some("hourly"), "off"), (Plan::Hourly, "line"));
        assert_eq!(effective(Some("off"), Some("hourly"), "daily 00:05"), (Plan::Off, "meter"));
        assert_eq!(effective(None, None, "15min"), (Plan::Quarter, "project"));
    }

    #[test]
    fn load() {
        // 250 Zähler alle 15 min bei 300 Bd passt nicht
        let slow = estimate_ms(300);
        assert!(bus_load((0..250).map(|_| (Plan::Quarter, slow))) > 1.0);
        // 250 Zähler täglich bei 2400 Bd ist nichts
        let fast = estimate_ms(2400);
        assert!(bus_load((0..250).map(|_| (Plan::parse("daily 00:05").unwrap(), fast))) < 0.01);
    }
}
