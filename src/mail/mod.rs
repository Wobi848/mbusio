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

impl Db {
    pub fn mail_config(&self) -> MailConfig {
        self.meta_json("mail")
    }

    pub fn set_mail_config(&mut self, c: &MailConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mail", &serde_json::to_string(c)?)
    }
}

#[cfg(test)]
pub(crate) mod tests;
