//! MQTT-Einstellungen und Topics: prüfen, Topic-Muster, Topic-Teile aus Namen.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

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
    /// Home Assistant: Konfig-Nachrichten für MQTT Discovery senden
    #[serde(default)]
    pub ha: bool,
    /// Präfix der Konfig-Topics für Home Assistant
    #[serde(default = "default_ha_prefix")]
    pub ha_prefix: String,
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

fn default_ha_prefix() -> String {
    "homeassistant".into()
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
            ha: false,
            ha_prefix: default_ha_prefix(),
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
        let prefix = self.ha_prefix.trim();
        if self.ha
            && (prefix.is_empty()
                || prefix.contains(['+', '#'])
                || prefix.starts_with('/')
                || prefix.ends_with('/'))
        {
            bail!(tr(
                "Präfix für Home Assistant ungültig (nicht leer, ohne + und #, ohne / am Anfang oder Ende)",
                "Invalid Home Assistant prefix (not empty, no + or #, no / at start or end)"
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

    pub fn client_id(&self) -> String {
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

    pub(super) fn topic(&self, rel: &str) -> String {
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
