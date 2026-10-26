//! MQTT-Ausgang (Konzept Dauerbetrieb, Phase 3): gibt Datenpunkte an einen Broker weiter.
//!
//! - Ein Topic je Datenpunkt unter einem Basis-Topic, Nutzlast JSON
//!   `{"value", "unit", "time", "quality", "name", "no"}`, retained, QoS 1.
//! - `<basis>/status` = `online`, als „Last Will“ `offline`: der Empfänger erkennt, wenn mbusio weg ist.
//! - **Zwischenspeicher:** alles geht zuerst in eine Warteschlange in der Datenbank und wird erst
//!   gelöscht, wenn der Broker den Empfang bestätigt hat (PUBACK). Ist der Broker weg, wird
//!   gesammelt (höchstens 7 Tage) und danach in richtiger Reihenfolge nachgeliefert, auch über
//!   einen Neustart hinweg.
//! - Optional die Uhrzeit vom Leitsystem: Nachricht auf `<basis>/time` (Sekunden oder ms seit 1970,
//!   oder RFC 3339).
//! - Ein Broker, der nicht geht, beeinflusst das Auslesen nicht (eigener Thread).
//!
//! Eigener kleiner Client für MQTT 3.1.1 (nur was gebraucht wird), damit die Zustellung genau
//! kontrolliert ist: veröffentlichen, auf die Bestätigung warten, dann erst aus der Warteschlange.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

use crate::i18n::{both, tr};

mod client;
mod config;
mod store;

use client::Session;
pub use config::*;
pub use store::SCHEMA;

const KEEPALIVE: u16 = 60;

// ---------------------------------------------------------------- Dienst

#[derive(Debug, Clone, Default, Serialize)]
pub struct MqttStatus {
    /// "off", "connecting", "connected", "error"
    pub state: String,
    pub broker: String,
    pub error: Option<String>,
    pub since: Option<String>,
    /// seit dem Start gesendet
    pub sent: u64,
    pub queued: i64,
}

#[derive(Default)]
pub struct MqttState {
    pub status: Mutex<MqttStatus>,
    /// erhöht bei jeder Änderung der Einstellungen: neu verbinden
    pub version: AtomicU64,
}

/// Uhrzeit aus einer Nachricht: Sekunden oder Millisekunden seit 1970, oder RFC 3339
pub fn parse_time(payload: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(payload).ok()?.trim().trim_matches('"');
    if let Ok(n) = s.parse::<f64>() {
        return Some(if n > 1e11 {
            n as i64
        } else {
            (n * 1000.0) as i64
        });
    }
    Some(
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()?
            .timestamp_millis(),
    )
}

fn set_status(st: &crate::web::AppState, f: impl FnOnce(&mut MqttStatus)) {
    let mut s = st.mqtt.status.lock().unwrap();
    f(&mut s);
}

