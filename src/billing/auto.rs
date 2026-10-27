//! Automatischer Export (Konzept Export 3.5): nach Ende des Zeitraums (Vormonat bzw. Vorjahr) die
//! Datei nach den Einstellungen des Projekts in `exporte/<projekt>/` im Datenordner schreiben.
//! Der zuletzt exportierte Zeitraum steht in `meta` (`billing_done:<projekt>`); was fehlt, weil
//! das Gerät aus war, wird nachgeholt.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use super::*;
use crate::i18n::{both, Text};

/// Höchstens so viele verpasste Zeiträume nachholen
const CATCH_UP: usize = 12;

/// Eine Datei des automatischen Exports
#[derive(Debug, Serialize)]
pub struct ExportFile {
    pub name: String,
    pub size: u64,
    /// geschrieben (Ortszeit, JJJJ-MM-TT hh:mm:ss)
    pub time: String,
}

fn done_key(project_id: i64) -> String {
    format!("billing_done:{project_id}")
}

/// Ordner der Dateien eines Projekts
pub fn export_dir(data_dir: &Path, project_id: i64) -> PathBuf {
    data_dir.join("exporte").join(project_id.to_string())
}

impl Db {
    /// Wieder eingeschaltet: nicht alle Zeiträume seit dem Ausschalten nachholen, nur den letzten
    pub(super) fn billing_auto_restart(&self, project_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM meta WHERE key = ?1", [done_key(project_id)])?;
        Ok(())
    }

    /// Projekte mit eingeschaltetem automatischem Export
    pub(super) fn billing_auto_projects(&self) -> Result<Vec<(i64, Settings)>> {
        let rows: Vec<(String, String)> = self
            .conn
            .prepare("SELECT key, value FROM meta WHERE key LIKE 'billing:%'")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(k, v)| {
                let id = k.strip_prefix("billing:")?.parse().ok()?;
                let s: Settings = serde_json::from_str(&v).ok()?;
                (s.auto && s.period != "custom").then_some((id, s))
            })
            .collect())
    }

    /// Fällige Zeiträume, ältester zuerst: fertig und noch nicht exportiert. Der letzte erst, wenn
    /// danach schon gelesen wurde (Stand zum Ende vollständig) oder nach `wait_days`.
    pub(crate) fn billing_due(
        &self,
        project_id: i64,
        s: &Settings,
        now: NaiveDateTime,
    ) -> Result<Vec<(NaiveDate, NaiveDate)>> {
        let done: Option<NaiveDate> = self
            .meta(&done_key(project_id))?
            .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
        let mut out = Vec::new();
        let mut day = now.date();
        while out.len() < CATCH_UP {
            let (from, to) = s.range(day)?;
            if done.is_some_and(|d| to <= d) {
                break;
            }
            out.push((from, to));
            // ohne frühere Exporte nur der letzte Zeitraum
            if done.is_none() {
                break;
            }
            day = from;
        }
        out.reverse();
        if let Some(&(_, to)) = out.last() {
            if now.date() < to + Duration::days(s.wait_days) && !self.read_after(project_id, to)? {
                out.pop();
            }
        }
        Ok(out)
    }

    /// Wurde im Projekt nach dem Tag schon gelesen?
    fn read_after(&self, project_id: i64, day: NaiveDate) -> Result<bool> {
        let after = stand::after_day(day);
        for (_, m) in self.meters_with_line_label(project_id)? {
            if self.meter_reading_count(m.id, &after, "9999")? > 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Fällige Dateien aller Projekte schreiben; Meldungen fürs Protokoll
    pub fn billing_auto(&self, data_dir: &Path, now: NaiveDateTime) -> Result<Vec<Text>> {
        let mut out = Vec::new();
        for (id, s) in self.billing_auto_projects()? {
            for (from, to) in self.billing_due(id, &s, now)? {
                let path = self.billing_write(data_dir, id, &s, from, to)?;
                self.set_meta(&done_key(id), &to.to_string())?;
                let name = path.display().to_string();
                out.push(both(|| {
                    format!(
                        "{}: {name}",
                        tr("Abrechnung exportiert", "Billing exported")
                    )
                }));
            }
        }
        Ok(out)
    }

    /// Datei für Excel: Name und Inhalt (UTF-8 mit BOM), wie abgelegt und gemailt
    pub(super) fn billing_csv_file(
        &self,
        project_id: i64,
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<(String, String)> {
        let t = self.billing_table_for(project_id, s, from, to, None)?;
        let what = if s.form == "series" {
            tr("verlauf", "series")
        } else {
            tr("abrechnung", "billing")
        };
        let project = crate::export::file_part(&self.project(project_id)?.name);
        Ok((
            format!("{project}-{what}-{from}-{to}.csv"),
            format!("\u{feff}{}", csv(&t)),
        ))
    }

    fn billing_write(
        &self,
        data_dir: &Path,
        project_id: i64,
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<PathBuf> {
        let (name, data) = self.billing_csv_file(project_id, s, from, to)?;
        let dir = export_dir(data_dir, project_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(name);
        std::fs::write(&path, data)?;
        Ok(path)
    }
}

/// Dateien des automatischen Exports eines Projekts, neueste zuerst
pub fn export_files(data_dir: &Path, project_id: i64) -> Result<Vec<ExportFile>> {
    let dir = export_dir(data_dir, project_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let meta = e.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let time: chrono::DateTime<chrono::Local> = meta.modified()?.into();
        out.push(ExportFile {
            name: e.file_name().to_string_lossy().into_owned(),
            size: meta.len(),
            time: time.format("%Y-%m-%d %H:%M:%S").to_string(),
        });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time).then(b.name.cmp(&a.name)));
    Ok(out)
}
