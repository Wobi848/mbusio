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

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{Db, Meter, DISTURBED_AFTER};
use crate::i18n::{self, tr, Lang};
use crate::mbus::decode::Record;
use crate::mbus::master::{MeterReading, Target};
use crate::plan::{self, Plan};

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

// ---------------------------------------------------------------- CSV

/// Spalten der CSV (Reihenfolge fest, Kopfzeile wird beim Import übersprungen)
const CSV_HEAD_DE: &str =
    "Nr;Linie;Zähler;Adresse;Kennung;Name;Einheit;Faktor;Weitergabe;Aktiv;MQTT;Topic";
const CSV_HEAD_EN: &str = "No;Line;Meter;Address;Key;Name;Unit;Factor;Send;Active;MQTT;Topic";

fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Eine CSV-Zeile mit Semikolon, Anführungszeichen wie Excel
fn csv_split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ';' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

fn parse_num(s: &str) -> Option<f64> {
    // Excel mit deutschem Gebietsschema schreibt Komma
    s.trim().replace(',', ".").replace('\'', "").parse().ok()
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "1" | "ja" | "yes" | "x" | "true" | "wahr" | "✓" => Some(true),
        "0" | "nein" | "no" | "" | "false" | "falsch" | "-" => Some(false),
        _ => None,
    }
}

/// Nutzlast für MQTT: Wert, Einheit, Zeit (UTC), Qualität, Name, Nummer
fn mqtt_payload(d: &Datapoint, value: f64, quality: &str, utc: i64, time_unsure: bool) -> String {
    let time = chrono::DateTime::from_timestamp(utc, 0)
        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default();
    let mut v = json!({
        "value": value,
        "unit": d.unit,
        "time": time,
        "quality": quality,
        "name": d.name,
        "no": d.no,
    });
    if time_unsure {
        v["time_unsure"] = json!(true);
    }
    v.to_string()
}

/// Letzter Teil des Topics aus der Kennung, sprachunabhängig: energy, energy-t1, volume-s2, power-max
fn key_slug(key: &str) -> String {
    let Some(p) = parts(key) else {
        return crate::mqtt::slug(key);
    };
    let mut s = p.quantity.replace('_', "-");
    if p.function != "instantaneous" {
        s += &format!("-{}", &p.function[..p.function.len().min(3)]);
    }
    if p.tariff != "0" {
        s += &format!("-t{}", p.tariff);
    }
    if p.storage != "0" {
        s += &format!("-s{}", p.storage);
    }
    if p.subunit != "0" {
        s += &format!("-u{}", p.subunit);
    }
    s
}

/// Ergebnis eines CSV-Imports (Vorschau oder ausgeführt)
#[derive(Debug, Default, Serialize)]
pub struct ImportResult {
    /// geänderte Felder: Nr, Name, Feld, alt, neu
    pub changes: Vec<Value>,
    /// neu anzulegende Datenpunkte
    pub added: Vec<Value>,
    /// Zeilen, die nicht übernommen werden können
    pub errors: Vec<String>,
    pub applied: bool,
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

