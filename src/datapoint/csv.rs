//! Datenpunkte als CSV exportieren und mit Vorschau wieder einlesen.
//!
//! Feste Spalten `Nr;Linie;Zähler;Adresse;Kennung;Name;Einheit;Faktor;Weitergabe;Aktiv`, danach je
//! Ausgang zwei Spalten (z.B. `MQTT;Topic`). Fehlen die Spalten eines Ausgangs (ältere Datei),
//! bleibt dessen Einstellung, wie sie ist. Die Datei zerlegt die Oberfläche (`parseTable`).

use crate::changeset::ChangeSet;

use super::*;

const FIXED: usize = 10;

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

fn header(lang: Lang) -> Vec<String> {
    let fixed = if lang == Lang::De {
        "Nr;Linie;Zähler;Adresse;Kennung;Name;Einheit;Faktor;Weitergabe;Aktiv"
    } else {
        "No;Line;Meter;Address;Key;Name;Unit;Factor;Send;Active"
    };
    let mut h: Vec<String> = fixed.split(';').map(String::from).collect();
    for key in OUTPUTS {
        h.push(output_label(key));
        h.push(address_label(key));
    }
    h
}

impl Db {
    pub fn dp_csv(&self, project_id: i64, lang: Lang) -> Result<String> {
        let mut out = crate::export::row(&header(lang));
        for v in self.dp_list(None, Some(project_id))? {
            let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
            let flag = |b: bool| (if b { "1" } else { "0" }).to_string();
            let mut row = vec![
                v["no"].to_string(),
                s("line"),
                s("meter"),
                s("address"),
                s("key"),
                s("name"),
                s("unit"),
                v["factor"].as_f64().unwrap_or(1.0).to_string(),
                s("send"),
                flag(v["active"].as_bool().unwrap_or(false)),
            ];
            for key in OUTPUTS {
                let o = &v["outputs"][key];
                row.push(flag(o["enabled"].as_bool().unwrap_or(false)));
                row.push(o["address"].as_str().unwrap_or("").to_string());
            }
            out.push_str(&crate::export::row(&row));
        }
        Ok(out)
    }

