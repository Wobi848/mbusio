//! Ausgänge (MQTT, später Modbus …): je Datenpunkt und Ausgang ein/aus und eine Adresse (Topic,
//! Register …), gespeichert in `dp_outputs` (Konzept Ausgänge).
//!
//! Hier steht nur, was für alle Ausgänge gleich ist: wann gesendet wird (jede Auslesung, bei
//! Änderung, täglich), dass geänderte Qualität gemeldet wird, was zuletzt gesendet wurde,
//! Adressen vorschlagen und nach Muster neu setzen, die Liste für den Integrator. Was ein Ausgang
//! anders macht, steht in seinem Modul hinter der Schnittstelle [`Output`]. Ein neuer Ausgang ist
//! ein neues Modul und ein Eintrag in [`OUTPUTS`].

use std::collections::{BTreeMap, HashSet};

use crate::changeset::ChangeSet;

use super::*;

mod home_assistant;
mod modbus;
mod mqtt;

/// Was ein Ausgang selbst können muss; alles Gemeinsame steht in diesem Modul
pub trait Output: Sync {
    /// Schlüssel, so gespeichert (Datenbank, CSV, Export, API): nie ändern
    fn key(&self) -> &'static str;
    /// Name für Oberfläche und CSV
    fn label(&self) -> String;
    /// Wie die Adresse beim Ausgang heisst (Spalte in Oberfläche und CSV)
    fn address_label(&self) -> String;
    fn check_address(&self, address: &str) -> Result<()>;
    /// Adresse nach dem Muster der Einstellungen (noch nicht eindeutig gemacht)
    fn pattern_address(&self, db: &Db, place: &Place, d: &Datapoint) -> Result<String>;
    /// Ist der Ausgang als Ganzes eingeschaltet (z.B. MQTT mit Broker)?
    fn is_on(&self, db: &Db) -> bool;
    /// Einen Wert zustellen (z.B. in die Warteschlange für den Broker)
    fn deliver(&self, db: &mut Db, d: &Datapoint, address: &str, s: &Sample) -> Result<()>;
    /// Müssen Adressen über alle Projekte eindeutig sein (MQTT: ein Broker) oder nur im Projekt
    /// (Modbus: ein Server je Projekt)?
    fn global_addresses(&self) -> bool {
        true
    }
    /// Adresse, wie sie ein Leitsystem sieht (MQTT: mit Basis-Topic)
    fn full_address(&self, _db: &Db, address: &str) -> String {
        address.to_string()
    }
    /// Was ein Leitsystem zusätzlich zur Liste der Datenpunkte wissen muss (ohne Zugangsdaten)
    fn list_info(&self, _db: &Db) -> Option<Value> {
        None
    }
    /// Eigene Zusatznachrichten nachführen (MQTT: Konfiguration für Home Assistant); läuft bei
    /// jedem Abgleich, solange der Ausgang an ist. Liefert die Anzahl Nachrichten.
    fn sync(&self, _db: &mut Db) -> Result<usize> {
        Ok(0)
    }
}

/// Alle Ausgänge, in der Reihenfolge der Oberfläche
pub static OUTPUTS: [&dyn Output; 2] = [&mqtt::Mqtt, &modbus::Modbus];

/// Ausgang zu einem Schlüssel
pub fn output(key: &str) -> Result<&'static dyn Output> {
    OUTPUTS
        .iter()
        .copied()
        .find(|o| o.key() == key)
        .ok_or_else(|| anyhow::anyhow!("{}: {key}", tr("Unbekannter Ausgang", "Unknown output")))
}

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
pub struct Sample<'a> {
    pub value: f64,
    pub quality: &'a str,
    pub utc: i64,
    pub unsure: bool,
    /// Tag der Auslesung (Ortszeit), für „1× täglich“
    pub day: &'a str,
}

/// Wo ein Datenpunkt liegt: für Adressen und die Liste des Ausgangs
pub struct Place {
    pub project_id: i64,
    pub project: String,
    pub gateway: String,
    pub line: String,
    pub meter: Meter,
}

