//! Ausgänge (MQTT, später Modbus): je Datenpunkt und Ausgang ein/aus und eine Adresse (Topic,
//! Register …), gespeichert in `dp_outputs`.
//!
//! Für alle Ausgänge gleich: wann gesendet wird (jede Auslesung, bei Änderung, täglich), dass
//! geänderte Qualität gemeldet wird, was zuletzt gesendet wurde. Was ein Ausgang anders macht,
//! steht nur in den `match`-Zweigen unten (Name, Adresse prüfen/vorschlagen, senden). Ein neuer
//! Ausgang ist ein neuer Eintrag in [`OUTPUTS`] und je ein Zweig.

use std::collections::BTreeMap;

use super::*;

/// Alle Ausgänge, in der Reihenfolge der Oberfläche
pub const OUTPUTS: [&str; 1] = ["mqtt"];

/// Einstellung eines Datenpunkts für einen Ausgang
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DpOutput {
    #[serde(default)]
    pub enabled: bool,
    /// Topic, Register …; wird beim Einschalten vorgeschlagen und bleibt dann stabil
    #[serde(default)]
    pub address: Option<String>,
}

pub type Outputs = BTreeMap<String, DpOutput>;

/// Ein gelesener Wert, wie er an die Ausgänge geht
pub(super) struct Sample<'a> {
    pub value: f64,
    pub quality: &'a str,
    pub utc: i64,
    pub unsure: bool,
    /// Tag der Auslesung (Ortszeit), für „1× täglich“
    pub day: &'a str,
}

/// Name des Ausgangs für Oberfläche und CSV
pub fn output_label(key: &str) -> String {
    match key {
        "mqtt" => "MQTT".into(),
        k => k.into(),
    }
}

/// Wie die Adresse beim Ausgang heisst (Spalte in Oberfläche und CSV)
pub fn address_label(key: &str) -> String {
    match key {
        "mqtt" => "Topic".into(),
        _ => tr("Adresse", "Address"),
    }
}

pub fn check_address(key: &str, address: &str) -> Result<()> {
    match key {
        "mqtt" => crate::mqtt::check_topic(address),
        k => bail!("{}: {k}", tr("Unbekannter Ausgang", "Unknown output")),
    }
}

/// Nutzlast für MQTT: Wert, Einheit, Zeit (UTC), Qualität, Name, Nummer
fn mqtt_payload(d: &Datapoint, s: &Sample) -> String {
    let time = chrono::DateTime::from_timestamp(s.utc, 0)
        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default();
    let mut v = json!({
        "value": s.value,
        "unit": d.unit,
        "time": time,
        "quality": s.quality,
        "name": d.name,
        "no": d.no,
    });
    if s.unsure {
        v["time_unsure"] = json!(true);
    }
    v.to_string()
}

/// Letzter Teil eines Topics aus der Kennung, sprachunabhängig: energy, energy-t1, volume-s2
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

/// Soll gesendet werden? Nach der Weitergabe-Art des Datenpunkts; geänderte Qualität immer
fn should_send(mode: &str, last: &Sent, s: &Sample) -> bool {
    let quality_changed = last.quality.as_deref() != Some(s.quality);
    match mode {
        "change" => last.value != Some(s.value) || quality_changed,
        "daily" => last.day.as_deref() != Some(s.day) || quality_changed,
        _ => true,
    }
}

/// Was an einen Ausgang zuletzt gesendet wurde
#[derive(Default)]
struct Sent {
    value: Option<f64>,
    quality: Option<String>,
    day: Option<String>,
}

impl Db {
    /// Ist der Ausgang als Ganzes eingeschaltet (z.B. MQTT mit Broker)?
    pub fn output_on(&self, key: &str) -> bool {
        match key {
            "mqtt" => self.mqtt_enabled,
            _ => false,
        }
    }

