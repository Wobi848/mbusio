//! Ausdünnen und Speicherschutz (Konzept Dauerbetrieb 4.5): ältere Auslesungen und Werte auf einen
//! je Zähler bzw. Datenpunkt und Tag, und zwar den letzten (Stand am Tagesende, den die Abrechnung
//! braucht). Je Projekt bewusst eingeschaltet, oder für alle, wenn der Speicher knapp wird; danach
//! gibt die Datenbank den Platz frei (`auto_vacuum = INCREMENTAL`).

use super::*;

/// Speicherschutz; gespeichert in `meta` `storage`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    /// bei knappem Speicher selbst ausdünnen und verkleinern
    pub protect: bool,
    /// erste Stufe: älter als so viele Tage auf einen je Tag
    pub first_days: i64,
    /// zweite Stufe, wenn die erste nicht reicht
    pub second_days: i64,
}

impl Default for StorageSettings {
    fn default() -> Self {
        StorageSettings {
            protect: true,
            first_days: 365,
            second_days: 90,
        }
    }
}

/// Jünger wird nie automatisch ausgedünnt
pub const MIN_DAYS: i64 = 30;

impl StorageSettings {
    fn check(&self) -> Result<()> {
        if self.second_days < MIN_DAYS || self.first_days < self.second_days {
            anyhow::bail!(tr(
                "Stufen: erste ≥ zweite ≥ 30 Tage",
                "Steps: first ≥ second ≥ 30 days"
            ));
        }
        Ok(())
    }
}

/// Was der Speicherschutz getan hat
#[derive(Debug, PartialEq)]
pub struct Protected {
    pub days: i64,
    pub deleted: usize,
    pub freed: u64,
}

fn cutoff(now: chrono::NaiveDateTime, days: i64) -> String {
    (now - chrono::Duration::days(days))
        .format("%Y-%m-%d 00:00:00")
        .to_string()
}

impl Db {
    pub fn storage_settings(&self) -> StorageSettings {
        self.meta_json("storage")
    }

    pub fn set_storage_settings(&mut self, s: &StorageSettings) -> Result<()> {
        s.check()?;
        self.set_meta("storage", &serde_json::to_string(s)?)
    }

    /// Vor `cutoff` je Zähler und Tag nur die letzte Auslesung behalten, je Datenpunkt und Tag den
    /// letzten Wert; `project`: nur dessen Zähler, sonst alle. Liefert gelöschte Auslesungen.
    fn thin_before(&self, cutoff: &str, project: Option<i64>) -> Result<usize> {
        const METERS: &str = "SELECT meters.id FROM meters JOIN lines ON lines.id = meters.line_id
             JOIN gateways ON gateways.id = lines.gateway_id WHERE ?2 IS NULL OR gateways.project_id = ?2";
        let n = self.conn.execute(
            &format!(
                "DELETE FROM readings WHERE timestamp < ?1 AND meter IN ({METERS})
                 AND id NOT IN (
                   SELECT MAX(id) FROM readings WHERE timestamp < ?1 AND meter IS NOT NULL
                   GROUP BY meter, substr(timestamp, 1, 10))"
            ),
            params![cutoff, project],
        )?;
        self.conn.execute(
            &format!(
                "DELETE FROM dp_values WHERE time < ?1 AND dp IN (
                   SELECT datapoints.id FROM datapoints WHERE meter_id IN ({METERS}))
                 AND rowid NOT IN (SELECT MAX(rowid) FROM dp_values WHERE time < ?1 GROUP BY dp, substr(time, 1, 10))"
            ),
            params![cutoff, project],
        )?;
        Ok(n)
    }

    /// Verlauf ausdünnen, wo im Projekt eingeschaltet (`thin_days`). Liefert gelöschte Auslesungen.
    pub fn thin_readings(&mut self, now: chrono::NaiveDateTime) -> Result<usize> {
        let projects: Vec<(i64, i64)> = self
            .conn
            .prepare("SELECT id, thin_days FROM projects WHERE thin_days > 0")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut n = 0;
        for (project, days) in projects {
            n += self.thin_before(&cutoff(now, days), Some(project))?;
        }
        Ok(n)
    }

    pub fn set_thin_days(&mut self, project_id: i64, days: Option<i64>) -> Result<()> {
        let days = days.filter(|d| *d > 0);
        if self.conn.execute(
            "UPDATE projects SET thin_days = ?2 WHERE id = ?1",
            params![project_id, days],
        )? == 0
        {
            return Err(not_found(&tr("Projekt", "Project"), project_id));
        }
        Ok(())
    }

    /// Speicher knapp: die erste Stufe, die etwas löscht, ausführen und verkleinern.
    /// `free`: freier Platz beim Datenordner (für die einmalige Umstellung der Datenbank).
    pub fn protect_storage(
        &mut self,
        now: chrono::NaiveDateTime,
        s: &StorageSettings,
        free: u64,
    ) -> Result<Option<Protected>> {
        if !s.protect {
            return Ok(None);
        }
        for days in [s.first_days, s.second_days.max(MIN_DAYS)] {
            let deleted = self.thin_before(&cutoff(now, days), None)?;
            if deleted > 0 {
                let freed = self.compact(free)?;
                return Ok(Some(Protected {
                    days,
                    deleted,
                    freed,
                }));
            }
        }
        Ok(None)
    }

    /// Freie Seiten an das Dateisystem zurückgeben. Ältere Datenbanken ohne `auto_vacuum` werden
    /// einmal umgestellt (VACUUM schreibt eine Kopie, deshalb nur mit genug freiem Platz).
    /// Liefert die frei gewordenen Byte.
    fn compact(&mut self, free: u64) -> Result<u64> {
        let before = self.size();
        let mode: i64 = self
            .conn
            .query_row("PRAGMA auto_vacuum", [], |r| r.get(0))?;
        if mode != 2 {
            if free < before + before / 5 {
                return Ok(0);
            }
            self.conn
                .execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;")?;
        }
        self.conn
            .execute_batch("PRAGMA incremental_vacuum; PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(before.saturating_sub(self.size()))
    }
}
