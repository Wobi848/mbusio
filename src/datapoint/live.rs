//! Aktueller Stand für Empfänger, die mitlaufen statt abzufragen (Node-RED, Konzept Node-RED):
//! der Baum zur Auswahl und je Projekt die Werte, dazu was ein Empfänger seit seinem letzten Stand
//! bekommen muss. Es gilt dieselbe Weitergabe-Regel wie bei den Ausgängen.

use std::collections::BTreeMap;

use super::outputs::{should_send, Sample, Sent};
use super::*;

/// Wert eines Datenpunkts, wie ihn ein Empfänger bekommt
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveValue {
    pub no: i64,
    pub value: f64,
    pub unit: String,
    /// Zeitpunkt der Auslesung, UTC (RFC 3339)
    pub time: String,
    pub quality: String,
    #[serde(skip)]
    send: String,
    /// Tag der Auslesung (Ortszeit), für „1× täglich“
    #[serde(skip)]
    day: String,
}

/// Welcher Teil eines Projekts: alles, eine Linie oder ein Zähler
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Scope {
    pub project: i64,
    #[serde(default)]
    pub line: Option<i64>,
    #[serde(default)]
    pub meter: Option<i64>,
}

/// Stand eines Projektteils: Aufbau (Linien, Zähler, Datenpunkte, Namen) und die Werte der
/// Linien im Dauerbetrieb
#[derive(Debug, Default)]
pub struct LiveState {
    shape: Vec<String>,
    pub values: BTreeMap<i64, LiveValue>,
}

impl LiveState {
    /// Was ein Empfänger bekommen muss, der zuletzt `old` hatte: ob sich der Aufbau geändert hat,
    /// und neue Werte nach der Weitergabe-Art des Datenpunkts (geänderte Qualität immer)
    pub fn since(&self, old: &LiveState) -> (bool, Vec<&LiveValue>) {
        let values = self
            .values
            .iter()
            .filter(|(no, v)| match old.values.get(no) {
                None => true,
                Some(o) => is_new(o, v),
            })
            .map(|(_, v)| v)
            .collect();
        (self.shape != old.shape, values)
    }
}

/// Neue Auslesung oder geänderte Qualität, und laut Weitergabe-Art zu senden?
fn is_new(old: &LiveValue, v: &LiveValue) -> bool {
    if old.time == v.time && old.value == v.value && old.quality == v.quality {
        return false;
    }
    let last = Sent {
        value: Some(old.value),
        quality: Some(old.quality.clone()),
        day: Some(old.day.clone()),
    };
    let s = Sample {
        value: v.value,
        quality: &v.quality,
        utc: 0,
        unsure: false,
        day: &v.day,
    };
    should_send(&v.send, &last, &s)
}

fn utc_text(utc: Option<i64>) -> String {
    utc.and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

impl Db {
    /// Baum zur Auswahl: Projekt › Gateway › Linie › Zähler › aktive Datenpunkte mit aktuellem Wert
    pub fn live_tree(&self) -> Result<Value> {
        let mut projects = Vec::new();
        for p in self.tree()? {
            let mut gateways = Vec::new();
            for g in &p.gateways {
                let mut lines = Vec::new();
                for l in &g.lines {
                    let meters: Vec<Value> = self
                        .dp_by_meters(Some(l.line.id), None)?
                        .into_iter()
                        .filter_map(|(_, m, dps)| {
                            let dps: Vec<Value> = dps
                                .iter()
                                .filter(|d| d["active"] == true)
                                .map(|d| {
                                    json!({
                                        "no": d["no"], "name": d["name"], "unit": d["unit"],
                                        "value": d["last_value"], "time": utc_text(d["last_utc"].as_i64()),
                                        "quality": d["quality"],
                                    })
                                })
                                .collect();
                            (!dps.is_empty()).then(|| {
                                json!({
                                    "id": m.id, "name": meter_label(&m), "medium": m.medium,
                                    "ident": crate::meterlist::meter_ident(&m), "pa": m.primary_address,
                                    "datapoints": dps,
                                })
                            })
                        })
                        .collect();
                    lines.push(json!({
                        "id": l.line.id, "name": l.line.name, "operation": l.line.operation,
                        "meters": meters,
                    }));
                }
                gateways
                    .push(json!({ "id": g.gateway.id, "name": g.gateway.name, "lines": lines }));
            }
            projects
                .push(json!({ "id": p.project.id, "name": p.project.name, "gateways": gateways }));
        }
        Ok(json!(projects))
    }

    /// Stand eines Projektteils; Werte nur aus Linien im Dauerbetrieb (wie bei den Ausgängen)
    pub fn live_state(&self, scope: Scope) -> Result<LiveState> {
        self.project(scope.project)?;
        let mut st = LiveState::default();
        for (line, m, dps) in self.dp_by_meters(scope.line, Some(scope.project))? {
            if scope.meter.is_some_and(|id| id != m.id) {
                continue;
            }
            for d in dps.iter().filter(|d| d["active"] == true) {
                st.shape.push(format!(
                    "{line}|{}|{}|{}|{}",
                    meter_label(&m),
                    d["no"],
                    d["name"],
                    d["unit"]
                ));
                let quality = d["quality"].as_str().unwrap_or("none");
                let (Some(no), Some(value)) = (d["no"].as_i64(), d["last_value"].as_f64()) else {
                    continue;
                };
                if d["operation"] != true || quality == "none" {
                    continue;
                }
                st.values.insert(
                    no,
                    LiveValue {
                        no,
                        value,
                        unit: d["unit"].as_str().unwrap_or("").into(),
                        time: utc_text(d["last_utc"].as_i64()),
                        quality: quality.into(),
                        send: d["send"].as_str().unwrap_or("every").into(),
                        day: d["last_time"]
                            .as_str()
                            .and_then(|t| t.get(..10))
                            .unwrap_or("")
                            .into(),
                    },
                );
            }
        }
        Ok(st)
    }
}
