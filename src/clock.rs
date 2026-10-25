//! Uhrzeit von mbusio: Systemzeit, ob sie bestätigt ist, und Korrektur aus dem Browser.
//!
//! Ein Raspberry Pi ohne Uhr-Modul startet nach einem Stromausfall mit einer plausiblen, aber
//! falschen Zeit. Deshalb merkt sich mbusio, ob und woher die Zeit bestätigt ist; solange nicht,
//! werden Auslesungen als „Zeit unsicher“ gekennzeichnet (Konzept Dauerbetrieb, 4.6).
//!
//! „Uhrzeit vom Gerät übernehmen“ stellt nicht die Systemuhr (dafür bräuchte es Root-Rechte),
//! sondern rechnet mit einer Abweichung zur Systemzeit weiter.

use std::sync::Mutex;

use std::time::Instant;

use chrono::{DateTime, Local, NaiveDateTime, TimeDelta};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct TimeState {
    /// Uhrzeit von einer Quelle bestätigt
    pub confirmed: bool,
    /// "ntp", "rtc", "system" (Windows/macOS), "browser"; None = keine
    pub source: Option<String>,
    /// wann zuletzt bestätigt (Ortszeit)
    pub since: Option<String>,
    /// Korrektur gegenüber der Systemuhr in Sekunden (aus dem Browser)
    pub offset_s: f64,
}

struct Clock {
    state: TimeState,
    offset: TimeDelta,
    /// Systemuhr nie als bestätigt ansehen (Test, oder wenn man ihr nicht traut)
    distrust_system: bool,
    /// Zeitraffer für Dauertests: so viel schneller läuft die Uhr ab dem Start
    scale: Option<(f64, DateTime<Local>, Instant)>,
}

static CLOCK: Mutex<Clock> = Mutex::new(Clock {
    state: TimeState {
        confirmed: false,
        source: None,
        since: None,
        offset_s: 0.0,
    },
    offset: TimeDelta::zero(),
    distrust_system: false,
    scale: None,
});

fn now_local() -> DateTime<Local> {
    let c = CLOCK.lock().unwrap();
    let base = match c.scale {
        Some((f, wall, mono)) => {
            wall + TimeDelta::milliseconds((mono.elapsed().as_secs_f64() * f * 1000.0) as i64)
        }
        None => Local::now(),
    };
    base + c.offset
}

/// Jetzt, Ortszeit, mit Korrektur
pub fn now() -> NaiveDateTime {
    now_local().naive_local()
}

/// Jetzt in UTC, Sekunden seit 1970
pub fn utc_secs() -> i64 {
    now_local().timestamp()
}

/// Zeitraffer (nur für Tests): die Uhr läuft `factor`-mal schneller
pub fn set_scale(factor: f64) {
    CLOCK.lock().unwrap().scale = Some((factor, Local::now(), Instant::now()));
}

/// Faktor des Zeitraffers, 1 = Echtzeit
pub fn scale() -> f64 {
    CLOCK.lock().unwrap().scale.map_or(1.0, |s| s.0)
}

/// Zeitstempel wie in der Datenbank
pub fn stamp() -> String {
    now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn confirmed() -> bool {
    CLOCK.lock().unwrap().state.confirmed
}

pub fn state() -> TimeState {
    CLOCK.lock().unwrap().state.clone()
}

pub fn distrust_system() {
    CLOCK.lock().unwrap().distrust_system = true;
}

/// Bestätigt die Systemuhr die Zeit? Linux: Zeitserver (NTP) synchronisiert oder beim Start aus
/// einem Uhr-Modul gestellt. Windows und macOS: PCs haben eine Uhr mit Batterie.
fn system_source() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        let ntp = std::process::Command::new("timedatectl")
            .args(["show", "-p", "NTPSynchronized", "--value"])
            .output()
            .ok()
            .is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes");
        if ntp {
            return Some("ntp");
        }
        let rtc =
            std::fs::read_to_string("/sys/class/rtc/rtc0/hctosys").is_ok_and(|s| s.trim() == "1");
        rtc.then_some("rtc")
    } else {
        Some("system")
    }
}

/// Quellen des Systems; sie haben Vorrang vor Browser und Leitsystem
const SYSTEM_SOURCES: [&str; 3] = ["ntp", "rtc", "system"];

impl Clock {
    fn system_confirmed(&self) -> bool {
        self.state
            .source
            .as_deref()
            .is_some_and(|s| SYSTEM_SOURCES.contains(&s))
    }

    /// Zeitquelle des Systems übernehmen (`None` = keine); `now`: Systemzeit. Liefert eine
    /// Meldung fürs Protokoll, wenn sich etwas geändert hat.
    fn system(&mut self, source: Option<&str>, now: DateTime<Local>) -> Option<String> {
        match source {
            Some(src) if self.state.source.as_deref() != Some(src) => {
                // eine echte Zeitquelle hat Vorrang vor der Korrektur aus dem Browser
                self.offset = TimeDelta::zero();
                self.state = TimeState {
                    confirmed: true,
                    source: Some(src.into()),
                    since: Some(now.format("%Y-%m-%d %H:%M:%S").to_string()),
                    offset_s: 0.0,
                };
                Some(format!("Uhrzeit bestätigt ({src})"))
            }
            None if self.system_confirmed() => {
                self.state.confirmed = false;
                self.state.source = None;
                Some("Uhrzeit nicht mehr bestätigt".into())
            }
            _ => None,
        }
    }

