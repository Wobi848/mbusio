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

use anyhow::{bail, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::i18n::{self, tr, Lang};

mod client;
use client::Session;

/// Warteschlange: älter wird verworfen, mehr auch (älteste zuerst)
const QUEUE_MAX_AGE_S: i64 = 7 * 86400;
const QUEUE_MAX_ROWS: i64 = 200_000;
const KEEPALIVE: u16 = 60;

// ---------------------------------------------------------------- Einstellungen

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqttConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
    /// Zertifikat des Brokers nicht prüfen (selbst signiert, nur im geschützten Netz)
    #[serde(default)]
    pub insecure: bool,
    /// eigenes CA-Zertifikat (PEM); leer = übliche öffentliche Zertifizierungsstellen
    #[serde(default)]
    pub ca: String,
    #[serde(default)]
    pub username: String,
    /// wird nie an die Oberfläche, ins Protokoll oder in einen Export gegeben
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_base")]
    pub base: String,
    /// Uhrzeit vom Leitsystem über `<basis>/time` übernehmen
    #[serde(default)]
    pub time_sync: bool,
    /// Muster für die Topics der Datenpunkte (relativ zum Basis-Topic), siehe `render_topic`
    #[serde(default = "default_topic")]
    pub topic: String,
}

fn default_port() -> u16 {
    1883
}

fn default_base() -> String {
    "mbusio".into()
}

/// bisheriger fester Aufbau: Projekt / Gateway-Linie / Zähler / Grösse
pub const DEFAULT_TOPIC: &str = "{projekt}/{gateway}-{linie}/{zaehler}/{wert}";

fn default_topic() -> String {
    DEFAULT_TOPIC.into()
}

/// Platzhalter im Topic-Muster
pub const TOPIC_VARS: [&str; 10] = [
    "projekt", "gateway", "linie", "zaehler", "id", "pa", "ort", "medium", "wert", "nr",
];

/// Topic aus Muster und Werten: jeder Wert für Topics lesbar gemacht (slug), leere Teile und
/// überzählige Trenner fallen weg. `vars`: (Platzhalter ohne Klammern, Wert)
pub fn render_topic(pattern: &str, vars: &[(&str, String)]) -> String {
    let mut s = pattern.trim().to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{k}}}"), &slug(v));
    }
    s.split('/')
        .map(|part| {
            let mut p = String::new();
            for c in part.chars() {
                if !(c == '-' && (p.is_empty() || p.ends_with('-'))) {
                    p.push(c);
                }
            }
            p.trim_end_matches('-').to_string()
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

impl Default for MqttConfig {
    fn default() -> Self {
        MqttConfig {
            enabled: false,
            host: String::new(),
            port: 1883,
            tls: false,
            insecure: false,
            ca: String::new(),
            username: String::new(),
            password: String::new(),
            client_id: String::new(),
            base: default_base(),
            time_sync: false,
            topic: default_topic(),
        }
    }
}

impl MqttConfig {
    pub fn check(&self) -> Result<()> {
        if self.enabled && self.host.trim().is_empty() {
            bail!(tr("Broker-Adresse fehlt", "Broker address missing"));
        }
        if self.base.trim().is_empty()
            || self.base.contains(['+', '#'])
            || self.base.starts_with('/')
        {
            bail!(tr(
                "Basis-Topic ungültig (nicht leer, ohne + und #, nicht mit / beginnen)",
                "Invalid base topic (not empty, no + or #, must not start with /)"
            ));
        }
        // Muster: nur bekannte Platzhalter, ohne + und #, mit {wert} oder {nr} (sonst wären die
        // Topics der Werte eines Zählers gleich)
        let p = &self.topic;
        let unknown = p
            .split('{')
            .skip(1)
            .filter_map(|x| x.split_once('}').map(|(k, _)| k))
            .find(|k| !TOPIC_VARS.contains(k));
        if p.trim().is_empty()
            || p.contains(['+', '#'])
            || !(p.contains("{wert}") || p.contains("{nr}"))
        {
            bail!(tr(
                "Topic-Muster ungültig (ohne + und #, mit {wert} oder {nr})",
                "Invalid topic pattern (no + or #, must contain {wert} or {nr})"
            ));
        }
        if let Some(k) = unknown {
            bail!(
                "{}: {{{k}}}",
                tr(
                    "Unbekannter Platzhalter im Topic-Muster",
                    "Unknown placeholder in topic pattern"
                )
            );
        }
        Ok(())
    }

    fn client_id(&self) -> String {
        if !self.client_id.trim().is_empty() {
            return self.client_id.trim().to_string();
        }
        let host = std::fs::read_to_string("/etc/hostname")
            .ok()
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .unwrap_or_default();
        format!("mbusio-{}", slug(host.trim()))
            .trim_end_matches('-')
            .to_string()
    }

    fn topic(&self, rel: &str) -> String {
        format!("{}/{rel}", self.base.trim().trim_end_matches('/'))
    }
}

/// Teil eines Topics aus einem Namen: klein, ä → ae, nur a-z 0-9 und Bindestrich
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            'é' | 'è' | 'ê' => out.push('e'),
            'à' | 'â' => out.push('a'),
            c if c.is_ascii_alphanumeric() || c == '_' => out.push(c),
            _ => {
                if !out.ends_with('-') {
                    out.push('-')
                }
            }
        }
    }
    out.trim_matches('-').to_string()
}