    /// Einstellungen aller Ausgänge für die Datenpunkte eines Zählers: Datenpunkt → Ausgänge
    pub(super) fn outputs_of_meter(&self, meter_id: i64) -> Result<HashMap<i64, Outputs>> {
        let mut st = self.conn.prepare(
            "SELECT dp, output, enabled, address FROM dp_outputs
             WHERE dp IN (SELECT id FROM datapoints WHERE meter_id = ?1)",
        )?;
        let mut out: HashMap<i64, Outputs> = HashMap::new();
        let rows = st.query_map([meter_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                DpOutput {
                    enabled: r.get(2)?,
                    address: r.get(3)?,
                },
            ))
        })?;
        for row in rows {
            let (dp, key, o) = row?;
            out.entry(dp).or_default().insert(key, o);
        }
        Ok(out)
    }

    /// Vorschlag für die Adresse eines Datenpunkts an einem Ausgang, eindeutig gemacht
    fn default_address(&self, key: &str, dp_id: i64) -> Result<String> {
        let d = self.dp(dp_id)?;
        let address = match key {
            "mqtt" => {
                let (project, gateway, line, meter_id): (String, String, String, i64) =
                    self.conn.query_row(
                        "SELECT projects.name, gateways.name, lines.name, meters.id FROM datapoints
                         JOIN meters ON meters.id = datapoints.meter_id JOIN lines ON lines.id = meters.line_id
                         JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
                         WHERE datapoints.id = ?1",
                        [dp_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )?;
                let slug = crate::mqtt::slug;
                format!(
                    "{}/{}-{}/{}/{}",
                    slug(&project),
                    slug(&gateway),
                    slug(&line),
                    slug(&meter_label(&self.meter(meter_id)?)),
                    key_slug(&d.key)
                )
            }
            k => bail!("{}: {k}", tr("Unbekannter Ausgang", "Unknown output")),
        };
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM dp_outputs WHERE output = ?1 AND address = ?2 AND dp != ?3)",
            params![key, address, dp_id],
            |r| r.get(0),
        )?;
        Ok(if taken {
            format!("{address}-{}", d.no)
        } else {
            address
        })
    }

    /// Ausgang für einen Datenpunkt ein- oder ausschalten; beim Einschalten eine Adresse
    /// vorschlagen (falls noch keine) und den aktuellen Wert beim nächsten Durchgang senden
    pub(super) fn dp_set_output(&mut self, dp_id: i64, key: &str, on: bool) -> Result<()> {
        if !OUTPUTS.contains(&key) {
            bail!("{}: {key}", tr("Unbekannter Ausgang", "Unknown output"));
        }
        if !on {
            self.conn.execute(
                "UPDATE dp_outputs SET enabled = 0 WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
            )?;
            return Ok(());
        }
        let current: Option<(bool, Option<String>)> = self
            .conn
            .query_row(
                "SELECT enabled, address FROM dp_outputs WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        // schon an: nichts ändern (sonst würde „zuletzt gesendet“ vergessen)
        if let Some((true, Some(_))) = current {
            return Ok(());
        }
        let address = match current.and_then(|c| c.1) {
            Some(a) => a,
            None => self.default_address(key, dp_id)?,
        };
        self.conn.execute(
            "INSERT INTO dp_outputs (dp, output, enabled, address) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(dp, output) DO UPDATE SET enabled = 1, address = excluded.address, sent_quality = NULL",
            params![dp_id, key, address],
        )?;
        Ok(())
    }

    /// Eigene Adresse setzen (leer = Vorschlag behalten); geändert: aktuellen Wert dort neu senden
    pub(super) fn dp_set_output_address(
        &mut self,
        dp_id: i64,
        key: &str,
        address: Option<&str>,
    ) -> Result<()> {
        let Some(a) = address.map(str::trim).filter(|a| !a.is_empty()) else {
            return Ok(());
        };
        check_address(key, a)?;
        self.conn.execute(
            "INSERT INTO dp_outputs (dp, output, enabled, address) VALUES (?1, ?2, 0, ?3)
             ON CONFLICT(dp, output) DO UPDATE SET address = excluded.address,
               sent_quality = CASE WHEN address IS excluded.address THEN sent_quality ELSE NULL END",
            params![dp_id, key, a],
        )?;
        Ok(())
    }

    /// Alle Ausgänge eines Datenpunkts setzen, wie sie in `outputs` stehen
    pub(super) fn dp_set_outputs(&mut self, dp_id: i64, outputs: &Outputs) -> Result<()> {
        for (key, o) in outputs {
            self.dp_set_output_address(dp_id, key, o.address.as_deref())?;
            self.dp_set_output(dp_id, key, o.enabled)?;
        }
        Ok(())
    }

    fn sent(&self, dp_id: i64, key: &str) -> Result<Sent> {
        Ok(self
            .conn
            .query_row(
                "SELECT sent_value, sent_quality, sent_day FROM dp_outputs WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
                |r| {
                    Ok(Sent {
                        value: r.get(0)?,
                        quality: r.get(1)?,
                        day: r.get(2)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    /// Wert an einen Ausgang geben und als gesendet merken
    fn send_to(&mut self, key: &str, d: &Datapoint, address: &str, s: &Sample) -> Result<()> {
        match key {
            "mqtt" => self.mqtt_enqueue(address, &mqtt_payload(d, s))?,
            k => bail!("{}: {k}", tr("Unbekannter Ausgang", "Unknown output")),
        }
        self.conn.execute(
            "UPDATE dp_outputs SET sent_value = ?3, sent_quality = ?4, sent_day = ?5
             WHERE dp = ?1 AND output = ?2",
            params![d.id, key, s.value, s.quality, s.day],
        )?;
        Ok(())
    }

    /// Neuer Wert eines Datenpunkts: an alle eingeschalteten Ausgänge, wie eingestellt
    pub(super) fn dispatch(&mut self, d: &Datapoint, s: &Sample) -> Result<()> {
        if !d.active {
            return Ok(());
        }
        for (key, o) in &d.outputs {
            let Some(address) = o.address.as_deref().filter(|_| o.enabled) else {
                continue;
            };
            if !self.output_on(key) {
                continue;
            }
            if should_send(&d.send, &self.sent(d.id, key)?, s) {
                self.send_to(key, d, address, s)?;
            }
        }
        Ok(())
    }

    /// Geänderte Qualität melden (Zähler antwortet nicht mehr, Wert veraltet) und gerade
    /// eingeschaltete Datenpunkte zum ersten Mal senden. Liefert die Anzahl Nachrichten.
    pub fn outputs_refresh(&mut self) -> Result<usize> {
        if !OUTPUTS.iter().any(|k| self.output_on(k)) {
            return Ok(0);
        }
        let mut n = 0;
        for v in self.dp_list(None, None)? {
            let quality = v["quality"].as_str().unwrap_or("none").to_string();
            let Some(value) = v["last_value"].as_f64() else {
                continue;
            };
            if quality == "none" {
                continue;
            }
            let mut d: Datapoint = serde_json::from_value(v.clone())?;
            d.id = v["id"].as_i64().unwrap_or(0);
            let unsure: bool = self.conn.query_row(
                "SELECT last_unsure FROM datapoints WHERE id = ?1",
                [d.id],
                |r| r.get(0),
            )?;
            let day = v["last_time"]
                .as_str()
                .unwrap_or("")
                .get(..10)
                .unwrap_or("")
                .to_string();
            let s = Sample {
                value,
                quality: &quality,
                utc: v["last_utc"]
                    .as_i64()
                    .unwrap_or_else(crate::clock::utc_secs),
                unsure,
                day: &day,
            };
            for (key, o) in &d.outputs {
                let Some(address) = o.address.as_deref().filter(|_| o.enabled && d.active) else {
                    continue;
                };
                if !self.output_on(key) {
                    continue;
                }
                if self.sent(d.id, key)?.quality.as_deref() != Some(quality.as_str()) {
                    self.send_to(key, &d, address, &s)?;
                    n += 1;
                }
            }
        }
        Ok(n)
    }
}