/// Soll gesendet werden? Nach der Weitergabe-Art des Datenpunkts; geänderte Qualität immer
pub(super) fn should_send(mode: &str, last: &Sent, s: &Sample) -> bool {
    let quality_changed = last.quality.as_deref() != Some(s.quality);
    match mode {
        "change" => last.value != Some(s.value) || quality_changed,
        "daily" => last.day.as_deref() != Some(s.day) || quality_changed,
        _ => true,
    }
}

/// Was an einen Ausgang zuletzt gesendet wurde
#[derive(Default)]
pub(super) struct Sent {
    pub value: Option<f64>,
    pub quality: Option<String>,
    pub day: Option<String>,
}

impl Db {
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

    /// Projekt, Gateway, Linie und Zähler eines Datenpunkts
    fn dp_place(&self, dp_id: i64) -> Result<Place> {
        let (project_id, project, gateway, line, meter_id): (i64, String, String, String, i64) = self.conn.query_row(
            "SELECT projects.id, projects.name, gateways.name, lines.name, meters.id FROM datapoints
             JOIN meters ON meters.id = datapoints.meter_id JOIN lines ON lines.id = meters.line_id
             JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
             WHERE datapoints.id = ?1",
            [dp_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        Ok(Place {
            project_id,
            project,
            gateway,
            line,
            meter: self.meter(meter_id)?,
        })
    }

    /// Adresse eines Datenpunkts an einem Ausgang nach dem Muster (noch nicht eindeutig gemacht)
    fn pattern_address(&self, key: &str, dp_id: i64) -> Result<String> {
        output(key)?.pattern_address(self, &self.dp_place(dp_id)?, &self.dp(dp_id)?)
    }

    /// Projekt eines Datenpunkts
    fn dp_project(&self, dp_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT gateways.project_id FROM datapoints JOIN meters ON meters.id = datapoints.meter_id
             JOIN lines ON lines.id = meters.line_id JOIN gateways ON gateways.id = lines.gateway_id
             WHERE datapoints.id = ?1",
            [dp_id],
            |r| r.get(0),
        )?)
    }

