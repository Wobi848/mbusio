//! Ausgang MQTT: wie Datenpunkte zu Topics und Nachrichten werden. Verbindung, Warteschlange und
//! Last Will stehen in `crate::mqtt`.

use super::*;

pub struct Mqtt;

/// Nutzlast: Wert, Einheit, Zeit (UTC), Qualität, Name, Nummer
fn payload(d: &Datapoint, s: &Sample) -> String {
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

fn base(db: &Db) -> String {
    db.mqtt_config()
        .base
        .trim()
        .trim_end_matches('/')
        .to_string()
}

impl Output for Mqtt {
    fn key(&self) -> &'static str {
        "mqtt"
    }

    fn label(&self) -> String {
        "MQTT".into()
    }

    fn address_label(&self) -> String {
        "Topic".into()
    }

    fn check_address(&self, address: &str) -> Result<()> {
        crate::mqtt::check_topic(address)
    }

    fn pattern_address(&self, db: &Db, p: &Place, d: &Datapoint) -> Result<String> {
        let m = &p.meter;
        let vars = [
            ("projekt", p.project.clone()),
            ("gateway", p.gateway.clone()),
            ("linie", p.line.clone()),
            ("zaehler", meter_label(m)),
            ("id", crate::meterlist::meter_ident(m).unwrap_or_default()),
            (
                "pa",
                m.primary_address.map(|p| p.to_string()).unwrap_or_default(),
            ),
            ("ort", m.location.clone()),
            // sprachunabhängig wie die Grösse: heat-outlet, water …
            (
                "medium",
                m.medium
                    .as_deref()
                    .unwrap_or("")
                    .trim_start_matches("medium_")
                    .to_string(),
            ),
            ("wert", key_slug(&d.key)),
            ("nr", d.no.to_string()),
        ];
        Ok(crate::mqtt::render_topic(&db.mqtt_config().topic, &vars))
    }

    fn is_on(&self, db: &Db) -> bool {
        db.mqtt_config().enabled
    }

    fn deliver(&self, db: &mut Db, d: &Datapoint, address: &str, s: &Sample) -> Result<()> {
        db.mqtt_enqueue(d.id, address, &payload(d, s))
    }

    fn full_address(&self, db: &Db, address: &str) -> String {
        let base = base(db);
        if base.is_empty() {
            address.to_string()
        } else {
            format!("{base}/{address}")
        }
    }

    fn sync(&self, db: &mut Db) -> Result<usize> {
        super::home_assistant::sync(db)
    }

    fn list_info(&self, db: &Db) -> Option<Value> {
        let base = base(db);
        Some(json!({
            "base": base,
            "status_topic": format!("{base}/status"),
            "status": "online / offline (Last Will)",
            "time_topic": if db.mqtt_config().time_sync { Some(format!("{base}/time")) } else { None },
            "qos": 1,
            "retained": true,
            "payload": {
                "value": tr("Zahl, mit Faktor umgerechnet", "number, factor applied"),
                "unit": tr("Einheit", "unit"),
                "time": tr("Zeitpunkt der Auslesung, UTC (RFC 3339)", "time of reading, UTC (RFC 3339)"),
                "quality": "good | stale | disturbed | invalid",
                "name": tr("Name des Datenpunkts", "name of the datapoint"),
                "no": tr("Nummer des Datenpunkts (bleibt gleich)", "number of the datapoint (stays the same)"),
                "time_unsure": tr("nur wenn die Uhrzeit nicht bestätigt war: true", "only if the clock was not confirmed: true"),
            },
        }))
    }
}