    /// Uhrzeit `at` aus einer Quelle ("browser", "mqtt") übernehmen; `now`: Systemzeit. Eine
    /// Quelle des Systems hat Vorrang und bleibt. Liefert die Korrektur in Sekunden.
    fn external(&mut self, source: &str, at: DateTime<Local>, now: DateTime<Local>) -> f64 {
        if self.system_confirmed() {
            return 0.0;
        }
        let offset = at.naive_local() - now.naive_local();
        // unter 2 s ist Messungenauigkeit (Laufzeit der Anfrage): Systemzeit gilt dann als richtig
        let offset = if offset.num_milliseconds().abs() < 2000 {
            TimeDelta::zero()
        } else {
            offset
        };
        let secs = offset.num_milliseconds() as f64 / 1000.0;
        self.offset = offset;
        self.state = TimeState {
            confirmed: true,
            source: Some(source.into()),
            since: Some(at.format("%Y-%m-%d %H:%M:%S").to_string()),
            offset_s: secs,
        };
        secs
    }
}

/// Zeitquelle prüfen (beim Start und danach regelmässig). Liefert eine Meldung fürs Protokoll,
/// wenn sich etwas geändert hat.
pub fn check_system() -> Option<String> {
    let source = if CLOCK.lock().unwrap().distrust_system {
        None
    } else {
        system_source()
    };
    CLOCK.lock().unwrap().system(source, Local::now())
}

/// Uhrzeit vom Gerät des Benutzers übernehmen (Millisekunden seit 1970, UTC).
/// Liefert die Korrektur in Sekunden.
pub fn set_from_browser(epoch_ms: i64) -> Option<f64> {
    set_from("browser", epoch_ms)
}

/// Uhrzeit aus einer Quelle übernehmen ("browser", "mqtt"); eine echte Systemquelle (NTP,
/// Uhr-Modul) hat Vorrang und wird nicht überschrieben
pub fn set_from(source: &str, epoch_ms: i64) -> Option<f64> {
    let at = chrono::DateTime::from_timestamp_millis(epoch_ms)?.with_timezone(&Local);
    Some(CLOCK.lock().unwrap().external(source, at, Local::now()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> Clock {
        Clock {
            state: TimeState::default(),
            offset: TimeDelta::zero(),
            distrust_system: false,
            scale: None,
        }
    }

    fn at(s: &str) -> DateTime<Local> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
    }

    #[test]
    fn browser_time_corrects_the_clock_unless_it_is_close() {
        let now = at("2026-10-25 12:00:00");
        // Pi nach Stromausfall: 3 Stunden zurück; Browser weiss es besser
        let mut c = clock();
        assert_eq!(
            c.external("browser", at("2026-10-25 15:00:00"), now),
            10800.0
        );
        assert_eq!(c.offset, TimeDelta::hours(3));
        assert!(c.state.confirmed);
        assert_eq!(c.state.source.as_deref(), Some("browser"));
        // unter 2 s: Laufzeit der Anfrage, Systemzeit gilt als richtig (aber bestätigt)
        let mut c = clock();
        assert_eq!(
            c.external("mqtt", now + TimeDelta::milliseconds(1500), now),
            0.0
        );
        assert_eq!(c.offset, TimeDelta::zero());
        assert!(c.state.confirmed);
    }

    #[test]
    fn system_source_wins_and_losing_it_unconfirms() {
        let now = at("2026-10-25 12:00:00");
        let mut c = clock();
        c.external("browser", at("2026-10-25 12:10:00"), now);
        // NTP kommt: Korrektur aus dem Browser fällt weg
        assert_eq!(
            c.system(Some("ntp"), now).as_deref(),
            Some("Uhrzeit bestätigt (ntp)")
        );
        assert_eq!(c.offset, TimeDelta::zero());
        // danach ändert Browser/Leitsystem nichts mehr
        assert_eq!(c.external("mqtt", at("2026-10-25 13:00:00"), now), 0.0);
        assert_eq!(c.state.source.as_deref(), Some("ntp"));
        // gleiche Quelle: keine neue Meldung
        assert_eq!(c.system(Some("ntp"), now), None);
        // NTP weg: nicht mehr bestätigt
        assert_eq!(
            c.system(None, now).as_deref(),
            Some("Uhrzeit nicht mehr bestätigt")
        );
        assert!(!c.state.confirmed);
        // eine Browser-Zeit bleibt bestehen, wenn das System keine Quelle hat
        c.external("browser", at("2026-10-25 12:10:00"), now);
        assert_eq!(c.system(None, now), None);
        assert!(c.state.confirmed);
    }
}
