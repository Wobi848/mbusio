//! Stand eines Werts am Ende eines Tages (Konzept Export 3.2): Stichtagswert des Zählers, sonst
//! die letzte Auslesung davor, für den Anfang notfalls die erste danach; dazu der Zählertausch.

use chrono::{Duration, NaiveDate};

use super::*;

/// So viele Auslesungen werden je Richtung angesehen (eine kann den Wert nicht enthalten)
const LOOK: usize = 5;
/// Stichtagswerte des Zählers werden in Auslesungen bis so lange nach dem Stichtag gesucht
const STORED_DAYS: i64 = 400;

/// Woher ein Stand kommt
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Stichtagswert, den der Zähler selbst gespeichert hat (genau dieses Datum)
    Meter,
    /// letzte Auslesung am oder vor dem Stichtag
    Reading,
    /// keine davor: erste Auslesung danach (nur für den Anfang eines Zeitraums)
    After,
    Missing,
}

#[derive(Debug, Clone)]
pub struct Stand {
    pub value: Option<f64>,
    pub source: Source,
    /// Zeit der Auslesung, aus der der Wert stammt
    pub time: String,
    pub unsure: bool,
    /// Fehler, den der Zähler in dieser Auslesung selbst meldet (`db::fault_of`)
    pub fault: Option<&'static str>,
}

impl Stand {
    fn missing() -> Self {
        Stand {
            value: None,
            source: Source::Missing,
            time: String::new(),
            unsure: false,
            fault: None,
        }
    }

    fn from(r: &MeterReading, value: f64, source: Source) -> Self {
        Stand {
            value: Some(value),
            source,
            time: r.timestamp.clone(),
            unsure: r.time_unsure,
            fault: crate::db::fault_of(r),
        }
    }
}

/// Ein Zählertausch im Zeitraum: wann, Endstand des alten und Anfang des neuen Zählers (fehlt, wenn
/// keine Auslesung davor bzw. danach den Wert hat)
pub struct Swap {
    pub time: String,
    /// gemeldet oder bestätigt; erkannt und nicht bestätigt: false
    pub confirmed: bool,
    pub old_end: Option<f64>,
    pub new_start: Option<f64>,
}

/// Erste Sekunde nach dem Tag (Ortszeit, Format wie `readings.timestamp`)
pub(super) fn after_day(day: NaiveDate) -> String {
    format!("{} 00:00:00", day + Duration::days(1))
}

/// Zahlenwert zu einer Kennung in einer Auslesung
pub(super) fn value_of(r: &MeterReading, key: &str) -> Option<f64> {
    r.records
        .iter()
        .find(|x| key_of(x) == key)
        .and_then(|x| x.value.as_f64())
}

/// Stichtagswert, den der Zähler für `day` gespeichert hat: gleicher Wert mit Speicher > 0 und ein
/// Datum mit derselben Speichernummer, das genau `day` ist
fn stored_value(r: &MeterReading, key: &str, day: NaiveDate) -> Option<f64> {
    let k: Vec<&str> = key.split('|').collect();
    let [function, "0", tariff, subunit, quantity, unit] = k[..] else {
        return None;
    };
    let date = day.to_string();
    r.records.iter().find_map(|x| {
        let same = x.storage > 0
            && x.function == function
            && x.tariff.to_string() == tariff
            && x.subunit.to_string() == subunit
            && x.quantity == quantity
            && x.unit == unit;
        let dated = same
            && r.records.iter().any(|d| {
                d.storage == x.storage
                    && matches!(d.quantity.as_str(), "date" | "date_time")
                    && d.value_text.starts_with(&date)
            });
        if dated {
            x.value.as_f64()
        } else {
            None
        }
    })
}

impl Db {
    /// Stand von `key` am Ende von `day`; `allow_after`: fehlt eine Auslesung davor, die erste
    /// danach (bis `until`) nehmen
    pub fn stand_at(
        &self,
        meter_id: i64,
        key: &str,
        day: NaiveDate,
        allow_after: Option<NaiveDate>,
    ) -> Result<Stand> {
        let end = after_day(day);
        // 1. Stichtagswert des Zählers aus einer späteren Auslesung
        let horizon = after_day(day + Duration::days(STORED_DAYS));
        for r in self.meter_readings_from(meter_id, &end, LOOK)? {
            if r.timestamp >= horizon {
                break;
            }
            if let Some(v) = stored_value(&r, key, day) {
                return Ok(Stand::from(&r, v, Source::Meter));
            }
        }
        // 2. letzte Auslesung am oder vor dem Tag
        for r in self.meter_readings_before(meter_id, &end, LOOK)? {
            if let Some(v) = value_of(&r, key) {
                return Ok(Stand::from(&r, v, Source::Reading));
            }
        }
        // 3. erste Auslesung danach, im Zeitraum
        if let Some(until) = allow_after {
            let limit = after_day(until);
            for r in self.meter_readings_from(meter_id, &end, LOOK)? {
                if r.timestamp >= limit {
                    break;
                }
                if let Some(v) = value_of(&r, key) {
                    return Ok(Stand::from(&r, v, Source::After));
                }
            }
        }
        Ok(Stand::missing())
    }

    /// Zählertausche zwischen den beiden Zeitpunkten.
    /// Alt und neu werden über die ID im Telegramm unterschieden: der neue Zähler wird vor dem
    /// Eintrag des Tauschs gelesen, seine erste Auslesung ist also etwas älter als der Tausch.
    pub fn exchanges_between(
        &self,
        meter_id: i64,
        key: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<Swap>> {
        let found: Vec<(String, Option<String>, bool)> = {
            let mut st = self.conn.prepare(
                "SELECT time, new_id, confirmed FROM exchanges WHERE meter_id = ?1 AND time >= ?2 AND time < ?3 ORDER BY time",
            )?;
            let rows = st.query_map(params![meter_id, from, to], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let is_new = |r: &MeterReading, new: &Option<String>| {
            new.is_some() && r.header.as_ref().map(|h| &h.id) == new.as_ref()
        };
        let mut out = Vec::new();
        for (time, new_id, confirmed) in found {
            // eine Stunde Spielraum vor dem Eintrag: so lange dauert ein Tausch höchstens
            let window = crate::plan::parse_time(&time)
                .map(|t| {
                    (t - Duration::hours(1))
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| time.clone());
            let old_end = self
                .meter_readings_before(meter_id, &time, LOOK * 2)?
                .iter()
                .filter(|r| !is_new(r, &new_id))
                .find_map(|r| value_of(r, key));
            let new_start = self
                .meter_readings_from(meter_id, &window, LOOK * 2)?
                .iter()
                .filter(|r| is_new(r, &new_id) || (new_id.is_none() && r.timestamp >= time))
                .find_map(|r| value_of(r, key));
            out.push(Swap {
                time,
                confirmed,
                old_end,
                new_start,
            });
        }
        Ok(out)
    }
}
