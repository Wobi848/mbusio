//! Abgelegte Dateien per E-Mail (Konzept Export, Teil C): was zu senden ist, Nachricht bauen,
//! Ergebnis je Datei festhalten. Gesendet wird ausserhalb (ohne die Datenbank zu sperren).

use std::path::Path;

use chrono::NaiveDateTime;

use super::auto::{export_dir, export_files, ExportFile};
use super::*;
use crate::i18n::{both, Text};
use crate::mail::{self, Attachment, MailConfig, Message};

/// Zeitraum aus dem Namen einer abgelegten Datei: …-JJJJ-MM-TT-JJJJ-MM-TT.csv
fn period_of_name(name: &str) -> Option<(NaiveDate, NaiveDate)> {
    let stem = name.strip_suffix(".csv")?;
    let day = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    let n = stem.len();
    Some((day(stem.get(n - 21..n - 11)?)?, day(stem.get(n - 10..)?)?))
}

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

    /// Betreff nach den Einstellungen des Projekts (Platzhalter siehe Konzept E-Mail 9)
    pub fn billing_subject(
        &self,
        project_id: i64,
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<String> {
        let template = if s.mail_subject.trim().is_empty() {
            default_subject()
        } else {
            s.mail_subject.clone()
        };
        let project = self.project(project_id)?.name;
        let (von, bis) = (
            from.format("%d.%m.%Y").to_string(),
            to.format("%d.%m.%Y").to_string(),
        );
        Ok(mail::fill(
            &template,
            &[
                ("projekt", &project),
                ("zeitraum", &s.label(from, to)),
                ("von", &von),
                ("bis", &bis),
                ("geraet", &self.mail_config().device_name()),
            ],
        ))
    }

    /// Nachricht mit einer Datei an die Empfänger des Projekts
    fn billing_message(
        &self,
        project_id: i64,
        name: &str,
        data: Vec<u8>,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Outgoing> {
        let s = self.billing_settings(project_id);
        let recipients = mail::recipients(&s.mail_to)?;
        if recipients.is_empty() {
            bail!(tr("Keine Empfänger eingetragen", "No recipients entered"));
        }
        let config = self.mail_config();
        let message = Message {
            from: config.ready()?,
            to: recipients,
            subject: self.billing_subject(project_id, &s, from, to)?,
            text: format!(
                "{}: {name}\r\n{}\r\n\r\n{} {}",
                tr("Im Anhang", "Attached"),
                tr(
                    "CSV für Excel, Spalten mit Semikolon getrennt.",
                    "CSV for Excel, columns separated by semicolons."
                ),
                tr("Gesendet von mbusio", "Sent by mbusio"),
                config.device_name()
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

    /// Eine abgelegte Datei als Nachricht; der Zeitraum steht im Namen (…-von-bis.csv)
    pub fn billing_outgoing(
        &self,
        data_dir: &Path,
        project_id: i64,
        name: &str,
    ) -> Result<Outgoing> {
        let data = std::fs::read(export_dir(data_dir, project_id).join(name))?;
        let (from, to) = period_of_name(name).ok_or_else(|| {
            anyhow::anyhow!("{}: {name}", tr("Zeitraum unklar", "unclear period"))
        })?;
        self.billing_message(project_id, name, data, from, to)
    }

    /// Die Abrechnung mit den gespeicherten Einstellungen jetzt als Nachricht (von Hand)
    pub fn billing_mail_now(&self, project_id: i64, today: NaiveDate) -> Result<Outgoing> {
        let s = self.billing_settings(project_id);
        let (from, to) = s.range(today)?;
        let (name, data) = self.billing_csv_file(project_id, &s, from, to)?;
        self.billing_message(project_id, &name, data.into_bytes(), from, to)
    }

    /// Zu sendende Dateien: automatischer Export mit Empfängern, Datei aus den letzten Tagen und
    /// noch nicht gesendet (neu oder fehlgeschlagen); dazu Meldungen, wo keine Nachricht entsteht
    pub fn billing_mail_pending(
        &self,
        data_dir: &Path,
        now: NaiveDateTime,
    ) -> Result<(Vec<Outgoing>, Vec<Text>)> {
        let since = (now - Duration::days(self.mail_config().retry_days))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let mut out = Vec::new();
        let mut notes = Vec::new();
        // ausgeschaltet: nichts versuchen, die Dateien warten (bis zu den Tagen für erneute Versuche)
        if !self.mail_config().enabled {
            return Ok((out, notes));
        }
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

    /// Projekte, deren abgelegte Datei nicht gesendet werden konnte (in der Zeit für neue
    /// Versuche), mit dem Grund der neuesten
    pub fn billing_unsent(
        &self,
        data_dir: &Path,
        now: NaiveDateTime,
    ) -> Result<Vec<(i64, String)>> {
        let since = (now - Duration::days(self.mail_config().retry_days))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let mut out = Vec::new();
        for (id, _) in self.billing_auto_projects()? {
            let states = self.mail_states(id);
            let why = export_files(data_dir, id)?
                .into_iter()
                .filter(|f| f.time >= since)
                .find_map(|f| {
                    states
                        .get(&f.name)
                        .filter(|m| m.sent.is_none())?
                        .error
                        .clone()
                });
            if let Some(why) = why {
                out.push((id, why));
            }
        }
        Ok(out)
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
