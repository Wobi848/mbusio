//! Zahlen des Berichts aus der Datenbank (gelesene Zähler je Linie, Weitergabe je Ausgang) und
//! der gespeicherte Zustand

use std::collections::HashMap;

use anyhow::Result;

use super::ReportState;
use crate::db::Db;
use crate::plan::Plan;

const STATE_KEY: &str = "report_state";

/// Eine Linie: so viele Zähler mit Plan gelesen, von so vielen
pub struct ReadLine {
    /// Projekt › Gateway › Linie
    pub place: String,
    pub read: usize,
    pub total: usize,
}

/// Ein Ausgang: Einträge im Protokoll der Weitergabe je Ergebnis
#[derive(Default)]
pub struct OutputCount {
    pub output: String,
    pub sent: i64,
    pub dropped: i64,
    pub failed: i64,
}

impl Db {
    pub fn report_state(&self) -> ReportState {
        self.meta_json(STATE_KEY)
    }

    pub fn set_report_state(&self, s: &ReportState) -> Result<()> {
        self.set_meta(STATE_KEY, &serde_json::to_string(s)?)
    }

    /// Linien mit Zählern im Plan; gelesen = letzte Auslesung seit `since` (Ortszeit wie `stamp`)
    pub fn report_reading(&self, since: &str) -> Result<Vec<ReadLine>> {
        let projects: HashMap<i64, String> = self
            .conn
            .prepare(
                "SELECT gateways.id, projects.name FROM gateways
                 JOIN projects ON projects.id = gateways.project_id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for pl in self.planned()? {
            let planned: Vec<_> = pl.meters.iter().filter(|(_, p)| *p != Plan::Off).collect();
            if planned.is_empty() {
                continue;
            }
            let read = planned
                .iter()
                .filter(|(m, _)| m.last_read.as_deref().is_some_and(|t| t >= since))
                .count();
            let project = projects
                .get(&pl.line.gateway_id)
                .cloned()
                .unwrap_or_default();
            out.push(ReadLine {
                place: format!("{project} › {}", pl.label),
                read,
                total: planned.len(),
            });
        }
        Ok(out)
    }

    /// Einträge im Protokoll der Weitergabe seit `since` je Ausgang
    pub fn report_outputs(&self, since: &str) -> Result<Vec<OutputCount>> {
        let mut st = self.conn.prepare(
            "SELECT output, result, COUNT(*) FROM out_log WHERE time >= ?1
             GROUP BY output, result ORDER BY output",
        )?;
        let rows = st.query_map([since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out: Vec<OutputCount> = Vec::new();
        for row in rows {
            let (output, result, n) = row?;
            if out.last().is_none_or(|o| o.output != output) {
                out.push(OutputCount {
                    output,
                    ..Default::default()
                });
            }
            let o = out.last_mut().expect("eben angelegt");
            match result.as_str() {
                "sent" => o.sent = n,
                "dropped" => o.dropped = n,
                "failed" => o.failed = n,
                _ => {}
            }
        }
        Ok(out)
    }
}
