//! Home Assistant über MQTT Discovery (Konzept Home Assistant): je aktivem Datenpunkt mit
//! MQTT-Häkchen eine retained Konfig-Nachricht, damit Home Assistant den Sensor von selbst anlegt.
//! Der Wert selbst kommt aus der normalen MQTT-Nachricht des Datenpunkts.

use std::collections::BTreeMap;

use super::mqtt::Mqtt;
use super::*;

/// Wasser im Sinne von Home Assistant (Energie-Dashboard „Wasser“)
const WATER: [&str; 5] = [
    "medium_water",
    "medium_warm_water",
    "medium_hot_water",
    "medium_cold_water",
    "medium_dual_water",
];

/// Typ in Home Assistant, nur wenn die Einheit dazu passt (sonst lehnt Home Assistant den Sensor ab)
fn device_class(quantity: &str, unit: &str, medium: &str) -> Option<&'static str> {
    let (class, units): (&str, &[&str]) = match quantity {
        "energy" | "energy_cooling" => ("energy", &["Wh", "kWh", "MWh", "kJ", "MJ", "GJ"]),
        "volume" if WATER.contains(&medium) => ("water", &["m³", "L"]),
        "volume" if medium == "medium_gas" => ("gas", &["m³", "L"]),
        "volume" => ("volume", &["m³", "L"]),
        "power" => ("power", &["W", "kW", "MW"]),
        "volume_flow" => ("volume_flow_rate", &["m³/h", "L/h", "L/min"]),
        "flow_temperature" | "return_temperature" | "external_temperature" => {
            ("temperature", &["°C"])
        }
        "pressure" => ("pressure", &["bar"]),
        "voltage" => ("voltage", &["V"]),
        "current" => ("current", &["A"]),
        "mass" => ("weight", &["kg"]),
        "on_time" | "operating_time" => ("duration", &["s", "min", "h", "d"]),
        _ => return None,
    };
    units.contains(&unit).then_some(class)
}

/// Zählerstände und Betriebszeiten steigen; als Speicherwert (Stichtag …) nur „Summe“
fn state_class(key: &str) -> &'static str {
    let Some(p) = parts(key) else {
        return "measurement";
    };
    let rising =
        COUNTERS.contains(&p.quantity) || matches!(p.quantity, "on_time" | "operating_time");
    match (rising, p.storage == "0" && p.function == "instantaneous") {
        (true, true) => "total_increasing",
        (true, false) => "total",
        _ => "measurement",
    }
}

/// Konfig-Nachricht für einen Datenpunkt
fn config(db: &Db, node: &str, p: &Place, d: &Datapoint, address: &str) -> Value {
    let m = &p.meter;
    let state = Mqtt.full_address(db, address);
    let quantity = parts(&d.key).map(|k| k.quantity).unwrap_or("");
    // Home Assistant setzt den Gerätenamen (Zähler) davor: „Whg 1 · Energie“ → „Energie“
    let name = d
        .name
        .strip_prefix(&format!("{} · ", meter_label(m)))
        .unwrap_or(&d.name);
    let mut v = json!({
        "name": name,
        "unique_id": format!("{node}-p{}-dp{}", p.project_id, d.no),
        "state_topic": state,
        "value_template": "{{ value_json.value }}",
        "json_attributes_topic": state,
        "state_class": state_class(&d.key),
        "availability_mode": "all",
        "availability": [
            { "topic": Mqtt.full_address(db, "status") },
            {
                "topic": state,
                "value_template": "{{ 'online' if value_json.quality in ['good', 'stale'] else 'offline' }}",
            },
        ],
        "device": {
            "identifiers": [format!("{node}-m{}", m.id)],
            "name": meter_label(m),
            "manufacturer": m.manufacturer,
            "serial_number": crate::meterlist::meter_ident(m),
        },
        "origin": {
            "name": "mbusio",
            "sw_version": env!("CARGO_PKG_VERSION"),
            "support_url": "https://github.com/Wobi848/mbusio",
        },
    });
    if !d.unit.is_empty() {
        v["unit_of_measurement"] = json!(d.unit);
    }
    if let Some(class) = device_class(quantity, &d.unit, m.medium.as_deref().unwrap_or("")) {
        v["device_class"] = json!(class);
    }
    v
}

