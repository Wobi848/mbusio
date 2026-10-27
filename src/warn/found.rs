//! Probleme aus der Datenbank: Zähler (gestört, Fehler, Batterie, Tausch) und nicht gesendete
//! Abrechnungen; dazu der gespeicherte Zustand der Warnungen.

use std::path::Path;

use anyhow::Result;
use chrono::NaiveDateTime;
use rusqlite::OptionalExtension;

use super::{Problem, WarnState};
use crate::datapoint::meter_label;
use crate::db::{Db, Meter, PlannedLine, DISTURBED_AFTER};
use crate::i18n::{both, tr};
use crate::plan::Plan;

const STATE_KEY: &str = "warn_state";

fn meter_problem(
    pl: &PlannedLine,
    m: &Meter,
    kind: &'static str,
    what: impl Fn() -> String,
    since: Option<String>,
) -> Problem {
    let label = meter_label(m);
    Problem {
        key: format!("{kind}:{}", m.id),
        kind,
        text: both(|| format!("{label}: {}", what())),
        place: pl.label.clone(),
        since,
        line: Some(pl.line.id),
        meter: Some(m.id),
        project: None,
    }
}

impl Db {
    pub fn warn_state(&self) -> WarnState {
        self.meta_json(STATE_KEY)
    }

    pub fn set_warn_state(&self, s: &WarnState) -> Result<()> {
        self.set_meta(STATE_KEY, &serde_json::to_string(s)?)
    }

    /// Beginn der offenen Störung eines Zählers
    fn disturbed_since(&self, meter_id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT start FROM disturbances WHERE meter = ?1 AND end IS NULL",
                [meter_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Probleme der Zähler auf diesen Linien
    pub fn meter_problems(&self, lines: &[PlannedLine]) -> Result<Vec<Problem>> {
        let mut out = Vec::new();
        for pl in lines {
            let swaps = self.open_swaps(pl.line.id)?;
            for (m, p) in &pl.meters {
                if *p != Plan::Off && m.plan_fails >= DISTURBED_AFTER {
                    let since = self.disturbed_since(m.id)?;
                    out.push(meter_problem(
                        pl,
                        m,
                        "disturbed",
                        || {
                            tr(
                                "gestört, Termine nacheinander nicht gelesen",
                                "disturbed, terms missed in a row",
                            )
                        },
                        since,
                    ));
                }
                match m.fault.as_deref() {
                    Some("power_low") => out.push(meter_problem(
                        pl,
                        m,
                        "battery",
                        || tr("Batterie schwach", "battery low"),
                        None,
                    )),
                    Some(f) => out.push(meter_problem(
                        pl,
                        m,
                        "fault",
                        || format!("{} ({f})", tr("meldet Fehler", "reports an error")),
                        None,
                    )),
                    None => {}
                }
                if let Some(sw) = swaps.get(&m.id) {
                    let (old, new) = (
                        sw["old_id"].as_str().unwrap_or("?").to_string(),
                        sw["new_id"].as_str().unwrap_or("?").to_string(),
                    );
                    out.push(meter_problem(
                        pl,
                        m,
                        "swap",
                        || {
                            format!(
                                "{} {old} → {new}",
                                tr("Zählertausch erkannt, ID", "meter replacement detected, ID")
                            )
                        },
                        sw["time"].as_str().map(String::from),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Abrechnungen, deren Datei nicht gesendet werden konnte (noch in der Zeit für neue Versuche)
    pub fn billing_problems(&self, data_dir: &Path, now: NaiveDateTime) -> Result<Vec<Problem>> {
        let mut out = Vec::new();
        for (project_id, why) in self.billing_unsent(data_dir, now)? {
            let project = self.project(project_id)?.name;
            out.push(Problem {
                key: format!("billing:{project_id}"),
                kind: "billing",
                text: both(|| {
                    format!(
                        "{}: {why}",
                        tr("Abrechnung nicht gesendet", "Billing export not sent")
                    )
                }),
                place: project.clone(),
                since: None,
                line: None,
                meter: None,
                project: Some(project_id),
            });
        }
        Ok(out)
    }
}