    /// Anderer Datenpunkt mit derselben Adresse an diesem Ausgang, über alle Projekte oder nur im
    /// Projekt (siehe `Output::global_addresses`); `enabled`: nur eingeschaltete zählen
    fn address_owner(
        &self,
        key: &str,
        address: &str,
        dp_id: i64,
        enabled: bool,
    ) -> Result<Option<i64>> {
        // None = alle Projekte
        let project = if output(key)?.global_addresses() {
            None
        } else {
            Some(self.dp_project(dp_id)?)
        };
        Ok(self
            .conn
            .query_row(
                "SELECT dp_outputs.dp FROM dp_outputs JOIN datapoints ON datapoints.id = dp_outputs.dp
                   JOIN meters ON meters.id = datapoints.meter_id JOIN lines ON lines.id = meters.line_id
                   JOIN gateways ON gateways.id = lines.gateway_id
                 WHERE dp_outputs.output = ?1 AND dp_outputs.address = ?2 AND dp_outputs.dp != ?3
                   AND (?4 IS NULL OR gateways.project_id = ?4) AND (?5 = 0 OR dp_outputs.enabled = 1)
                 LIMIT 1",
                params![key, address, dp_id, project, enabled],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Eingeschaltete Ausgänge dieser Datenpunkte: keine Adresse doppelt (sonst würde bei Modbus
    /// ein Wert den anderen überschreiben, bei MQTT zwei Werte dasselbe Topic teilen)
    pub(super) fn check_unique_addresses(&self, dp_ids: &[i64]) -> Result<()> {
        for &id in dp_ids {
            let d = self.dp(id)?;
            for (key, o) in d.outputs.iter().filter(|(_, o)| o.enabled) {
                let Some(a) = o.address.as_deref() else {
                    continue;
                };
                if let Some(other) = self.address_owner(key, a, id, true)? {
                    let other = self.dp(other)?;
                    anyhow::bail!(
                        "{} „{a}“ {} {} · {}",
                        output(key)?.address_label(),
                        tr("hat schon Datenpunkt", "already used by datapoint"),
                        other.no,
                        other.name
                    );
                }
            }
        }
        Ok(())
    }

    /// Vorschlag für die Adresse eines Datenpunkts an einem Ausgang, eindeutig gemacht
    fn default_address(&self, key: &str, dp_id: i64) -> Result<String> {
        let address = self.pattern_address(key, dp_id)?;
        let taken = self.address_owner(key, &address, dp_id, false)?.is_some();
        Ok(if taken {
            format!("{address}-{}", self.dp(dp_id)?.no)
        } else {
            address
        })
    }

    /// Datenpunkte eines Projekts, die einen Ausgang haben (eingeschaltet oder mit Adresse)
    fn dps_with_output(
        &self,
        project_id: i64,
        key: &str,
    ) -> Result<Vec<(i64, Option<String>, bool)>> {
        let mut st = self.conn.prepare(
            "SELECT dp_outputs.dp, dp_outputs.address, dp_outputs.enabled FROM dp_outputs
             JOIN datapoints ON datapoints.id = dp_outputs.dp JOIN meters ON meters.id = datapoints.meter_id
             JOIN lines ON lines.id = meters.line_id JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 AND dp_outputs.output = ?2 ORDER BY datapoints.no",
        )?;
        let rows = st.query_map(params![project_id, key], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Alle Adressen eines Projekts an einem Ausgang nach dem Muster neu setzen: zuerst nur zeigen,
    /// was sich ändert (`apply` = false). Doppelte werden mit der Nummer des Datenpunkts eindeutig.
    pub fn dp_readdress(&mut self, project_id: i64, key: &str, apply: bool) -> Result<ChangeSet> {
        let o = output(key)?;
        let address_label = o.address_label();
        let list = self.dps_with_output(project_id, key)?;
        let mine: HashSet<i64> = list.iter().map(|(dp, ..)| *dp).collect();
        // Adressen anderer Projekte bleiben belegt, wenn sie global eindeutig sein müssen
        let mut used: HashSet<String> = if !o.global_addresses() {
            HashSet::new()
        } else {
            let mut st = self.conn.prepare(
                "SELECT dp, address FROM dp_outputs WHERE output = ?1 AND address IS NOT NULL",
            )?;
            let rows =
                st.query_map([key], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(|r| r.ok())
                .filter(|(dp, _)| !mine.contains(dp))
                .map(|(_, a)| a)
                .collect()
        };
        let mut cs = ChangeSet::default();
        let mut updates = Vec::new();
        for (dp, old, _) in &list {
            let mut new = self.pattern_address(key, *dp)?;
            if used.contains(&new) {
                new = format!("{new}-{}", self.dp(*dp)?.no);
            }
            used.insert(new.clone());
            let d = self.dp(*dp)?;
            let item = format!("{} · {}", d.no, d.name);
            if cs.change(
                &item,
                address_label.clone(),
                old.as_deref().unwrap_or(""),
                &new,
            ) {
                updates.push((*dp, new));
            } else {
                cs.unchanged += 1;
            }
        }
        let key = key.to_string();
        cs.finish(apply, || {
            self.tx(|db| {
                for (dp, a) in &updates {
                    db.dp_set_output_address(*dp, &key, Some(a))?;
                }
                Ok(())
            })
        })
    }

    /// Liste für den Integrator: eingeschaltete Datenpunkte eines Ausgangs mit voller Adresse
    /// und allem, was man zum Einbinden braucht
    pub fn output_list(&self, project_id: i64, key: &str, lang: Lang) -> Result<Vec<Value>> {
        let o = output(key)?;
        let mut out = Vec::new();
        for (dp, address, enabled) in self.dps_with_output(project_id, key)? {
            let Some(address) = address.filter(|_| enabled) else {
                continue;
            };
            let d = self.dp(dp)?;
            let Place {
                gateway,
                line,
                meter: m,
                ..
            } = self.dp_place(dp)?;
            out.push(json!({
                "no": d.no,
                "address": o.full_address(self, &address),
                "name": d.name,
                "unit": d.unit,
                "factor": d.factor,
                "send": d.send,
                "meter": meter_label(&m),
                "id": crate::meterlist::meter_ident(&m),
                "pa": m.primary_address,
                "location": m.location,
                "medium": m.medium.as_deref().map(|k| crate::i18n::label(k, lang)),
                "line": format!("{gateway} › {line}"),
                "key": d.key,
            }));
        }
        Ok(out)
    }

    /// Ausgang für einen Datenpunkt ein- oder ausschalten; beim Einschalten eine Adresse
    /// vorschlagen (falls noch keine) und den aktuellen Wert beim nächsten Durchgang senden
    pub(super) fn dp_set_output(&mut self, dp_id: i64, key: &str, on: bool) -> Result<()> {
        output(key)?;
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
        output(key)?.check_address(a)?;
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
    fn send_to(&mut self, o: &dyn Output, d: &Datapoint, address: &str, s: &Sample) -> Result<()> {
        o.deliver(self, d, address, s)?;
        self.conn.execute(
            "UPDATE dp_outputs SET sent_value = ?3, sent_quality = ?4, sent_day = ?5
             WHERE dp = ?1 AND output = ?2",
            params![d.id, o.key(), s.value, s.quality, s.day],
        )?;
        Ok(())
    }

    /// Eingeschaltete Ausgänge eines Datenpunkts mit ihrer Adresse (nur Ausgänge, die als Ganzes
    /// an sind)
    fn active_outputs<'a>(&self, d: &'a Datapoint) -> Vec<(&'static dyn Output, &'a str)> {
        if !d.active {
            return Vec::new();
        }
        OUTPUTS
            .iter()
            .filter_map(|o| {
                let x = d.outputs.get(o.key())?;
                let address = x.address.as_deref().filter(|_| x.enabled)?;
                o.is_on(self).then_some((*o, address))
            })
            .collect()
    }

    /// Neuer Wert eines Datenpunkts: an alle eingeschalteten Ausgänge, wie eingestellt
    pub(super) fn dispatch(&mut self, d: &Datapoint, s: &Sample) -> Result<()> {
        for (o, address) in self.active_outputs(d) {
            if should_send(&d.send, &self.sent(d.id, o.key())?, s) {
                self.send_to(o, d, address, s)?;
            }
        }
        Ok(())
    }

    /// Geänderte Qualität melden (Zähler antwortet nicht mehr, Wert veraltet), gerade
    /// eingeschaltete Datenpunkte zum ersten Mal senden und die Zusatznachrichten der Ausgänge
    /// nachführen. Liefert die Anzahl Nachrichten.
    pub fn outputs_refresh(&mut self) -> Result<usize> {
        // Qualität kann sich mit der Zeit ändern (veraltet), Einstellungen ebenso
        self.notify();
        let on: Vec<&dyn Output> = OUTPUTS.iter().copied().filter(|o| o.is_on(self)).collect();
        if on.is_empty() {
            return Ok(0);
        }
        let mut n = 0;
        for o in on {
            n += o.sync(self)?;
        }
        for v in self.dp_list(None, None)? {
            // nur Linien im Dauerbetrieb melden ihre Qualität
            if v["operation"] != true {
                continue;
            }
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
            for (o, address) in self.active_outputs(&d) {
                if self.sent(d.id, o.key())?.quality.as_deref() != Some(quality.as_str()) {
                    self.send_to(o, &d, address, &s)?;
                    n += 1;
                }
            }
        }
        Ok(n)
    }
}