/// Topic (relativ zum Basis-Topic) prüfen: nicht leer, keine Platzhalter, keine leeren Teile
pub fn check_topic(rel: &str) -> Result<()> {
    if rel.is_empty()
        || rel.contains(['+', '#'])
        || rel.starts_with('/')
        || rel.ends_with('/')
        || rel.contains("//")
    {
        bail!("{}: „{rel}“", tr("Topic ungültig", "Invalid topic"));
    }
    Ok(())
}

// ---------------------------------------------------------------- Datenbank

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mqtt_queue (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    payload TEXT NOT NULL,
    created INTEGER NOT NULL
);
";

impl Db {
    pub fn mqtt_config(&self) -> MqttConfig {
        self.meta("mqtt")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    pub fn set_mqtt_config(&mut self, c: &MqttConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mqtt", &serde_json::to_string(c)?)?;
        Ok(())
    }

    /// Nachricht für den Broker vormerken (Topic relativ zum Basis-Topic)
    pub(crate) fn mqtt_enqueue(&mut self, topic: &str, payload: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mqtt_queue (topic, payload, created) VALUES (?1, ?2, ?3)",
            params![topic, payload, crate::clock::utc_secs()],
        )?;
        Ok(())
    }

    pub(crate) fn mqtt_peek(&self, n: usize) -> Result<Vec<(i64, String, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, topic, payload FROM mqtt_queue ORDER BY id LIMIT ?1")?;
        let rows = st.query_map([n as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn mqtt_done(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM mqtt_queue WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn mqtt_queued(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM mqtt_queue", [], |r| r.get(0))?)
    }

    /// Warteschlange begrenzen; liefert die Anzahl verworfener Nachrichten
    pub fn mqtt_trim(&mut self) -> Result<usize> {
        let old = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE created < ?1",
            [crate::clock::utc_secs() - QUEUE_MAX_AGE_S],
        )?;
        let over = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE id <= (SELECT MAX(id) FROM mqtt_queue) - ?1",
            [QUEUE_MAX_ROWS],
        )?;
        Ok(old + over)
    }
}

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
    i18n::set(Lang::De);
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
        format!(
            "MQTT {} {}:{}",
            tr("verbunden mit", "connected to"),
            cfg.host.trim(),
            cfg.port
        ),
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
        for (id, topic, payload) in batch {
            s.publish(&cfg.topic(&topic), payload.as_bytes())?;
            st.db.lock().unwrap().mqtt_done(id)?;
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
                                    format!(
                                        "{} ({:+.0} s)",
                                        tr(
                                            "Uhrzeit vom Leitsystem (MQTT) übernommen",
                                            "Time taken from control system (MQTT)"
                                        ),
                                        off
                                    ),
                                );
                            }
                        }
                    }
                    None => st.log.push(
                        "INFO",
                        format!(
                            "MQTT {}: {}",
                            topic,
                            tr("Uhrzeit unlesbar", "time unreadable")
                        ),
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
