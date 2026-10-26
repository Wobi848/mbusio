//! Datenpunkte (Konzept Dauerbetrieb, Phase 2): ein bestimmter Wert eines bestimmten Zählers,
//! z.B. „Whg 3 Wärme · Energie“. Nur Datenpunkte werden später weitergegeben (MQTT, Modbus).
//!
//! - **Stabile Kennung** aus der Bedeutung des Werts (Funktion, Speicher, Tarif, Untereinheit,
//!   Grösse, Einheit), nicht aus der Position im Telegramm.
//! - **Stabile Nummer** je Projekt: wird nie neu vergeben, Grundlage für Register und Objekte.
//! - **Qualität:** gut · veraltet · gestört · ungültig.
//! - **Plausibilität:** Zählerstände dürfen nicht sinken.
//! - **Zählertausch:** alter Endstand und neuer Anfangsstand werden festgehalten, der Datenpunkt
//!   läuft durchgehend weiter.

mod csv;
mod edit;
mod exchange;
mod key;
mod list;
mod live;
mod outputs;
mod project_file;
mod quality;
mod select;
mod templates;
#[cfg(test)]
mod tests;
mod values;

pub use self::key::{is_counter, key_label, key_of, key_unit};
use self::key::{parts, COUNTERS};
pub use self::live::{LiveState, LiveValue, Scope};
pub(crate) use self::outputs::Entry as OutLogEntry;
pub use self::outputs::{output, sample_of, LogFilter, LogRow, Output, Outputs, OUTPUTS};
pub use self::project_file::{from_export, template_from_export};
pub use self::quality::{check, quality};

use self::outputs::Sample;

use crate::db::{Db, Meter, DISTURBED_AFTER};
use crate::i18n::{self, both, tr, Lang, Text};
use crate::mbus::decode::Record;
use crate::mbus::master::{MeterReading, Target};
use crate::plan::{self, Plan};
use anyhow::{anyhow, bail, Result};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Weitergabe (wirkt ab den Ausgängen MQTT/Modbus)
pub const SEND_MODES: [&str; 3] = ["every", "change", "daily"];