    fn project_of_meter(&self, meter_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT gateways.project_id FROM meters JOIN lines ON lines.id = meters.line_id
             JOIN gateways ON gateways.id = lines.gateway_id WHERE meters.id = ?1",
            [meter_id],
            |r| r.get(0),
        )?)
    }

    /// Nächste Nummer im Projekt; Nummern werden nie neu vergeben, auch nach dem Löschen nicht
    fn next_no(&self, project_id: i64) -> Result<i64> {
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
    fn dp_create(&mut self, meter: &Meter, key: &str, lang: Lang) -> Result<Option<i64>> {
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

    /// Vorgeschlagenes Topic (relativ): projekt/gateway-linie/zaehler/wert; eindeutig gemacht
    fn dp_default_topic(&self, id: i64) -> Result<String> {
        let (project, gateway, line, meter_id): (String, String, String, i64) = self.conn.query_row(
            "SELECT projects.name, gateways.name, lines.name, meters.id FROM datapoints
             JOIN meters ON meters.id = datapoints.meter_id JOIN lines ON lines.id = meters.line_id
             JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
             WHERE datapoints.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let d = self.dp(id)?;
        let m = self.meter(meter_id)?;
        let slug = crate::mqtt::slug;
        let topic = format!(
            "{}/{}-{}/{}/{}",
            slug(&project),
            slug(&gateway),
            slug(&line),
            slug(&meter_label(&m)),
            key_slug(&d.key)
        );
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM datapoints WHERE mqtt_topic = ?1 AND id != ?2)",
            params![topic, id],
            |r| r.get(0),
        )?;
        Ok(if taken {
            format!("{topic}-{}", d.no)
        } else {
            topic
        })
    }

    /// MQTT für einen Datenpunkt ein- oder ausschalten; beim Einschalten Topic erzeugen (falls
    /// noch keins) und den aktuellen Wert beim nächsten Durchgang senden
    fn dp_set_mqtt(&mut self, id: i64, on: bool) -> Result<()> {
        if on {
            let d = self.dp(id)?;
            let topic = match d.mqtt_topic {
                Some(t) => t,
                None => self.dp_default_topic(id)?,
            };
            self.conn.execute(
                "UPDATE datapoints SET mqtt = 1, mqtt_topic = ?2, mqtt_quality = NULL WHERE id = ?1",
                params![id, topic],
            )?;
        } else {
            self.conn
                .execute("UPDATE datapoints SET mqtt = 0 WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    /// Qualität an den Broker melden, wenn sie sich geändert hat (z.B. Zähler antwortet nicht
    /// mehr, Wert veraltet), oder ein Datenpunkt gerade eingeschaltet wurde. Liefert die Anzahl.
    pub fn mqtt_refresh(&mut self) -> Result<usize> {
        if !self.mqtt_enabled {
            return Ok(0);
        }
        let mut n = 0;
        for v in self.dp_list(None, None)? {
            let d: Datapoint = serde_json::from_value(v.clone())?;
            let quality = v["quality"].as_str().unwrap_or("none");
            let id = v["id"].as_i64().unwrap_or(0);
            let (Some(value), Some(topic)) = (v["last_value"].as_f64(), d.mqtt_topic.as_deref())
            else {
                continue;
            };
            if !d.mqtt || !d.active || quality == "none" {
                continue;
            }
            let (sent, unsure): (Option<String>, bool) = self.conn.query_row(
                "SELECT mqtt_quality, last_unsure FROM datapoints WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if sent.as_deref() == Some(quality) {
                continue;
            }
            let utc = v["last_utc"]
                .as_i64()
                .unwrap_or_else(crate::clock::utc_secs);
            self.mqtt_enqueue(topic, &mqtt_payload(&d, value, quality, utc, unsure))?;
            self.conn.execute(
                "UPDATE datapoints SET mqtt_quality = ?2, mqtt_value = ?3 WHERE id = ?1",
                params![id, quality, value],
            )?;
            n += 1;
        }
        Ok(n)
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

    /// Vorlagen anwenden, die für neue Zähler gelten (einmal je Zähler, nach seiner ersten
    /// Auslesung, wenn das Medium bekannt ist)
    pub(crate) fn dp_auto_templates(&mut self, meter_id: i64) -> Result<usize> {
        let meter = self.meter(meter_id)?;
        let done: bool = self.conn.query_row(
            "SELECT templated FROM meters WHERE id = ?1",
            [meter_id],
            |r| r.get(0),
        )?;
        if done || meter.medium.is_none() {
            return Ok(0);
        }
        self.conn
            .execute("UPDATE meters SET templated = 1 WHERE id = ?1", [meter_id])?;
        let mut n = 0;
        for t in self.templates(meter.line_id)? {
            if t.auto && Some(&t.medium) == meter.medium.as_ref() {
                n += self.apply_template_to(&meter, &t, i18n::current())?;
            }
        }
        Ok(n)
    }

    fn apply_template_to(&mut self, meter: &Meter, t: &Template, lang: Lang) -> Result<usize> {
        // nur Werte, die der Zähler auch liefert
        let Some(r) = self.latest_reading(meter.id)? else {
            return Ok(0);
        };
        let mut n = 0;
        for k in &t.keys {
            if r.records
                .iter()
                .any(|x| key_of(x) == *k && x.value.is_number())
            {
                if let Some(id) = self.dp_create(meter, k, lang)? {
                    if t.mqtt {
                        self.dp_set_mqtt(id, true)?;
                    }
                    n += 1;
                }
            }
        }
        Ok(n)
    }

    pub fn templates(&self, line_id: i64) -> Result<Vec<Template>> {
        let mut st = self
            .conn
            .prepare("SELECT id, line_id, medium, keys, auto, mqtt FROM dp_templates WHERE line_id = ?1 ORDER BY id")?;
        let rows = st.query_map([line_id], |r| {
            Ok(Template {
                id: r.get(0)?,
                line_id: r.get(1)?,
                medium: r.get(2)?,
                keys: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                auto: r.get(4)?,
                mqtt: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Vorlage für ein Medium der Linie speichern und auf alle passenden Zähler anwenden.
    /// Liefert die Anzahl neuer Datenpunkte.
    pub fn template_save(
        &mut self,
        line_id: i64,
        medium: &str,
        keys: &[String],
        auto: bool,
        mqtt: bool,
        lang: Lang,
    ) -> Result<usize> {
        self.tx(|db| {
            db.conn.execute(
                "INSERT INTO dp_templates (line_id, medium, keys, auto, mqtt) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(line_id, medium) DO UPDATE SET keys = excluded.keys, auto = excluded.auto, mqtt = excluded.mqtt",
                params![line_id, medium, serde_json::to_string(keys)?, auto, mqtt],
            )?;
            let t = Template {
                id: 0,
                line_id,
                medium: medium.into(),
                keys: keys.to_vec(),
                auto,
                mqtt,
            };
            let mut n = 0;
            for m in db.meters(line_id)? {
                if m.medium.as_deref() == Some(medium) {
                    n += db.apply_template_to(&m, &t, lang)?;
                }
            }
            Ok(n)
        })
    }

    pub fn template_delete(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM dp_templates WHERE id = ?1", [id])?;
        Ok(())
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

    /// Zählertausch: Zählerstände laufen mit dem neuen Zähler durchgehend weiter. Liefert eine
    /// Zeile je Zählerstand (alter Endstand → neuer Anfangsstand).
    pub fn exchange_meter(
        &mut self,
        meter_id: i64,
        new_address: &str,
        reading: &MeterReading,
    ) -> Result<Vec<String>> {
        let meter = self.meter(meter_id)?;
        let new_address = new_address.trim().to_ascii_uppercase();
        let target = Target::parse(&new_address)?;
        if self
            .meters(meter.line_id)?
            .iter()
            .any(|m| m.id != meter_id && crate::db::listed(m, &target))
        {
            bail!(tr(
                "Zähler mit dieser Adresse ist schon in der Liste",
                "A meter with this address is already in the list"
            ));
        }
        let new_id = reading.header.as_ref().map(|h| h.id.clone());
        self.tx(|db| {
            let mut lines = Vec::new();
            for dp in db.dp_by_meter(meter_id)? {
                if !is_counter(&dp.key) {
                    db.conn.execute("UPDATE datapoints SET invalid = NULL WHERE id = ?1", [dp.id])?;
                    continue;
                }
                let Some(old_end) = dp.last_good.or(dp.last_value) else { continue };
                let Some(start) = reading
                    .records
                    .iter()
                    .find(|x| key_of(x) == dp.key)
                    .and_then(|x| x.value.as_f64())
                else {
                    lines.push(format!("{}: {}", dp.name, tr("neuer Zähler liefert diesen Wert nicht", "new meter does not provide this value")));
                    continue;
                };
                let start = start * dp.factor;
                db.conn.execute(
                    "UPDATE datapoints SET shift = ?2, last_good = ?3, invalid = NULL WHERE id = ?1",
                    params![dp.id, old_end - start, old_end],
                )?;
                lines.push(format!(
                    "{}: {} {old_end} {} → {} {start} {}",
                    dp.name,
                    tr("Endstand", "final"),
                    dp.unit,
                    tr("Anfang neu", "new start"),
                    dp.unit
                ));
            }
            db.conn.execute(
                "UPDATE meters SET address = ?2, meter_id = NULL, manufacturer = NULL, secondary_address = NULL,
                   primary_address = NULL, last_error = NULL, plan_fails = 0, plan_pause = NULL WHERE id = ?1",
                params![meter_id, new_address],
            )?;
            db.conn.execute(
                "INSERT INTO exchanges (meter_id, time, old_id, new_id, details) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![meter_id, crate::clock::stamp(), meter.meter_id, new_id, serde_json::to_string(&lines)?],
            )?;
            db.add_change(
                meter.line_id,
                &format!(
                    "{} {}: {} → {}{}",
                    tr("Zähler getauscht", "Meter replaced"),
                    meter_label(&meter),
                    meter.meter_id.as_deref().unwrap_or(&meter.address),
                    new_id.as_deref().unwrap_or(&new_address),
                    if lines.is_empty() { String::new() } else { format!(" ({})", lines.join("; ")) }
                ),
            )?;
            db.record(meter.line_id, &target, Ok(reading), false)?;
            Ok(lines)
        })
    }

    // ------------------------------------------------------------ CSV

    pub fn dp_csv(&self, project_id: i64, lang: Lang) -> Result<String> {
        let mut out = String::from(if lang == Lang::De {
            CSV_HEAD_DE
        } else {
            CSV_HEAD_EN
        });
        out.push_str("\r\n");
        for v in self.dp_list(None, Some(project_id))? {
            let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
            let row = [
                v["no"].to_string(),
                s("line"),
                s("meter"),
                s("address"),
                s("key"),
                s("name"),
                s("unit"),
                v["factor"].as_f64().unwrap_or(1.0).to_string(),
                s("send"),
                (if v["active"].as_bool().unwrap_or(false) {
                    "1"
                } else {
                    "0"
                })
                .to_string(),
                (if v["mqtt"].as_bool().unwrap_or(false) {
                    "1"
                } else {
                    "0"
                })
                .to_string(),
                s("mqtt_topic"),
            ];
            out.push_str(
                &row.iter()
                    .map(|x| csv_field(x))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
            out.push_str("\r\n");
        }
        Ok(out)
    }

    /// CSV einlesen: zuerst nur zeigen, was sich ändert (`apply` = false), dann übernehmen.
    /// Zuordnung über die Nummer; Zeilen ohne Nummer legen neue Datenpunkte an (Linie, Adresse,
    /// Kennung). Fehlende Zeilen löschen nichts.
    pub fn dp_import(
        &mut self,
        project_id: i64,
        text: &str,
        apply: bool,
        lang: Lang,
    ) -> Result<ImportResult> {
        let existing: HashMap<i64, Value> = self
            .dp_list(None, Some(project_id))?
            .into_iter()
            .map(|v| (v["no"].as_i64().unwrap_or(0), v))
            .collect();
        let mut res = ImportResult::default();
        let mut updates: Vec<(i64, Datapoint)> = Vec::new();
        let mut creates: Vec<(Meter, String, Option<Datapoint>)> = Vec::new();
        let text = text.trim_start_matches('\u{feff}');
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let c = csv_split(line);
            let row = n + 1;
            let col = |i: usize| c.get(i).cloned().unwrap_or_default();
            if n == 0 && parse_num(&col(0)).is_none() && !col(0).is_empty() {
                continue; // Kopfzeile
            }
            if c.len() < 10 {
                res.errors.push(format!(
                    "{} {row}: {}",
                    tr("Zeile", "Row"),
                    tr(
                        "zu wenige Spalten (10 erwartet)",
                        "too few columns (10 expected)"
                    )
                ));
                continue;
            }
            let Some(factor) = parse_num(&col(7)).filter(|f| f.is_finite() && *f != 0.0) else {
                res.errors.push(format!(
                    "{} {row}: {} „{}“",
                    tr("Zeile", "Row"),
                    tr("Faktor ungültig", "invalid factor"),
                    col(7)
                ));
                continue;
            };
            let send = col(8).to_lowercase();
            if !SEND_MODES.contains(&send.as_str()) {
                res.errors.push(format!(
                    "{} {row}: {} „{}“ ({})",
                    tr("Zeile", "Row"),
                    tr("Weitergabe ungültig", "invalid send mode"),
                    col(8),
                    SEND_MODES.join("/")
                ));
                continue;
            }
            let Some(active) = parse_bool(&col(9)) else {
                res.errors.push(format!(
                    "{} {row}: {} „{}“",
                    tr("Zeile", "Row"),
                    tr("Aktiv ungültig (1/0)", "invalid active (1/0)"),
                    col(9)
                ));
                continue;
            };
            // Spalten MQTT und Topic gibt es erst ab Phase 3; fehlen sie, bleibt es wie es ist
            let has_mqtt = c.len() >= 12;
            let Some(mqtt) = (if has_mqtt {
                parse_bool(&col(10))
            } else {
                Some(false)
            }) else {
                res.errors.push(format!(
                    "{} {row}: {} „{}“",
                    tr("Zeile", "Row"),
                    tr("MQTT ungültig (1/0)", "invalid MQTT (1/0)"),
                    col(10)
                ));
                continue;
            };
            let topic = Some(col(11)).filter(|t| has_mqtt && !t.is_empty());
            if let Some(t) = &topic {
                if let Err(e) = crate::mqtt::check_topic(t) {
                    res.errors
                        .push(format!("{} {row}: {e:#}", tr("Zeile", "Row")));
                    continue;
                }
            }
            let wanted = Datapoint {
                key: col(4),
                name: col(5),
                unit: col(6),
                factor,
                send,
                active,
                mqtt,
                mqtt_topic: topic,
                ..Default::default()
            };
            if wanted.name.is_empty() {
                res.errors.push(format!(
                    "{} {row}: {}",
                    tr("Zeile", "Row"),
                    tr("Name fehlt", "name missing")
                ));
                continue;
            }
            match parse_num(&col(0)).map(|x| x as i64) {
                Some(no) => {
                    let Some(old) = existing.get(&no) else {
                        res.errors.push(format!(
                            "{} {row}: {} {no} {}",
                            tr("Zeile", "Row"),
                            tr("Nr.", "No."),
                            tr("gibt es im Projekt nicht", "does not exist in the project")
                        ));
                        continue;
                    };
                    let mut changed = false;
                    // ohne MQTT-Spalten die bisherigen Werte behalten
                    let wanted = if has_mqtt {
                        wanted
                    } else {
                        Datapoint {
                            mqtt: old["mqtt"].as_bool().unwrap_or(false),
                            mqtt_topic: old["mqtt_topic"].as_str().map(String::from),
                            ..wanted
                        }
                    };
                    let fields: [(&str, String, String); 7] = [
                        (
                            "name",
                            old["name"].as_str().unwrap_or("").into(),
                            wanted.name.clone(),
                        ),
                        (
                            "unit",
                            old["unit"].as_str().unwrap_or("").into(),
                            wanted.unit.clone(),
                        ),
                        (
                            "factor",
                            old["factor"].as_f64().unwrap_or(1.0).to_string(),
                            wanted.factor.to_string(),
                        ),
                        (
                            "send",
                            old["send"].as_str().unwrap_or("").into(),
                            wanted.send.clone(),
                        ),
                        (
                            "active",
                            (old["active"].as_bool().unwrap_or(false) as u8).to_string(),
                            (wanted.active as u8).to_string(),
                        ),
                        (
                            "mqtt",
                            (old["mqtt"].as_bool().unwrap_or(false) as u8).to_string(),
                            (wanted.mqtt as u8).to_string(),
                        ),
                        // leeres Topic in der Datei: vorhandenes behalten
                        (
                            "topic",
                            old["mqtt_topic"].as_str().unwrap_or("").into(),
                            wanted
                                .mqtt_topic
                                .clone()
                                .or_else(|| old["mqtt_topic"].as_str().map(String::from))
                                .unwrap_or_default(),
                        ),
                    ];
                    for (field, a, b) in fields {
                        if a != b {
                            changed = true;
                            res.changes.push(json!({ "no": no, "name": old["name"], "field": field, "old": a, "new": b }));
                        }
                    }
                    if changed {
                        updates.push((old["id"].as_i64().unwrap_or(0), wanted));
                    }
                }
                None => {
                    // neu: Zähler über Linie und Adresse finden
                    let line = col(1);
                    let address = col(3).to_ascii_uppercase();
                    let meter = self
                        .dp_list_meters(project_id)?
                        .into_iter()
                        .find(|(l, m)| *l == line && m.address == address)
                        .map(|(_, m)| m);
                    let Some(meter) = meter else {
                        res.errors.push(format!(
                            "{} {row}: {} „{line}“ / „{address}“",
                            tr("Zeile", "Row"),
                            tr("Zähler nicht gefunden", "meter not found")
                        ));
                        continue;
                    };
                    if parts(&wanted.key).is_none() {
                        res.errors.push(format!(
                            "{} {row}: {} „{}“",
                            tr("Zeile", "Row"),
                            tr("Kennung ungültig", "invalid key"),
                            wanted.key
                        ));
                        continue;
                    }
                    res.added.push(json!({ "meter": meter_label(&meter), "key": wanted.key, "name": wanted.name }));
                    creates.push((meter, wanted.key.clone(), Some(wanted)));
                }
            }
        }
        if apply && res.errors.is_empty() {
            self.tx(|db| {
                for (id, d) in &updates {
                    db.dp_update(*id, d)?;
                }
                for (m, key, d) in &creates {
                    if let (Some(id), Some(d)) = (db.dp_create(m, key, lang)?, d) {
                        db.dp_update(id, d)?;
                    }
                }
                Ok(())
            })?;
            res.applied = true;
        }
        Ok(res)
    }

    /// Alle Zähler des Projekts mit der Bezeichnung ihrer Linie ("Gateway › Linie")
    fn dp_list_meters(&self, project_id: i64) -> Result<Vec<(String, Meter)>> {
        let lines: Vec<(i64, String)> = self
            .conn
            .prepare(
                "SELECT lines.id, gateways.name || ' › ' || lines.name FROM lines
                 JOIN gateways ON gateways.id = lines.gateway_id WHERE gateways.project_id = ?1",
            )?
            .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (id, label) in lines {
            for m in self.meters(id)? {
                out.push((label.clone(), m));
            }
        }
        Ok(out)
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

    pub(crate) fn template_import(&mut self, line_id: i64, t: &Template) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO dp_templates (line_id, medium, keys, auto, mqtt) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![line_id, t.medium, serde_json::to_string(&t.keys)?, t.auto, t.mqtt],
        )?;
        Ok(())
    }

    /// Nach dem Import: Zähler für Nummern hinter die grösste vorhandene setzen
    pub(crate) fn dp_fix_next(&mut self, project_id: i64) -> Result<()> {
        self.next_no(project_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_labels() {
        assert!(is_counter("instantaneous|0|0|0|energy|kWh"));
        assert!(!is_counter("instantaneous|1|0|0|energy|kWh"));
        assert!(!is_counter("instantaneous|0|0|0|flow_temperature|°C"));
        assert_eq!(key_unit("instantaneous|0|1|0|energy|kWh"), "kWh");
        let l = key_label("instantaneous|2|1|0|energy|kWh", Lang::De);
        assert!(l.contains("Tarif 1") && l.contains("Speicher 2"), "{l}");
    }

    #[test]
    fn plausibility() {
        let k = "instantaneous|0|0|0|energy|kWh";
        assert_eq!(check(k, 100.0, Some(99.0), 0), None);
        assert_eq!(check(k, 100.0, Some(100.0), 0), None);
        assert_eq!(check(k, 98.0, Some(99.0), 0), Some("decreased"));
        // Temperatur darf sinken
        assert_eq!(
            check(
                "instantaneous|0|0|0|flow_temperature|°C",
                20.0,
                Some(60.0),
                0
            ),
            None
        );
        // Zähler meldet Fehler im Status
        assert_eq!(check(k, 100.0, None, 0x10), Some("status"));
    }

    use crate::db::Meter;
    use crate::mbus::master::{FrameLog, Master};
    use std::sync::Arc;

    const ENERGY: &str = "instantaneous|0|0|0|energy|kWh";

    /// Wärmezähler 1 aus dem Simulator auslesen
    fn heat_reading() -> MeterReading {
        let link = crate::link::open("SIMULATOR", 2400).unwrap();
        let mut m = Master::new(link, 2400, 60, Arc::new(FrameLog::default()));
        m.read_meter(&Target::Primary(1)).unwrap()
    }

    fn with_energy(r: &MeterReading, value: f64) -> MeterReading {
        let mut r = r.clone();
        for x in r.records.iter_mut().filter(|x| key_of(x) == ENERGY) {
            x.value = json!(value);
            x.value_text = value.to_string();
        }
        r
    }

    fn setup() -> (Db, i64, i64, MeterReading) {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap().id;
        let m = db
            .add_meter(
                line,
                Meter {
                    address: "1".into(),
                    name: "Whg 1".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let r = heat_reading();
        db.record(line, &Target::Primary(1), Ok(&r), false).unwrap();
        (db, line, m.id, r)
    }

    #[test]
    fn select_values_and_follow_readings() {
        let (mut db, line, mid, r) = setup();
        let c = db.dp_candidates(mid, Lang::De).unwrap();
        let values = c["values"].as_array().unwrap();
        // Hauptwert vorgeschlagen, noch nichts gewählt
        let main: Vec<_> = values.iter().filter(|v| v["main"] == true).collect();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0]["key"], ENERGY);
        assert!(values.iter().all(|v| v["selected"] == false));
        assert_eq!(
            db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap(),
            (1, 0)
        );
        let dp = &db.dp_by_meter(mid).unwrap()[0];
        assert_eq!(dp.name, "Whg 1 · Energie");
        assert_eq!(dp.no, 1);
        assert_eq!(dp.last_value, Some(123456.0));
        // neue Auslesung: Wert und Verlauf
        db.record(
            line,
            &Target::Primary(1),
            Ok(&with_energy(&r, 123460.0)),
            false,
        )
        .unwrap();
        let dp = &db.dp_by_meter(mid).unwrap()[0];
        assert_eq!(dp.last_value, Some(123460.0));
        assert!(dp.invalid.is_none());
        // Zählerstand sinkt: ungültig, Meldung, letzter guter Wert bleibt
        db.record(line, &Target::Primary(1), Ok(&with_energy(&r, 5.0)), false)
            .unwrap();
        let dp = &db.dp_by_meter(mid).unwrap()[0];
        assert_eq!(dp.invalid.as_deref(), Some("decreased"));
        assert_eq!(dp.last_good, Some(123460.0));
        assert!(
            db.notes.iter().any(|n| n.contains("gesunken")),
            "{:?}",
            db.notes
        );
        let m = db.meter(mid).unwrap();
        assert_eq!(quality(dp, &m, Plan::Off, crate::clock::now()), "invalid");
        // abwählen löscht, Nummer wird nicht wieder vergeben
        assert_eq!(db.dp_select(mid, &[], Lang::De).unwrap(), (0, 1));
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        assert_eq!(db.dp_by_meter(mid).unwrap()[0].no, 2);
    }

    #[test]
    fn exchange_continues_the_counter() {
        let (mut db, line, mid, r) = setup();
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        // neuer Zähler beginnt bei 10 kWh
        let mut new = with_energy(&r, 10.0);
        new.header.as_mut().unwrap().id = "99999999".into();
        let lines = db.exchange_meter(mid, "5", &new).unwrap();
        assert!(
            lines[0].contains("123456") && lines[0].contains("10"),
            "{lines:?}"
        );
        let dp = &db.dp_by_meter(mid).unwrap()[0];
        assert_eq!(dp.last_value, Some(123456.0));
        assert!(dp.invalid.is_none());
        let m = db.meter(mid).unwrap();
        assert_eq!(m.address, "5");
        assert_eq!(m.meter_id.as_deref(), Some("99999999"));
        // weiter: neuer Zähler bei 12 kWh → 123458
        db.record(
            line,
            &Target::Primary(5),
            Ok(&with_energy(&new, 12.0)),
            false,
        )
        .unwrap();
        assert_eq!(db.dp_by_meter(mid).unwrap()[0].last_value, Some(123458.0));
        assert!(db
            .changes(1)
            .unwrap()
            .iter()
            .any(|c| c.text.contains("getauscht")));
    }

    #[test]
    fn csv_preview_then_apply() {
        let (mut db, _line, mid, _r) = setup();
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        let csv = db.dp_csv(1, Lang::De).unwrap();
        assert!(csv.starts_with("Nr;Linie;Zähler"));
        // in Excel geändert: Name, Faktor (mit Komma), inaktiv; dazu ein neuer Wert ohne Nummer
        let row = csv.lines().nth(1).unwrap();
        let mut c = csv_split(row);
        c[5] = "Wohnung 1 Energie".into();
        c[6] = "MWh".into();
        c[7] = "0,001".into();
        c[9] = "0".into();
        let mut new = c.clone();
        new[0] = String::new();
        new[4] = "instantaneous|0|0|0|volume|m³".into();
        new[5] = "Wohnung 1 Volumen".into();
        new[6] = "m³".into();
        new[7] = "1".into();
        let text = format!(
            "{}\n{}\n{}\n",
            csv.lines().next().unwrap(),
            c.join(";"),
            new.join(";")
        );
        let preview = db.dp_import(1, &text, false, Lang::De).unwrap();
        assert!(preview.errors.is_empty(), "{:?}", preview.errors);
        assert_eq!(preview.changes.len(), 4);
        assert_eq!(preview.added.len(), 1);
        assert!(!preview.applied);
        assert_eq!(
            db.dp_by_meter(mid).unwrap().len(),
            1,
            "Vorschau ändert nichts"
        );
        let done = db.dp_import(1, &text, true, Lang::De).unwrap();
        assert!(done.applied);
        let dps = db.dp_by_meter(mid).unwrap();
        assert_eq!(dps.len(), 2);
        assert_eq!(dps[0].name, "Wohnung 1 Energie");
        assert_eq!(dps[0].factor, 0.001);
        assert!(!dps[0].active);
        assert_eq!(dps[0].last_value, Some(123.456));
        assert_eq!(dps[1].name, "Wohnung 1 Volumen");
        // fehlerhafte Zeile: nichts wird übernommen
        let bad = db
            .dp_import(1, "99;x;x;x;x;x;x;1;every;1", true, Lang::De)
            .unwrap();
        assert!(!bad.applied && bad.errors.len() == 1);
    }

    #[test]
    fn template_applies_to_new_meters() {
        let (mut db, line, mid, r) = setup();
        let medium = db.meter(mid).unwrap().medium.unwrap();
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        db.template_save(line, &medium, &[ENERGY.to_string()], true, false, Lang::De)
            .unwrap();
        // neuer Zähler gleicher Art: nach der ersten Auslesung hat er den Datenpunkt
        let m2 = db
            .add_meter(
                line,
                Meter {
                    address: "8".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut r2 = r.clone();
        r2.primary_address = 8;
        db.record(line, &Target::Primary(8), Ok(&r2), false)
            .unwrap();
        assert_eq!(db.dp_by_meter(m2.id).unwrap().len(), 1);
        // gelöscht bleibt gelöscht (Vorlage gilt nur einmal je Zähler)
        db.dp_select(m2.id, &[], Lang::De).unwrap();
        db.record(line, &Target::Primary(8), Ok(&r2), false)
            .unwrap();
        assert!(db.dp_by_meter(m2.id).unwrap().is_empty());
    }

    #[test]
    fn project_export_keeps_numbers_and_templates() {
        let (mut db, line, mid, _r) = setup();
        let medium = db.meter(mid).unwrap().medium.unwrap();
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        db.template_save(line, &medium, &[ENERGY.to_string()], true, false, Lang::De)
            .unwrap();
        let exported = db.export_project(1).unwrap();
        let pid = db.import_project(&exported).unwrap();
        let list = db.dp_list(None, Some(pid)).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["no"], 1);
        assert_eq!(list[0]["key"], ENERGY);
        let new_line = db
            .tree()
            .unwrap()
            .into_iter()
            .find(|p| p.project.id == pid)
            .unwrap()
            .gateways[0]
            .lines[0]
            .line
            .id;
        assert_eq!(db.templates(new_line).unwrap().len(), 1);
    }

    #[test]
    fn mqtt_queue_follows_send_mode_and_quality() {
        let (mut db, line, mid, r) = setup();
        db.dp_select(mid, &[ENERGY.to_string()], Lang::De).unwrap();
        let id = db.dp_by_meter(mid).unwrap()[0].id;
        db.mqtt_enabled = true;
        db.dp_bulk(&[id], "mqtt:on").unwrap();
        let topic = db.dp(id).unwrap().mqtt_topic.unwrap();
        assert_eq!(topic, "standard/gateway-1-linie-1/whg-1/energy");
        // eingeschaltet: aktueller Wert einmal, dann nicht mehr
        assert_eq!(db.mqtt_refresh().unwrap(), 1);
        assert_eq!(db.mqtt_refresh().unwrap(), 0);
        let q = |db: &Db| db.mqtt_peek(1000).unwrap();
        let first: Value = serde_json::from_str(&q(&db)[0].2).unwrap();
        assert_eq!(first["value"], 123456.0);
        assert_eq!(first["quality"], "good");
        assert_eq!(first["unit"], "kWh");
        // jede Auslesung
        db.record(
            line,
            &Target::Primary(1),
            Ok(&with_energy(&r, 123457.0)),
            false,
        )
        .unwrap();
        assert_eq!(q(&db).len(), 2);
        // nur bei Änderung: gleicher Wert nicht, anderer schon
        let mut d = db.dp(id).unwrap();
        d.send = "change".into();
        db.dp_update(id, &d).unwrap();
        db.record(
            line,
            &Target::Primary(1),
            Ok(&with_energy(&r, 123457.0)),
            false,
        )
        .unwrap();
        assert_eq!(q(&db).len(), 2);
        db.record(
            line,
            &Target::Primary(1),
            Ok(&with_energy(&r, 123458.0)),
            false,
        )
        .unwrap();
        assert_eq!(q(&db).len(), 3);
        // gesunken: geht als ungültig raus
        db.record(line, &Target::Primary(1), Ok(&with_energy(&r, 1.0)), false)
            .unwrap();
        let last: Value = serde_json::from_str(&q(&db).last().unwrap().2).unwrap();
        assert_eq!(last["quality"], "invalid");
        // Zähler umbenannt: Topic bleibt
        let mut m = db.meter(mid).unwrap();
        m.name = "Wohnung 1".into();
        db.update_meter(mid, m).unwrap();
        assert_eq!(db.dp(id).unwrap().mqtt_topic.unwrap(), topic);
        // MQTT aus: nichts mehr
        db.mqtt_enabled = false;
        db.record(
            line,
            &Target::Primary(1),
            Ok(&with_energy(&r, 123460.0)),
            false,
        )
        .unwrap();
        assert_eq!(q(&db).len(), 4);
        // zweiter Datenpunkt mit gleichem Vorschlag bekommt ein eindeutiges Topic
        assert!(crate::mqtt::check_topic(&topic).is_ok());
    }

    #[test]
    fn csv_split_handles_quotes() {
        assert_eq!(
            csv_split(r#"1;"A; B";"say ""hi""";x"#),
            vec!["1", "A; B", "say \"hi\"", "x"]
        );
        assert_eq!(parse_num("0,001"), Some(0.001));
        assert_eq!(parse_num("1'234.5"), Some(1234.5));
    }
}