/// Läuft im eigenen Thread, solange mbusio läuft
pub fn run(st: crate::web::AppState) {
    let mut backoff = Duration::from_secs(2);
    let mut last_error = String::new();
    loop {
        let version = st.mqtt.version.load(Ordering::SeqCst);
        let cfg = st.db.lock().unwrap().mqtt_config();
        if !cfg.enabled || cfg.host.trim().is_empty() {
            set_status(&st, |s| {
                *s = MqttStatus {
                    state: "off".into(),
                    ..Default::default()
                }
            });
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let broker = format!("{}:{}", cfg.host.trim(), cfg.port);
        set_status(&st, |s| {
            s.state = "connecting".into();
            s.broker = broker.clone();
        });
        let result = Session::connect(&cfg).and_then(|s| session(&st, &cfg, s, version));
        if let Err(e) = result {
            let msg = format!("{e:#}");
            if msg != last_error {
                st.log.push("INFO", format!("MQTT {broker}: {msg}"));
                last_error = msg.clone();
            }
            set_status(&st, |s| {
                s.state = "error".into();
                s.error = Some(msg);
                s.since = None;
            });
            // warten, aber bei geänderten Einstellungen sofort neu versuchen
            let until = Instant::now() + backoff;
            while Instant::now() < until && st.mqtt.version.load(Ordering::SeqCst) == version {
                std::thread::sleep(Duration::from_millis(200));
            }
            backoff = (backoff * 2).min(Duration::from_secs(60));
        } else {
            backoff = Duration::from_secs(2);
            last_error.clear();
        }
    }
}

/// Verbunden: Warteschlange abarbeiten, Uhrzeit annehmen, bis die Einstellungen sich ändern
fn session(
    st: &crate::web::AppState,
    cfg: &MqttConfig,
    mut s: Session,
    version: u64,
) -> Result<()> {
    s.publish(&cfg.topic("status"), b"online")?;
    if cfg.time_sync {
        s.subscribe(&cfg.topic("time"))?;
    }
    st.log.push(
        "INFO",
        both(|| {
            format!(
                "MQTT {} {}:{}",
                tr("verbunden mit", "connected to"),
                cfg.host.trim(),
                cfg.port
            )
        }),
    );
    set_status(st, |x| {
        x.state = "connected".into();
        x.error = None;
        x.since = Some(crate::clock::stamp());
    });
    loop {
        if st.mqtt.version.load(Ordering::SeqCst) != version {
            // Einstellungen geändert: sauber abmelden (dann gilt der Last Will nicht)
            let _ = s.publish(&cfg.topic("status"), b"offline");
            s.disconnect();
            return Ok(());
        }
        let batch = st.db.lock().unwrap().mqtt_peek(50)?;
        if batch.is_empty() {
            s.idle(Duration::from_millis(500))?;
        }
        for (id, topic, payload, abs) in batch {
            let topic = if abs { topic } else { cfg.topic(&topic) };
            s.publish(&topic, payload.as_bytes())?;
            st.db.lock().unwrap().mqtt_done(id, &topic)?;
            set_status(st, |x| x.sent += 1);
        }
        for (topic, payload) in std::mem::take(&mut s.inbox) {
            if topic == cfg.topic("time") {
                match parse_time(&payload) {
                    Some(ms) => {
                        let was = crate::clock::state();
                        if let Some(off) = crate::clock::set_from("mqtt", ms) {
                            if was.source.as_deref() != Some("mqtt")
                                || (off - was.offset_s).abs() >= 2.0
                            {
                                st.log.push(
                                    "INFO",
                                    both(|| {
                                        format!(
                                            "{} ({:+.0} s)",
                                            tr(
                                                "Uhrzeit vom Leitsystem (MQTT) übernommen",
                                                "Time taken from control system (MQTT)"
                                            ),
                                            off
                                        )
                                    }),
                                );
                            }
                        }
                    }
                    None => st.log.push(
                        "INFO",
                        both(|| {
                            format!(
                                "MQTT {}: {}",
                                topic,
                                tr("Uhrzeit unlesbar", "time unreadable")
                            )
                        }),
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_topics() {
        assert_eq!(slug("MFH Seestrasse 12"), "mfh-seestrasse-12");
        assert_eq!(slug("Wärme · Rücklauf (Whg 3)"), "waerme-ruecklauf-whg-3");
        assert!(check_topic("a/b/c").is_ok());
        assert!(check_topic("a/+/c").is_err());
        assert!(check_topic("/a").is_err());
        assert!(check_topic("a//b").is_err());
    }

    #[test]
    fn time_payloads() {
        assert_eq!(parse_time(b"1760000000"), Some(1_760_000_000_000));
        assert_eq!(parse_time(b"1760000000123"), Some(1_760_000_000_123));
        assert_eq!(
            parse_time(b"\"2026-10-17T12:00:00Z\""),
            Some(1_792_238_400_000)
        );
        assert_eq!(parse_time(b"morgen"), None);
    }
}
