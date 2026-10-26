//! E-Mail (Konzept Export, Teil C): Mailserver einmal je Gerät, Nachricht mit Anhang, eigener
//! kleiner SMTP-Client (wie der eigene MQTT-Client: keine grosse Bibliothek).

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::i18n::tr;

mod message;
mod smtp;

pub use message::{base64, Attachment, Message};
pub use smtp::send;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MailConfig {
    /// aus = es geht keine Mail raus (Dateien werden trotzdem abgelegt); Testmail geht immer
    pub enabled: bool,
    /// unterscheidet mehrere Geräte im Posteingang (`{geraet}`); leer = Name des Rechners
    pub device: String,
    pub host: String,
    pub port: u16,
    /// "starttls" (587), "tls" (465) oder "none" (25)
    pub security: String,
    /// Zertifikat des Servers nicht prüfen (selbst signiert, nur im geschützten Netz)
    pub insecure: bool,
    pub username: String,
    /// wird nie an die Oberfläche, ins Protokoll oder in einen Export gegeben
    pub password: String,
    /// Absender; leer = Benutzer
    pub from: String,
}

impl Default for MailConfig {
    fn default() -> Self {
        MailConfig {
            enabled: true,
            device: String::new(),
            host: String::new(),
            port: 587,
            security: "starttls".into(),
            insecure: false,
            username: String::new(),
            password: String::new(),
            from: String::new(),
        }
    }
}

impl MailConfig {
    pub fn check(&self) -> Result<()> {
        if !["starttls", "tls", "none"].contains(&self.security.as_str()) {
            bail!(tr("Verschlüsselung unbekannt", "Unknown encryption"));
        }
        if !self.from.is_empty() && !is_address(&self.from) {
            bail!(tr(
                "Absender ist keine E-Mail-Adresse",
                "Sender is not an e-mail address"
            ));
        }
        Ok(())
    }

    /// Gerätename für Betreff und Text: eingestellt oder der Name des Rechners
    pub fn device_name(&self) -> String {
        if !self.device.trim().is_empty() {
            return self.device.trim().to_string();
        }
        std::env::var("COMPUTERNAME")
            .ok()
            .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "mbusio".into())
    }

    /// Bereit zum Senden (eingeschaltet, Server und Absender da); sonst warum nicht
    pub fn ready(&self) -> Result<String> {
        if !self.enabled {
            bail!(tr(
                "E-Mail ist ausgeschaltet (Weitergabe → E-Mail)",
                "E-mail is switched off (Pass on → E-mail)"
            ));
        }
        self.sender()
    }

    /// Absender: eigener oder der Benutzer, wenn er eine Adresse ist
    pub fn sender(&self) -> Result<String> {
        let from = if self.from.is_empty() {
            &self.username
        } else {
            &self.from
        };
        if self.host.trim().is_empty() {
            bail!(tr("E-Mail-Server fehlt", "E-mail server missing"));
        }
        if !is_address(from) {
            bail!(tr("Absender fehlt", "Sender missing"));
        }
        Ok(from.trim().to_string())
    }
}

/// Grob: etwas@etwas.etwas, ohne Leerzeichen und spitze Klammern (gegen Kopfzeilen-Tricks)
pub fn is_address(s: &str) -> bool {
    let s = s.trim();
    let Some((user, domain)) = s.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.contains(|c: char| c.is_whitespace() || "<>,;\"".contains(c))
}

/// Empfänger aus einem Feld „a@x.de, b@y.de“ (Komma oder Semikolon); falsche Adresse: Fehler
pub fn recipients(field: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for a in field
        .split([',', ';'])
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        if !is_address(a) {
            bail!(
                "{}: {a}",
                tr("keine E-Mail-Adresse", "not an e-mail address")
            );
        }
        out.push(a.to_string());
    }
    Ok(out)
}

/// Letzter Versand, für die Karte des Ausgangs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LastMail {
    pub time: String,
    pub subject: String,
    /// None = gesendet
    pub error: Option<String>,
}

impl Db {
    pub fn mail_config(&self) -> MailConfig {
        self.meta_json("mail")
    }

    pub fn mail_last(&self) -> Option<LastMail> {
        self.meta("mail_last")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
    }

    /// Ergebnis eines Versands festhalten: im Protokoll der Weitergabe (Ausgang `email`) und als
    /// letzter Versand für die Karte. Jede Mail geht hier durch, egal wer sie schickt.
    pub fn mail_record(&mut self, m: &Message, result: &Result<()>) -> Result<()> {
        let error = result.as_ref().err().map(|e| format!("{e:#}"));
        let files: Vec<&str> = m.attachments.iter().map(|a| a.name.as_str()).collect();
        self.out_log(&crate::datapoint::OutLogEntry {
            output: "email",
            dp: None,
            name: Some(&m.subject),
            address: &m.to.join(", "),
            value: &files.join(", "),
            quality: "",
            result: if error.is_some() { "failed" } else { "sent" },
            reason: error.as_deref().unwrap_or(""),
        })?;
        let last = LastMail {
            time: crate::clock::stamp(),
            subject: m.subject.clone(),
            error,
        };
        self.set_meta("mail_last", &serde_json::to_string(&last)?)
    }

    pub fn set_mail_config(&mut self, c: &MailConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mail", &serde_json::to_string(c)?)
    }
}

#[cfg(test)]
pub(crate) mod tests;
