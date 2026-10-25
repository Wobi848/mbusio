//! Datenpunkte und Vorlagen aus einem Projekt-Export übernehmen (auch ältere Dateien).

use super::*;

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

impl Db {
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