    /// Zeilen der CSV einlesen: zuerst nur zeigen, was sich ändert (`apply` = false), dann übernehmen.
    /// Zuordnung über die Nummer; Zeilen ohne Nummer legen neue Datenpunkte an (Linie, Adresse,
    /// Kennung). Fehlende Zeilen löschen nichts.
    pub fn dp_import(
        &mut self,
        project_id: i64,
        rows: &[Vec<String>],
        apply: bool,
        lang: Lang,
    ) -> Result<ChangeSet> {
        let existing: HashMap<i64, Datapoint> = self
            .dp_list(None, Some(project_id))?
            .into_iter()
            .filter_map(|v| Some((v["no"].as_i64()?, serde_json::from_value(v).ok()?)))
            .collect();
        let mut cs = ChangeSet::default();
        let mut updates: Vec<(i64, Datapoint)> = Vec::new();
        let mut creates: Vec<(Meter, Datapoint)> = Vec::new();
        for (n, c) in rows.iter().enumerate() {
            let row = n + 1;
            let col = |i: usize| c.get(i).cloned().unwrap_or_default();
            if n == 0 && parse_num(&col(0)).is_none() && !col(0).is_empty() {
                continue; // Kopfzeile
            }
            if c.len() < FIXED {
                cs.row_error(
                    row,
                    tr(
                        "zu wenige Spalten (10 erwartet)",
                        "too few columns (10 expected)",
                    ),
                );
                continue;
            }
            let Some(factor) = parse_num(&col(7)).filter(|f| f.is_finite() && *f != 0.0) else {
                cs.row_error(
                    row,
                    format!("{} „{}“", tr("Faktor ungültig", "invalid factor"), col(7)),
                );
                continue;
            };
            let send = col(8).to_lowercase();
            if !SEND_MODES.contains(&send.as_str()) {
                cs.row_error(
                    row,
                    format!(
                        "{} „{}“ ({})",
                        tr("Weitergabe ungültig", "invalid send mode"),
                        col(8),
                        SEND_MODES.join("/")
                    ),
                );
                continue;
            }
            let Some(active) = parse_bool(&col(9)) else {
                cs.row_error(
                    row,
                    format!(
                        "{} „{}“",
                        tr("Aktiv ungültig (1/0)", "invalid active (1/0)"),
                        col(9)
                    ),
                );
                continue;
            };
            let no = parse_num(&col(0)).map(|x| x as i64);
            let old = no.and_then(|no| existing.get(&no));
            // Ausgänge: je zwei Spalten; fehlen sie, bleibt es wie bisher
            let mut outputs = old.map(|d| d.outputs.clone()).unwrap_or_default();
            let mut bad = None;
            for (k, key) in OUTPUTS.iter().enumerate() {
                let i = FIXED + 2 * k;
                if c.len() <= i {
                    continue;
                }
                let Some(on) = parse_bool(&col(i)) else {
                    bad = Some(format!("{} „{}“", output_label(key), col(i)));
                    break;
                };
                let address = Some(col(i + 1)).filter(|a| !a.is_empty());
                if let Some(a) = &address {
                    if let Err(e) = outputs::check_address(key, a) {
                        bad = Some(format!("{e:#}"));
                        break;
                    }
                }
                let o = outputs.entry(key.to_string()).or_default();
                o.enabled = on;
                // leere Adresse in der Datei: vorhandene behalten
                if address.is_some() {
                    o.address = address;
                }
            }
            if let Some(e) = bad {
                cs.row_error(row, e);
                continue;
            }
            let wanted = Datapoint {
                key: col(4),
                name: col(5),
                unit: col(6),
                factor,
                send,
                active,
                outputs,
                ..Default::default()
            };
            if wanted.name.is_empty() {
                cs.row_error(row, tr("Name fehlt", "name missing"));
                continue;
            }
            match (no, old) {
                (Some(no), None) => cs.row_error(
                    row,
                    format!(
                        "{} {no} {}",
                        tr("Nr.", "No."),
                        tr("gibt es im Projekt nicht", "does not exist in the project")
                    ),
                ),
                (Some(no), Some(old)) => {
                    let item = format!("{no} · {}", old.name);
                    let flag = |b: bool| (b as u8).to_string();
                    let mut changed = cs.change(&item, tr("Name", "Name"), &old.name, &wanted.name);
                    changed |= cs.change(&item, tr("Einheit", "Unit"), &old.unit, &wanted.unit);
                    changed |= cs.change(
                        &item,
                        tr("Faktor", "Factor"),
                        &old.factor.to_string(),
                        &wanted.factor.to_string(),
                    );
                    changed |= cs.change(&item, tr("Weitergabe", "Send"), &old.send, &wanted.send);
                    changed |= cs.change(
                        &item,
                        tr("Aktiv", "Active"),
                        &flag(old.active),
                        &flag(wanted.active),
                    );
                    for key in OUTPUTS {
                        let (a, b) = (
                            old.outputs.get(key).cloned().unwrap_or_default(),
                            wanted.outputs.get(key).cloned().unwrap_or_default(),
                        );
                        changed |=
                            cs.change(&item, output_label(key), &flag(a.enabled), &flag(b.enabled));
                        changed |= cs.change(
                            &item,
                            address_label(key),
                            a.address.as_deref().unwrap_or(""),
                            b.address.as_deref().unwrap_or(""),
                        );
                    }
                    if changed {
                        updates.push((old.id, wanted));
                    } else {
                        cs.unchanged += 1;
                    }
                }
                (None, _) => {
                    // neu: Zähler über Linie und Adresse finden
                    let line = col(1);
                    let address = col(3).to_ascii_uppercase();
                    let meter = self
                        .meters_with_line_label(project_id)?
                        .into_iter()
                        .find(|(l, m)| *l == line && m.address == address)
                        .map(|(_, m)| m);
                    let Some(meter) = meter else {
                        cs.row_error(
                            row,
                            format!(
                                "{} „{line}“ / „{address}“",
                                tr("Zähler nicht gefunden", "meter not found")
                            ),
                        );
                        continue;
                    };
                    if parts(&wanted.key).is_none() {
                        cs.row_error(
                            row,
                            format!("{} „{}“", tr("Kennung ungültig", "invalid key"), wanted.key),
                        );
                        continue;
                    }
                    cs.add(meter_label(&meter), wanted.name.clone());
                    creates.push((meter, wanted));
                }
            }
        }
        cs.finish(apply, || {
            self.tx(|db| {
                for (id, d) in &updates {
                    db.dp_update(*id, d)?;
                }
                for (m, d) in &creates {
                    if let Some(id) = db.dp_create(m, &d.key, lang)? {
                        db.dp_update(id, d)?;
                    }
                }
                Ok(())
            })
        })
    }
}
