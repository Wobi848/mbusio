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
mod exchange;
mod outputs;
mod templates;
#[cfg(test)]
mod tests;

pub use self::outputs::{output, Output, Outputs, OUTPUTS};

use self::outputs::Sample;

use crate::db::{Db, Meter, DISTURBED_AFTER};
use crate::i18n::{self, tr, Lang};
use crate::mbus::decode::Record;
use crate::mbus::master::{MeterReading, Target};
use crate::plan::{self, Plan};
use anyhow::{anyhow, bail, Result};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Zählerstände: dürfen nicht sinken, werden beim Zählertausch fortgeführt
const COUNTERS: [&str; 5] = ["energy", "energy_cooling", "volume", "mass", "hca_units"];

/// Weitergabe (wirkt ab den Ausgängen MQTT/Modbus)
pub const SEND_MODES: [&str; 3] = ["every", "change", "daily"];

/// Kennung eines Werts: Funktion|Speicher|Tarif|Untereinheit|Grösse|Einheit
pub fn key_of(r: &Record) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        r.function, r.storage, r.tariff, r.subunit, r.quantity, r.unit
    )
}

struct KeyParts<'a> {
    function: &'a str,
    storage: &'a str,
    tariff: &'a str,
    subunit: &'a str,
    quantity: &'a str,
    unit: &'a str,
}

fn parts(key: &str) -> Option<KeyParts<'_>> {
    let mut it = key.splitn(6, '|');
    Some(KeyParts {
        function: it.next()?,
        storage: it.next()?,
        tariff: it.next()?,
        subunit: it.next()?,
        quantity: it.next()?,
        unit: it.next()?,
    })
}

/// Zählerstand (aktuell, Speicher 0)? Nur dort gilt „darf nicht sinken“.
pub fn is_counter(key: &str) -> bool {
    parts(key).is_some_and(|p| {
        p.function == "instantaneous" && p.storage == "0" && COUNTERS.contains(&p.quantity)
    })
}

/// Lesbare Bezeichnung des Werts, z.B. „Energie · Tarif 1 · Speicher 2“
pub fn key_label(key: &str, lang: Lang) -> String {
    let Some(p) = parts(key) else {
        return key.to_string();
    };
    let mut s = i18n::label(p.quantity, lang);
    if p.function != "instantaneous" {
        s += &format!(" · {}", i18n::label(p.function, lang));
    }
    let de = lang == Lang::De;
    if p.tariff != "0" {
        s += &format!(" · {} {}", if de { "Tarif" } else { "tariff" }, p.tariff);
    }
    if p.storage != "0" {
        s += &format!(
            " · {} {}",
            if de { "Speicher" } else { "storage" },
            p.storage
        );
    }
    if p.subunit != "0" {
        s += &format!(
            " · {} {}",
            if de { "Untereinheit" } else { "subunit" },
            p.subunit
        );
    }
    s
}

pub fn key_unit(key: &str) -> String {
    parts(key).map(|p| p.unit.to_string()).unwrap_or_default()
}

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

/// Datenpunkt aus einem Projekt-Export; ältere Dateien hatten MQTT als eigene Felder
pub fn from_export(v: &Value) -> Option<Datapoint> {
    let mut v = v.clone();
    if v.get("outputs").is_none()
        && (v["mqtt"].as_bool() == Some(true) || v["mqtt_topic"].is_string())
    {
        v["outputs"] = json!({ "mqtt": { "enabled": v["mqtt"], "address": v["mqtt_topic"] } });
    }
    serde_json::from_value(v).ok()
}

/// Vorlage aus einem Projekt-Export (ältere Dateien: `mqtt: true`)
pub fn template_from_export(v: &Value) -> Option<Template> {
    let mut v = v.clone();
    if v.get("outputs").is_none() && v["mqtt"].as_bool() == Some(true) {
        v["outputs"] = json!(["mqtt"]);
    }
    serde_json::from_value(v).ok()
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
    details TEXT NOT NULL
);
";

