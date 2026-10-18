//! Weitergabe per MQTT: Topic je Datenpunkt, Nutzlast, Meldung geänderter Qualität

use super::*;

/// Nutzlast für MQTT: Wert, Einheit, Zeit (UTC), Qualität, Name, Nummer
pub(super) fn mqtt_payload(
    d: &Datapoint,
    value: f64,
    quality: &str,
    utc: i64,
    time_unsure: bool,
) -> String {
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
pub(super) fn key_slug(key: &str) -> String {
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

impl Db {
    /// Vorgeschlagenes Topic (relativ): projekt/gateway-linie/zaehler/wert; eindeutig gemacht
    pub(super) fn dp_default_topic(&self, id: i64) -> Result<String> {
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
    pub(super) fn dp_set_mqtt(&mut self, id: i64, on: bool) -> Result<()> {
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
}