/// Bezeichnung des Zählers für Namen: Name, sonst ID, sonst Adresse
pub fn meter_label(m: &Meter) -> String {
    if !m.name.is_empty() {
        m.name.clone()
    } else {
        m.meter_id.clone().unwrap_or_else(|| m.address.clone())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Datapoint {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub meter_id: i64,
    /// stabile Nummer im Projekt
    #[serde(default)]
    pub no: i64,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub unit: String,
    /// Umrechnung: Wert × Faktor (z.B. Wh → kWh: 0.001)
    #[serde(default = "one")]
    pub factor: f64,
    /// Fortführung nach Zählertausch: wird zum umgerechneten Wert addiert
    #[serde(default)]
    pub shift: f64,
    /// "every", "change", "daily"
    #[serde(default = "every")]
    pub send: String,
    #[serde(default = "yes")]
    pub active: bool,
    /// Ausgänge (MQTT …): je Ausgang ein/aus und Adresse
    #[serde(default)]
    pub outputs: Outputs,
    #[serde(default, skip_deserializing)]
    pub last_value: Option<f64>,
    #[serde(default, skip_deserializing)]
    pub last_time: Option<String>,
    #[serde(default, skip_deserializing)]
    pub last_utc: Option<i64>,
    /// letzter plausibler Wert (für „darf nicht sinken“)
    #[serde(default, skip_deserializing)]
    pub last_good: Option<f64>,
    /// warum der letzte Wert ungültig ist: "decreased" oder "status"
    #[serde(default, skip_deserializing)]
    pub invalid: Option<String>,
}

fn one() -> f64 {
    1.0
}

fn every() -> String {
    "every".into()
}

fn yes() -> bool {
    true
}

fn dp_of(r: &Row) -> rusqlite::Result<Datapoint> {
    Ok(Datapoint {
        id: r.get("id")?,
        meter_id: r.get("meter_id")?,
        no: r.get("no")?,
        key: r.get("key")?,
        name: r.get("name")?,
        unit: r.get("unit")?,
        factor: r.get("factor")?,
        shift: r.get("shift")?,
        send: r.get("send")?,
        active: r.get("active")?,
        outputs: Outputs::new(),
        last_value: r.get("last_value")?,
        last_time: r.get("last_time")?,
        last_utc: r.get("last_utc")?,
        last_good: r.get("last_good")?,
        invalid: r.get("invalid")?,
    })
}

/// Vorlage einer Linie: diese Werte bei allen Zählern dieses Mediums
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub line_id: i64,
    pub medium: String,
    pub keys: Vec<String>,
    /// auch auf später hinzukommende Zähler anwenden
    #[serde(default)]
    pub auto: bool,
    /// Ausgänge, an die neue Datenpunkte gleich gehen
    #[serde(default)]
    pub outputs: Vec<String>,
}

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS datapoints (
    id INTEGER PRIMARY KEY,
    meter_id INTEGER NOT NULL REFERENCES meters(id) ON DELETE CASCADE,
    no INTEGER NOT NULL,
    key TEXT NOT NULL,
    name TEXT NOT NULL,
    unit TEXT NOT NULL DEFAULT '',
    factor REAL NOT NULL DEFAULT 1,
    shift REAL NOT NULL DEFAULT 0,
    send TEXT NOT NULL DEFAULT 'every',
    active INTEGER NOT NULL DEFAULT 1,
    last_value REAL,
    last_time TEXT,
    last_utc INTEGER,
    last_good REAL,
    invalid TEXT,
    UNIQUE (meter_id, key)
);
CREATE TABLE IF NOT EXISTS dp_values (
    dp INTEGER NOT NULL REFERENCES datapoints(id) ON DELETE CASCADE,
    utc INTEGER NOT NULL,
    time TEXT NOT NULL,
    value REAL NOT NULL,
    valid INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS dp_values_dp ON dp_values(dp, utc);
CREATE TABLE IF NOT EXISTS dp_outputs (
    dp INTEGER NOT NULL REFERENCES datapoints(id) ON DELETE CASCADE,
    output TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    address TEXT,
    sent_value REAL,
    sent_quality TEXT,
    sent_day TEXT,
    PRIMARY KEY (dp, output)
);
CREATE TABLE IF NOT EXISTS out_log (
    id INTEGER PRIMARY KEY,
    utc INTEGER NOT NULL,
    time TEXT NOT NULL,
    output TEXT NOT NULL,
    dp INTEGER,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    value TEXT NOT NULL,
    quality TEXT NOT NULL,
    result TEXT NOT NULL,
    reason TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS out_log_utc ON out_log(utc);
CREATE TABLE IF NOT EXISTS dp_templates (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    medium TEXT NOT NULL,
    keys TEXT NOT NULL,
    auto INTEGER NOT NULL DEFAULT 0,
    UNIQUE (line_id, medium)
);
CREATE TABLE IF NOT EXISTS exchanges (
    id INTEGER PRIMARY KEY,
    meter_id INTEGER NOT NULL REFERENCES meters(id) ON DELETE CASCADE,
    time TEXT NOT NULL,
    old_id TEXT,
    new_id TEXT,
    details TEXT NOT NULL,
    confirmed INTEGER NOT NULL DEFAULT 1
);
";

// ---------------------------------------------------------------- Datenbank

impl Db {
    pub(crate) fn dp_by_meter(&self, meter_id: i64) -> Result<Vec<Datapoint>> {
        let mut st = self
            .conn
            .prepare("SELECT * FROM datapoints WHERE meter_id = ?1 ORDER BY no")?;
        let mut list: Vec<Datapoint> = st
            .query_map([meter_id], dp_of)?
            .collect::<rusqlite::Result<_>>()?;
        let mut outputs = self.outputs_of_meter(meter_id)?;
        for d in &mut list {
            d.outputs = outputs.remove(&d.id).unwrap_or_default();
        }
        Ok(list)
    }

    pub fn dp(&self, id: i64) -> Result<Datapoint> {
        let d = self
            .conn
            .query_row("SELECT * FROM datapoints WHERE id = ?1", [id], dp_of)
            .optional()?
            .ok_or_else(|| {
                anyhow!(
                    "{} {id}: {}",
                    tr("Datenpunkt", "Datapoint"),
                    tr("nicht gefunden", "not found")
                )
            })?;
        Ok(Datapoint {
            outputs: self
                .outputs_of_meter(d.meter_id)?
                .remove(&d.id)
                .unwrap_or_default(),
            ..d
        })
    }

    /// Projekt, zu dem ein Zähler (und damit jeder seiner Datenpunkte) gehört
    pub(super) fn project_of_meter(&self, meter_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT gateways.project_id FROM meters JOIN lines ON lines.id = meters.line_id
             JOIN gateways ON gateways.id = lines.gateway_id WHERE meters.id = ?1",
            [meter_id],
            |r| r.get(0),
        )?)
    }

    /// Nächste Nummer im Projekt; Nummern werden nie neu vergeben, auch nach dem Löschen nicht
    pub(super) fn next_no(&self, project_id: i64) -> Result<i64> {
        let no: i64 = self.conn.query_row(
            "SELECT MAX(dp_next, COALESCE((SELECT MAX(no) + 1 FROM datapoints JOIN meters ON meters.id = datapoints.meter_id
               JOIN lines ON lines.id = meters.line_id JOIN gateways ON gateways.id = lines.gateway_id
               WHERE gateways.project_id = ?1), 1)) FROM projects WHERE id = ?1",
            [project_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "UPDATE projects SET dp_next = ?2 WHERE id = ?1",
            params![project_id, no + 1],
        )?;
        Ok(no)
    }

    /// Datenpunkt anlegen, falls es ihn für diesen Zähler und Wert noch nicht gibt
    pub(super) fn dp_create(
        &mut self,
        meter: &Meter,
        key: &str,
        lang: Lang,
    ) -> Result<Option<i64>> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM datapoints WHERE meter_id = ?1 AND key = ?2)",
            params![meter.id, key],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(None);
        }
        let no = self.next_no(self.project_of_meter(meter.id)?)?;
        let name = format!("{} · {}", meter_label(meter), key_label(key, lang));
        self.conn.execute(
            "INSERT INTO datapoints (meter_id, no, key, name, unit) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meter.id, no, key, name, key_unit(key)],
        )?;
        let id = self.conn.last_insert_rowid();
        // gleich mit dem letzten Wert füllen, damit man sofort etwas sieht
        if let Some(r) = self.latest_reading(meter.id)? {
            self.dp_apply(meter.id, &r, false)?;
        }
        Ok(Some(id))
    }
}
