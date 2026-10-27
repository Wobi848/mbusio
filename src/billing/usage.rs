//! Verbrauch eines Werts in einem Zeitraum: Stand am Anfang (Ende des Vortags) und am Ende, über
//! einen Zählertausch hinweg (Endstand alt + Verbrauch neu). Gemeinsam für den Zeitraum der
//! Abrechnung und die Vorjahre zum Vergleich.

use super::stand::{after_day, Stand, Swap};
use super::*;

pub(super) struct Usage {
    pub start: Stand,
    pub end: Stand,
    pub swaps: Vec<Swap>,
    /// Ende − Anfang, über Tausche gerechnet; None, wenn ein Stand fehlt
    pub used: Option<f64>,
}

impl Db {
    pub(super) fn usage(
        &self,
        meter_id: i64,
        key: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Usage> {
        let before = from - Duration::days(1);
        let start = self.stand_at(meter_id, key, before, Some(to))?;
        let end = self.stand_at(meter_id, key, to, None)?;
        let swaps = self.exchanges_between(meter_id, key, &after_day(before), &after_day(to))?;
        let mut used = match (start.value, end.value) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };
        for sw in &swaps {
            used = match (used, sw.old_end, sw.new_start) {
                (Some(u), Some(o), Some(new)) => Some(u + o - new),
                _ => None,
            };
        }
        Ok(Usage {
            start,
            end,
            swaps,
            used,
        })
    }
}
