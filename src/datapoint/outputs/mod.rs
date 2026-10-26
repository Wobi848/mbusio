//! Ausgänge (MQTT, Modbus …): je Datenpunkt und Ausgang ein/aus und eine Adresse (Topic,
//! Register …), gespeichert in `dp_outputs` (Konzept Ausgänge).
//!
//! Was für alle Ausgänge gleich ist: hier die Schnittstelle [`Output`], die Liste [`OUTPUTS`] und
//! die Liste für den Integrator; `addresses` (Adressen vorschlagen, eindeutig halten, nach Muster
//! setzen), `send` (wann gesendet wird, geänderte Qualität, zuletzt gesendet). Was ein Ausgang
//! anders macht, steht in seinem Modul (mqtt, home_assistant, modbus). Ein neuer Ausgang ist ein
//! neues Modul und ein Eintrag in [`OUTPUTS`].

use std::collections::BTreeMap;

use super::*;

mod addresses;
mod home_assistant;
mod log;
mod modbus;
mod mqtt;
mod send;

pub use log::{sample_of, Entry, LogFilter, LogRow};
pub(in crate::datapoint) use send::{should_send, Sent};

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
}