/// Qualität eines Datenpunkts: "none" (noch kein Wert), "good", "stale", "disturbed", "invalid".
/// `line_down`: beim letzten Termin war die Linie nicht erreichbar (Wandler weg, Gateway aus) oder
/// kein Zähler hat geantwortet; dann gilt der Wert sofort als gestört, nicht erst nach 3 Terminen.
pub fn quality(
    dp: &Datapoint,
    meter: &Meter,
    plan: Plan,
    line_down: bool,
    now: chrono::NaiveDateTime,
) -> &'static str {
    let Some(time) = dp.last_time.as_deref().and_then(plan::parse_time) else {
        return "none";
    };
    if dp.invalid.is_some() {
        return "invalid";
    }
    let disturbed = if plan == Plan::Off {
        meter.last_error.is_some()
    } else {
        line_down || meter.plan_fails >= DISTURBED_AFTER
    };
    if disturbed {
        return "disturbed";
    }
    if let Some(iv) = plan.interval_s() {
        if (now - time).num_seconds() as f64 > 3.0 * iv {
            return "stale";
        }
    }
    "good"
}

/// Prüft einen neuen Wert: None = plausibel, sonst Grund
pub fn check(key: &str, value: f64, last_good: Option<f64>, status: u8) -> Option<&'static str> {
    // Statusbyte: 0x08 dauerhafter, 0x10 vorübergehender Fehler des Zählers. Gilt für Messwerte
    // (Temperatur, Leistung, Durchfluss); Zählerstände führt der Zähler weiter, die bleiben gültig
    // (z.B. Luft im Rohr: Durchfluss gestört, Energiestand stimmt).
    if status & 0x18 != 0 && !is_counter(key) {
        return Some("status");
    }
    if is_counter(key) {
        if let Some(g) = last_good {
            if value < g - g.abs() * 1e-9 {
                return Some("decreased");
            }
        }
    }
    None
}

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

    /// Werte eines Zählers zur Auswahl: alle Zahlenwerte seiner letzten Auslesung, mit Kennung,
    /// Bezeichnung, Wert, ob schon Datenpunkt und ob Hauptwert (Vorschlag)
    pub fn dp_candidates(&self, meter_id: i64, lang: Lang) -> Result<Value> {
        let meter = self.meter(meter_id)?;
        let existing: HashMap<String, i64> = self
            .dp_by_meter(meter_id)?
            .into_iter()
            .map(|d| (d.key, d.id))
            .collect();
        let reading = self.latest_reading(meter_id)?;
        let main = reading.as_ref().and_then(crate::db::main_value).map(key_of);
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();
        for r in reading.iter().flat_map(|r| r.records.iter()) {
            if !r.value.is_number() {
                continue;
            }
            let key = key_of(r);
            if !seen.insert(key.clone()) {
                continue;
            }
            items.push(json!({
                "key": key,
                "label": key_label(&key, lang),
                "value": r.value_text,
                "unit": r.unit,
                "selected": existing.contains_key(&key),
                "main": main.as_deref() == Some(key.as_str()),
                "counter": is_counter(&key),
            }));
        }
        // Datenpunkte, deren Wert in der letzten Auslesung fehlt, trotzdem zeigen
        for (key, _) in existing.iter().filter(|(k, _)| !seen.contains(*k)) {
            items.push(json!({ "key": key, "label": key_label(key, lang), "value": null, "unit": key_unit(key),
                "selected": true, "main": false, "counter": is_counter(key) }));
        }
        Ok(json!({
            "meter": meter,
            "read": reading.as_ref().map(|r| r.timestamp.clone()),
            "values": items,
            "template": self.templates(meter.line_id)?.into_iter().find(|t| Some(&t.medium) == meter.medium.as_ref()),
        }))
    }

    /// Auswahl eines Zählers setzen: fehlende Datenpunkte anlegen (gleich an diese Ausgänge),
    /// abgewählte löschen
    pub fn dp_select(
        &mut self,
        meter_id: i64,
        keys: &[String],
        outputs: &[String],
        lang: Lang,
    ) -> Result<(usize, usize)> {
        let meter = self.meter(meter_id)?;
        self.tx(|db| {
            let mut added = 0;
            for k in keys {
                if let Some(id) = db.dp_create(&meter, k, lang)? {
                    for o in outputs {
                        db.dp_set_output(id, o, true)?;
                    }
                    added += 1;
                }
            }
            let mut removed = 0;
            for d in db.dp_by_meter(meter_id)? {
                if !keys.contains(&d.key) {
                    db.conn
                        .execute("DELETE FROM datapoints WHERE id = ?1", [d.id])?;
                    removed += 1;
                }
            }
            Ok((added, removed))
        })
    }

    /// Name, Einheit, Faktor, Weitergabe, aktiv und Ausgänge ändern
    pub fn dp_update(&mut self, id: i64, d: &Datapoint) -> Result<Datapoint> {
        if d.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        if !SEND_MODES.contains(&d.send.as_str()) {
            bail!("send: {}", d.send);
        }
        if !d.factor.is_finite() || d.factor == 0.0 {
            bail!(tr(
                "Faktor muss eine Zahl ungleich 0 sein",
                "Factor must be a non-zero number"
            ));
        }
        let old = self.dp(id)?;
        for (key, o) in &d.outputs {
            if let Some(a) = o
                .address
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
            {
                output(key)?.check_address(a)?;
            }
        }
        // anderer Faktor: gespeicherte Werte passen nicht mehr zum Vergleich
        let reset = old.factor != d.factor;
        self.conn.execute(
            "UPDATE datapoints SET name = ?2, unit = ?3, factor = ?4, send = ?5, active = ?6,
               last_good = CASE WHEN ?7 THEN NULL ELSE last_good END,
               invalid = CASE WHEN ?7 THEN NULL ELSE invalid END
             WHERE id = ?1",
            params![
                id,
                d.name.trim(),
                d.unit.trim(),
                d.factor,
                d.send,
                d.active,
                reset
            ],
        )?;
        if reset {
            if let Some(r) = self.latest_reading(old.meter_id)? {
                self.dp_apply(old.meter_id, &r, false)?;
            }
        }
        self.dp_set_outputs(id, &d.outputs)?;
        self.dp(id)
    }

    /// Mehrere Datenpunkte auf einmal: "on", "off", "delete", "send:every|change|daily",
    /// "out:<ausgang>:on|off"
    pub fn dp_bulk(&mut self, ids: &[i64], action: &str) -> Result<usize> {
        self.tx(|db| {
            let mut n = 0;
            for id in ids {
                n += match action {
                    "on" => db
                        .conn
                        .execute("UPDATE datapoints SET active = 1 WHERE id = ?1", [id])?,
                    "off" => db
                        .conn
                        .execute("UPDATE datapoints SET active = 0 WHERE id = ?1", [id])?,
                    "delete" => db
                        .conn
                        .execute("DELETE FROM datapoints WHERE id = ?1", [id])?,
                    a if a.starts_with("out:") => {
                        let (key, on) =
                            match a.strip_prefix("out:").and_then(|x| x.rsplit_once(':')) {
                                Some((k, "on")) => (k, true),
                                Some((k, "off")) => (k, false),
                                _ => bail!("action: {a}"),
                            };
                        db.dp_set_output(*id, key, on)?;
                        1
                    }
                    a => match a.strip_prefix("send:").filter(|m| SEND_MODES.contains(m)) {
                        Some(m) => db.conn.execute(
                            "UPDATE datapoints SET send = ?2 WHERE id = ?1",
                            params![id, m],
                        )?,
                        None => bail!("action: {a}"),
                    },
                };
            }
            Ok(n)
        })
    }

    /// Neue Auslesung auf die Datenpunkte des Zählers übertragen. `store`: Wert auch in die
    /// Wertetabelle (Verlauf, Weitergabe). Liefert Meldungen (Zählerstand gesunken).
    pub(crate) fn dp_apply(
        &mut self,
        meter_id: i64,
        r: &MeterReading,
        store: bool,
    ) -> Result<Vec<String>> {
        let status = r.header.as_ref().map_or(0, |h| h.status);
        let utc = r.utc.unwrap_or_else(crate::clock::utc_secs);
        // weitergegeben wird nur aus Linien im Dauerbetrieb, nie beim Debuggen alter Projekte
        let send = store && self.meter_in_operation(meter_id)?;
        let mut notes = Vec::new();
        for dp in self.dp_by_meter(meter_id)? {
            // meldet der Zähler den Wert als Fehlerwert (Funktion „error_value“), ist er sofort
            // ungültig, statt still zu fehlen und erst nach drei Abständen zu veralten
            let find = |k: &str| r.records.iter().find(|x| key_of(x) == k);
            let err_key = dp
                .key
                .split_once('|')
                .map(|(_, rest)| format!("error_value|{rest}"));
            let (rec, meter_error) = match find(&dp.key) {
                Some(x) => (x, false),
                None => match err_key.as_deref().and_then(find) {
                    Some(x) => (x, true),
                    None => continue,
                },
            };
            let Some(raw) = rec.value.as_f64() else {
                continue;
            };
            let value = raw * dp.factor + dp.shift;
            let invalid = if meter_error {
                Some("error_value")
            } else {
                check(&dp.key, value, dp.last_good, status)
            };
            if invalid == Some("decreased") && dp.invalid.as_deref() != Some("decreased") {
                notes.push(format!(
                    "{}: {} {} → {} – {}",
                    dp.name,
                    tr("Zählerstand gesunken", "Meter reading decreased"),
                    dp.last_good.unwrap_or_default(),
                    value,
                    tr("Zählertausch?", "meter replaced?")
                ));
            }
            let good = if invalid.is_none() {
                Some(value)
            } else {
                dp.last_good
            };
            self.conn.execute(
                "UPDATE datapoints SET last_value = ?2, last_time = ?3, last_utc = ?4, last_good = ?5, invalid = ?6,
                   last_unsure = ?7
                 WHERE id = ?1",
                params![dp.id, value, r.timestamp, utc, good, invalid, r.time_unsure],
            )?;
            if store {
                self.conn.execute(
                    "INSERT INTO dp_values (dp, utc, time, value, valid) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![dp.id, utc, r.timestamp, value, invalid.is_none()],
                )?;
            }
            // an die Ausgänge, wie beim Datenpunkt eingestellt (auch bei vollem Speicher;
            // nicht beim blossen Nachrechnen, z.B. nach geändertem Faktor)
            if send {
                let day = r.timestamp.get(..10).unwrap_or("");
                let sample = Sample {
                    value,
                    quality: if invalid.is_some() { "invalid" } else { "good" },
                    utc,
                    unsure: r.time_unsure,
                    day,
                };
                self.dispatch(&dp, &sample)?;
            }
        }
        Ok(notes)
    }

    /// Zähler einer Linie (oder eines Projekts) mit ihren Datenpunkten, je Datenpunkt mit
    /// Qualität und Bezeichnung des Werts. Grundlage für Liste, Übersicht und CSV.
    fn dp_by_meters(
        &self,
        line_id: Option<i64>,
        project_id: Option<i64>,
    ) -> Result<Vec<(String, Meter, Vec<Value>)>> {
        let now = crate::clock::now();
        // Linie: id, Name, Plan, Plan des Projekts, beim letzten Termin nicht erreichbar
        type LineRow = (i64, String, Option<String>, String, bool, bool);
        let lines: Vec<LineRow> = self
            .conn
            .prepare(
                "SELECT lines.id, gateways.name || ' › ' || lines.name, lines.plan, projects.plan,
                   COALESCE(json_extract(lines.plan_result, '$.offline'), 0)
                     OR COALESCE(json_extract(lines.plan_result, '$.line_down'), 0),
                   lines.operation
                 FROM lines JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
                 WHERE (?1 IS NULL OR lines.id = ?1) AND (?2 IS NULL OR projects.id = ?2)
                 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
            )?
            .query_map(params![line_id, project_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (lid, label, line_plan, project_plan, line_down, operation) in lines {
            for m in self.meters(lid)? {
                let (p, _) =
                    plan::effective(m.plan.as_deref(), line_plan.as_deref(), &project_plan);
                let mut dps = Vec::new();
                for d in self.dp_by_meter(m.id)? {
                    let mut v = serde_json::to_value(&d)?;
                    v["quality"] = json!(quality(&d, &m, p, line_down, now));
                    v["label"] = json!(key_label(&d.key, i18n::current()));
                    v["line_id"] = json!(lid);
                    v["operation"] = json!(operation);
                    v["line"] = json!(label);
                    v["meter"] = json!(meter_label(&m));
                    v["address"] = json!(m.address);
                    dps.push(v);
                }
                out.push((label.clone(), m, dps));
            }
        }
        Ok(out)
    }

    /// Alle Datenpunkte einer Linie (oder eines Projekts), nach Nummer
    pub fn dp_list(&self, line_id: Option<i64>, project_id: Option<i64>) -> Result<Vec<Value>> {
        let mut out: Vec<Value> = self
            .dp_by_meters(line_id, project_id)?
            .into_iter()
            .flat_map(|(_, _, dps)| dps)
            .collect();
        out.sort_by_key(|v| v["no"].as_i64().unwrap_or(0));
        Ok(out)
    }

    /// Für den Reiter Weitergabe: je Zähler (auch ohne Datenpunkte) seine Datenpunkte
    pub fn dp_overview(&self, line_id: Option<i64>, project_id: Option<i64>) -> Result<Vec<Value>> {
        Ok(self
            .dp_by_meters(line_id, project_id)?
            .into_iter()
            .map(|(line, m, dps)| {
                json!({
                    "id": m.id,
                    "name": meter_label(&m),
                    "location": m.location,
                    "address": m.address,
                    "meter_id": m.meter_id,
                    "medium": m.medium,
                    "last_read": m.last_read,
                    "line_id": m.line_id,
                    "line": line,
                    "datapoints": dps,
                })
            })
            .collect())
    }

    /// Ausgänge für die Oberfläche: Schlüssel, Name, wie die Adresse heisst, eingeschaltet
    pub fn outputs_info(&self) -> Vec<Value> {
        OUTPUTS
            .iter()
            .map(|o| json!({ "key": o.key(), "label": o.label(), "address_label": o.address_label(), "on": o.is_on(self) }))
            .collect()
    }

    /// Datenpunkt aus einem Projekt-Export übernehmen (Nummer bleibt)
    pub(crate) fn dp_import_one(&mut self, meter_id: i64, d: &Datapoint) -> Result<()> {
        if parts(&d.key).is_none() {
            return Ok(());
        }
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO datapoints (meter_id, no, key, name, unit, factor, shift, send, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                meter_id,
                d.no,
                d.key,
                d.name,
                d.unit,
                d.factor,
                d.shift,
                if SEND_MODES.contains(&d.send.as_str()) { d.send.as_str() } else { "every" },
                d.active
            ],
        )?;
        if n == 1 {
            let id = self.conn.last_insert_rowid();
            for (key, o) in d.outputs.iter().filter(|(k, _)| output(k).is_ok()) {
                let address = o
                    .address
                    .as_deref()
                    .filter(|a| output(key).is_ok_and(|o| o.check_address(a).is_ok()));
                self.conn.execute(
                    "INSERT OR IGNORE INTO dp_outputs (dp, output, enabled, address) VALUES (?1, ?2, ?3, ?4)",
                    params![id, key, o.enabled, address],
                )?;
            }
        }
        Ok(())
    }

    /// Nach dem Import: Zähler für Nummern hinter die grösste vorhandene setzen
    pub(crate) fn dp_fix_next(&mut self, project_id: i64) -> Result<()> {
        self.next_no(project_id)?;
        Ok(())
    }
}
