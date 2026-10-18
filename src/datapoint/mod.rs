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
mod publish;
mod templates;
#[cfg(test)]
mod tests;

pub use self::csv::ImportResult;

use self::publish::*;

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
const COUNTERS: [&str; 4] = ["energy", "volume", "mass", "hca_units"];

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
    /// per MQTT weitergeben
    #[serde(default)]
    pub mqtt: bool,
    /// Topic relativ zum Basis-Topic; wird beim Einschalten einmal erzeugt und bleibt dann stabil
    #[serde(default)]
    pub mqtt_topic: Option<String>,
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
        mqtt: r.get("mqtt")?,
        mqtt_topic: r.get("mqtt_topic")?,
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
    /// neue Datenpunkte gleich per MQTT weitergeben
    #[serde(default)]
    pub mqtt: bool,
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

/// Qualität eines Datenpunkts: "none" (noch kein Wert), "good", "stale", "disturbed", "invalid"
pub fn quality(
    dp: &Datapoint,
    meter: &Meter,
    plan: Plan,
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
        meter.plan_fails >= DISTURBED_AFTER
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
    // Statusbyte: 0x08 dauerhafter, 0x10 vorübergehender Fehler des Zählers
    if status & 0x18 != 0 {
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
        let rows = st.query_map([meter_id], dp_of)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn dp(&self, id: i64) -> Result<Datapoint> {
        self.conn
            .query_row("SELECT * FROM datapoints WHERE id = ?1", [id], dp_of)
            .optional()?
            .ok_or_else(|| {
                anyhow!(
                    "{} {id}: {}",
                    tr("Datenpunkt", "Datapoint"),
                    tr("nicht gefunden", "not found")
                )
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

    /// Auswahl eines Zählers setzen: fehlende Datenpunkte anlegen, abgewählte löschen
    #[cfg(test)]
    pub fn dp_select(
        &mut self,
        meter_id: i64,
        keys: &[String],
        lang: Lang,
    ) -> Result<(usize, usize)> {
        self.dp_select_mqtt(meter_id, keys, false, lang)
    }

    /// Wie `dp_select`; neue Datenpunkte auf Wunsch gleich per MQTT
    pub fn dp_select_mqtt(
        &mut self,
        meter_id: i64,
        keys: &[String],
        mqtt: bool,
        lang: Lang,
    ) -> Result<(usize, usize)> {
        let meter = self.meter(meter_id)?;
        self.tx(|db| {
            let mut added = 0;
            for k in keys {
                if let Some(id) = db.dp_create(&meter, k, lang)? {
                    if mqtt {
                        db.dp_set_mqtt(id, true)?;
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

    /// Name, Einheit, Faktor, Weitergabe, aktiv ändern
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
        let topic = d
            .mqtt_topic
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty());
        if let Some(t) = topic {
            crate::mqtt::check_topic(t)?;
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
        // eigenes Topic übernehmen; geändertes Topic: aktuellen Wert dort neu senden
        if topic != old.mqtt_topic.as_deref() {
            self.conn.execute(
                "UPDATE datapoints SET mqtt_topic = ?2, mqtt_quality = NULL WHERE id = ?1",
                params![id, topic],
            )?;
        }
        if d.mqtt != old.mqtt || (d.mqtt && topic.is_none()) {
            self.dp_set_mqtt(id, d.mqtt)?;
        }
        self.dp(id)
    }

    /// Mehrere Datenpunkte auf einmal: "on", "off", "delete", "send:every|change|daily"
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
                    "mqtt:on" | "mqtt:off" => {
                        db.dp_set_mqtt(*id, action == "mqtt:on")?;
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
        let mut notes = Vec::new();
        for dp in self.dp_by_meter(meter_id)? {
            let Some(rec) = r.records.iter().find(|x| key_of(x) == dp.key) else {
                continue;
            };
            let Some(raw) = rec.value.as_f64() else {
                continue;
            };
            let value = raw * dp.factor + dp.shift;
            let invalid = check(&dp.key, value, dp.last_good, status);
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
            // weitergeben, wie beim Datenpunkt eingestellt (auch bei vollem Speicher)
            if self.mqtt_enabled && dp.mqtt && dp.active && store {
                if let Some(topic) = dp.mqtt_topic.as_deref() {
                    let quality = if invalid.is_some() { "invalid" } else { "good" };
                    let (sent_value, sent_quality, sent_day): (
                        Option<f64>,
                        Option<String>,
                        Option<String>,
                    ) = self.conn.query_row(
                        "SELECT mqtt_value, mqtt_quality, mqtt_day FROM datapoints WHERE id = ?1",
                        [dp.id],
                        |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)),
                    )?;
                    let day = r.timestamp.get(..10).unwrap_or("").to_string();
                    let quality_changed = sent_quality.as_deref() != Some(quality);
                    let send = match dp.send.as_str() {
                        "change" => sent_value != Some(value) || quality_changed,
                        "daily" => sent_day.as_deref() != Some(day.as_str()) || quality_changed,
                        _ => true,
                    };
                    if send {
                        self.mqtt_enqueue(
                            topic,
                            &mqtt_payload(&dp, value, quality, utc, r.time_unsure),
                        )?;
                        self.conn.execute(
                            "UPDATE datapoints SET mqtt_value = ?2, mqtt_quality = ?3, mqtt_day = ?4 WHERE id = ?1",
                            params![dp.id, value, quality, day],
                        )?;
                    }
                }
            }
        }
        Ok(notes)
    }

    /// Datenpunkte einer Linie (oder eines Projekts) für die Oberfläche, mit Zähler und Qualität
    pub fn dp_list(&self, line_id: Option<i64>, project_id: Option<i64>) -> Result<Vec<Value>> {
        let now = crate::clock::now();
        let lines: Vec<(i64, String, Option<String>, String)> = self
            .conn
            .prepare(
                "SELECT lines.id, gateways.name || ' › ' || lines.name, lines.plan, projects.plan
                 FROM lines JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
                 WHERE (?1 IS NULL OR lines.id = ?1) AND (?2 IS NULL OR projects.id = ?2)
                 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
            )?
            .query_map(params![line_id, project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (lid, label, line_plan, project_plan) in lines {
            for m in self.meters(lid)? {
                let (p, _) =
                    plan::effective(m.plan.as_deref(), line_plan.as_deref(), &project_plan);
                for d in self.dp_by_meter(m.id)? {
                    let q = quality(&d, &m, p, now);
                    let mut v = serde_json::to_value(&d)?;
                    v["quality"] = json!(q);
                    v["label"] = json!(key_label(&d.key, i18n::current()));
                    v["line_id"] = json!(lid);
                    v["line"] = json!(label);
                    v["meter"] = json!(meter_label(&m));
                    v["address"] = json!(m.address);
                    out.push(v);
                }
            }
        }
        out.sort_by_key(|v| v["no"].as_i64().unwrap_or(0));
        Ok(out)
    }

    /// Anzahl Datenpunkte und ungültige je Linie (für die Projekt-Karte)
    pub(crate) fn dp_counts(&self, line_id: i64) -> Result<(i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(invalid IS NOT NULL), 0) FROM datapoints
             JOIN meters ON meters.id = datapoints.meter_id WHERE meters.line_id = ?1",
            [line_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    /// Datenpunkte für den Projekt-Export (je Zähler) und Vorlagen (je Linie)
    pub(crate) fn dp_export(&self, meter_id: i64) -> Result<Vec<Datapoint>> {
        self.dp_by_meter(meter_id)
    }

    /// Datenpunkt aus einem Projekt-Export übernehmen (Nummer bleibt)
    pub(crate) fn dp_import_one(&mut self, meter_id: i64, d: &Datapoint) -> Result<()> {
        if parts(&d.key).is_none() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO datapoints (meter_id, no, key, name, unit, factor, shift, send, active, mqtt, mqtt_topic)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                meter_id,
                d.no,
                d.key,
                d.name,
                d.unit,
                d.factor,
                d.shift,
                if SEND_MODES.contains(&d.send.as_str()) { d.send.as_str() } else { "every" },
                d.active,
                d.mqtt,
                d.mqtt_topic.as_deref().filter(|t| crate::mqtt::check_topic(t).is_ok())
            ],
        )?;
        Ok(())
    }

    /// Nach dem Import: Zähler für Nummern hinter die grösste vorhandene setzen
    pub(crate) fn dp_fix_next(&mut self, project_id: i64) -> Result<()> {
        self.next_no(project_id)?;
        Ok(())
    }
}
