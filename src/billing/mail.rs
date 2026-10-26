//! Abgelegte Dateien per E-Mail (Konzept Export, Teil C): was zu senden ist, Nachricht bauen,
//! Ergebnis je Datei festhalten. Gesendet wird ausserhalb (ohne die Datenbank zu sperren).

use std::path::Path;

use chrono::NaiveDateTime;

use super::auto::{export_dir, export_files, ExportFile};
use super::*;
use crate::i18n::{both, Text};
use crate::mail::{self, Attachment, MailConfig, Message};

/// Nicht gesendete Dateien so lange erneut versuchen
const RETRY_DAYS: i64 = 7;

/// Wann eine Datei gesendet wurde oder warum nicht
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MailState {
    /// gesendet (Ortszeit)
    pub sent: Option<String>,
    pub error: Option<String>,
}

/// Eine Nachricht, bereit zum Senden
pub struct Outgoing {
    pub project_id: i64,
    pub name: String,
    pub config: MailConfig,
    pub message: Message,
}

fn mail_key(project_id: i64) -> String {
    format!("billing_mail:{project_id}")
}

/// Datei mit Zustand der E-Mail, für die Liste
#[derive(Debug, Serialize)]
pub struct FileView {
    #[serde(flatten)]
    pub file: ExportFile,
    pub mail: Option<MailState>,
}

impl Db {
    fn mail_states(&self, project_id: i64) -> BTreeMap<String, MailState> {
        self.meta_json(&mail_key(project_id))
    }

    /// Dateien des automatischen Exports mit Zustand der E-Mail, neueste zuerst
    pub fn billing_files(&self, data_dir: &Path, project_id: i64) -> Result<Vec<FileView>> {
        let mut states = self.mail_states(project_id);
        Ok(export_files(data_dir, project_id)?
            .into_iter()
            .map(|f| FileView {
                mail: states.remove(&f.name),
                file: f,
            })
            .collect())
    }

    /// Eine abgelegte Datei als Nachricht an die Empfänger des Projekts
    pub fn billing_outgoing(
        &self,
        data_dir: &Path,
        project_id: i64,
        name: &str,
    ) -> Result<Outgoing> {
        let s = self.billing_settings(project_id);
        let to = mail::recipients(&s.mail_to)?;
        if to.is_empty() {
            bail!(tr("Keine Empfänger eingetragen", "No recipients entered"));
        }
        let config = self.mail_config();
        let project = self.project(project_id)?.name;
        let data = std::fs::read(export_dir(data_dir, project_id).join(name))?;
        let message = Message {
            from: config.sender()?,
            to,
            subject: format!("mbusio: {} {project}", tr("Abrechnung", "Billing")),
            text: format!(
                "{}: {name}\r\n{}\r\n\r\n{}",
                tr("Im Anhang", "Attached"),
                tr(
                    "CSV für Excel, Spalten mit Semikolon getrennt.",
                    "CSV for Excel, columns separated by semicolons."
                ),
                tr("Gesendet von mbusio", "Sent by mbusio")
            ),
            attachments: vec![Attachment {
                name: name.to_string(),
                mime: "text/csv; charset=utf-8".into(),
                data,
            }],
        };
        Ok(Outgoing {
            project_id,
            name: name.to_string(),
            config,
            message,
        })
    }

    /// Zu sendende Dateien: automatischer Export mit Empfängern, Datei aus den letzten Tagen und
    /// noch nicht gesendet (neu oder fehlgeschlagen); dazu Meldungen, wo keine Nachricht entsteht
    pub fn billing_mail_pending(
        &self,
        data_dir: &Path,
        now: NaiveDateTime,
    ) -> Result<(Vec<Outgoing>, Vec<Text>)> {
        let since = (now - Duration::days(RETRY_DAYS))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let mut out = Vec::new();
        let mut notes = Vec::new();
        for (id, s) in self.billing_auto_projects()? {
            if s.mail_to.trim().is_empty() {
                continue;
            }
            let states = self.mail_states(id);
            for f in export_files(data_dir, id)? {
                if f.time < since || states.get(&f.name).is_some_and(|m| m.sent.is_some()) {
                    continue;
                }
                match self.billing_outgoing(data_dir, id, &f.name) {
                    Ok(o) => out.push(o),
                    // Einstellungen unvollständig: wie ein Fehler beim Senden festhalten
                    Err(e) => notes.extend(self.billing_mail_done(id, &f.name, Err(e), now)?),
                }
            }
        }
        Ok((out, notes))
    }

    /// Ergebnis festhalten; Meldung fürs Protokoll nur beim Erfolg, beim ersten Fehler und wenn sich
    /// der Grund ändert (ein falsches Passwort soll das Protokoll nicht füllen)
    pub fn billing_mail_done(
        &self,
        project_id: i64,
        name: &str,
        result: Result<()>,
        now: NaiveDateTime,
    ) -> Result<Option<Text>> {
        let mut states = self.mail_states(project_id);
        let old = states.remove(name).unwrap_or_default();
        let (state, msg) = match result {
            Ok(()) => (
                MailState {
                    sent: Some(now.format("%Y-%m-%d %H:%M:%S").to_string()),
                    error: None,
                },
                Some(both(|| {
                    format!("{}: {name}", tr("E-Mail gesendet", "E-mail sent"))
                })),
            ),
            Err(e) => {
                let why = format!("{e:#}");
                let msg = (old.error.as_deref() != Some(why.as_str())).then(|| {
                    both(|| {
                        format!(
                            "{}: {name}: {why}",
                            tr("E-Mail nicht gesendet", "E-mail not sent")
                        )
                    })
                });
                (
                    MailState {
                        sent: None,
                        error: Some(why),
                    },
                    msg,
                )
            }
        };
        states.insert(name.to_string(), state);
        self.set_meta(&mail_key(project_id), &serde_json::to_string(&states)?)?;
        Ok(msg)
    }
}
