//! Datenpunkte als CSV exportieren und mit Vorschau wieder einlesen

use super::*;

/// Spalten der CSV (Reihenfolge fest, Kopfzeile wird beim Import übersprungen)
pub(super) const CSV_HEAD_DE: &str =
    "Nr;Linie;Zähler;Adresse;Kennung;Name;Einheit;Faktor;Weitergabe;Aktiv;MQTT;Topic";

pub(super) const CSV_HEAD_EN: &str =
    "No;Line;Meter;Address;Key;Name;Unit;Factor;Send;Active;MQTT;Topic";

pub(super) fn parse_num(s: &str) -> Option<f64> {
    // Excel mit deutschem Gebietsschema schreibt Komma
    s.trim().replace(',', ".").replace('\'', "").parse().ok()
}

pub(super) fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "1" | "ja" | "yes" | "x" | "true" | "wahr" | "✓" => Some(true),
        "0" | "nein" | "no" | "" | "false" | "falsch" | "-" => Some(false),
        _ => None,
    }
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

impl Db {
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
            out.push_str(&crate::export::row(&row));
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
            let c = crate::export::split_line(line);
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
    pub(super) fn dp_list_meters(&self, project_id: i64) -> Result<Vec<(String, Meter)>> {
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
}