/// Gewünschte Konfig-Nachrichten: Topic → Nachricht (leer, wenn Home Assistant aus ist)
fn wanted(db: &Db) -> Result<BTreeMap<String, String>> {
    let cfg = db.mqtt_config();
    let mut out = BTreeMap::new();
    if !cfg.ha {
        return Ok(out);
    }
    let node = crate::mqtt::slug(&cfg.client_id());
    let prefix = cfg.ha_prefix.trim();
    let list: Vec<(i64, String)> = {
        let mut st = db.conn.prepare(
            "SELECT dp_outputs.dp, dp_outputs.address FROM dp_outputs
             JOIN datapoints ON datapoints.id = dp_outputs.dp
             WHERE dp_outputs.output = 'mqtt' AND dp_outputs.enabled = 1
               AND dp_outputs.address IS NOT NULL AND datapoints.active = 1",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (dp, address) in list {
        let d = db.dp(dp)?;
        let p = db.dp_place(dp)?;
        let topic = format!("{prefix}/sensor/{node}/p{}-dp{}/config", p.project_id, d.no);
        out.insert(topic, config(db, &node, &p, &d, &address).to_string());
    }
    Ok(out)
}

/// Konfig-Nachrichten abgleichen: neue und geänderte senden, nicht mehr gewünschte mit leerer
/// Nachricht entfernen. Liefert die Anzahl Nachrichten.
pub(super) fn sync(db: &mut Db) -> Result<usize> {
    let wanted = wanted(db)?;
    let sent: BTreeMap<String, String> = {
        let mut st = db.conn.prepare("SELECT topic, payload FROM mqtt_ha")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut n = 0;
    db.tx(|db| {
        for (topic, payload) in &wanted {
            if sent.get(topic) != Some(payload) {
                db.mqtt_enqueue_abs(topic, payload)?;
                db.conn.execute(
                    "INSERT INTO mqtt_ha (topic, payload) VALUES (?1, ?2)
                     ON CONFLICT(topic) DO UPDATE SET payload = excluded.payload",
                    params![topic, payload],
                )?;
                n += 1;
            }
        }
        for topic in sent.keys().filter(|t| !wanted.contains_key(*t)) {
            db.mqtt_enqueue_abs(topic, "")?;
            db.conn
                .execute("DELETE FROM mqtt_ha WHERE topic = ?1", [topic])?;
            n += 1;
        }
        Ok(())
    })?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_class_only_with_matching_unit() {
        assert_eq!(device_class("energy", "kWh", ""), Some("energy"));
        assert_eq!(device_class("energy", "kWh/h", ""), None);
        assert_eq!(device_class("volume", "m³", "medium_water"), Some("water"));
        assert_eq!(device_class("volume", "m³", "medium_gas"), Some("gas"));
        assert_eq!(
            device_class("volume", "m³", "medium_heat_outlet"),
            Some("volume")
        );
        assert_eq!(device_class("volume", "l", "medium_water"), None);
        assert_eq!(
            device_class("flow_temperature", "°C", ""),
            Some("temperature")
        );
        assert_eq!(device_class("temperature_difference", "K", ""), None);
        assert_eq!(device_class("hca_units", "", ""), None);
    }

    #[test]
    fn state_class_from_key() {
        assert_eq!(
            state_class("instantaneous|0|0|0|energy|kWh"),
            "total_increasing"
        );
        assert_eq!(state_class("instantaneous|1|0|0|energy|kWh"), "total");
        assert_eq!(state_class("maximum|0|0|0|volume|m³"), "total");
        assert_eq!(
            state_class("instantaneous|0|0|0|operating_time|h"),
            "total_increasing"
        );
        assert_eq!(state_class("instantaneous|0|0|0|power|kW"), "measurement");
    }
}
