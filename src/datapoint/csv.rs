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
        let meters = self.meters_with_line_label(project_id)?;
        let mut cs = ChangeSet::default();
        let mut updates: Vec<(i64, Datapoint)> = Vec::new();
        let mut creates: Vec<(Meter, Datapoint)> = Vec::new();
        for (n, c) in rows.iter().enumerate() {
            let row = n + 1;
            let first = c.first().map(String::as_str).unwrap_or("");
            let no = parse_num(first).map(|x| x as i64);
            if n == 0 && no.is_none() && !first.is_empty() {
                continue; // Kopfzeile
            }
            let old = no.and_then(|no| existing.get(&no));
            let wanted = match parse_row(c, old) {
                Ok(d) => d,
                Err(e) => {
                    cs.row_error(row, e);
                    continue;
                }
            };
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
                    if compare(&mut cs, &format!("{no} · {}", old.name), old, &wanted) {
                        updates.push((old.id, wanted));
                    } else {
                        cs.unchanged += 1;
                    }
                }
                (None, _) => match meter_for_new(c, &meters, &wanted) {
                    Ok(meter) => {
                        cs.add(meter_label(meter), wanted.name.clone());
                        creates.push((meter.clone(), wanted));
                    }
                    Err(e) => cs.row_error(row, e),
                },
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

/// Eine Zeile prüfen und als gewünschten Datenpunkt liefern; Ausgänge ohne Spalten bleiben wie bisher
fn parse_row(c: &[String], old: Option<&Datapoint>) -> std::result::Result<Datapoint, String> {
    let col = |i: usize| c.get(i).cloned().unwrap_or_default();
    if c.len() < FIXED {
        return Err(tr(
            "zu wenige Spalten (10 erwartet)",
            "too few columns (10 expected)",
        ));
    }
    let factor = parse_num(&col(7))
        .filter(|f| f.is_finite() && *f != 0.0)
        .ok_or_else(|| format!("{} „{}“", tr("Faktor ungültig", "invalid factor"), col(7)))?;
    let send = col(8).to_lowercase();
    if !SEND_MODES.contains(&send.as_str()) {
        return Err(format!(
            "{} „{}“ ({})",
            tr("Weitergabe ungültig", "invalid send mode"),
            col(8),
            SEND_MODES.join("/")
        ));
    }
    let active = parse_bool(&col(9)).ok_or_else(|| {
        format!(
            "{} „{}“",
            tr("Aktiv ungültig (1/0)", "invalid active (1/0)"),
            col(9)
        )
    })?;
    // Ausgänge: je zwei Spalten
    let mut outputs = old.map(|d| d.outputs.clone()).unwrap_or_default();
    for (k, key) in OUTPUTS.iter().enumerate() {
        let i = FIXED + 2 * k;
        if c.len() <= i {
            continue;
        }
        let on =
            parse_bool(&col(i)).ok_or_else(|| format!("{} „{}“", output_label(key), col(i)))?;
        let address = Some(col(i + 1)).filter(|a| !a.is_empty());
        if let Some(a) = &address {
            outputs::check_address(key, a).map_err(|e| format!("{e:#}"))?;
        }
        let o = outputs.entry(key.to_string()).or_default();
        o.enabled = on;
        // leere Adresse in der Datei: vorhandene behalten
        if address.is_some() {
            o.address = address;
        }
    }
    let name = col(5);
    if name.is_empty() {
        return Err(tr("Name fehlt", "name missing"));
    }
    Ok(Datapoint {
        key: col(4),
        name,
        unit: col(6),
        factor,
        send,
        active,
        outputs,
        ..Default::default()
    })
}

/// Unterschiede eines vorhandenen Datenpunkts in die Vorschau; true, wenn sich etwas ändert
fn compare(cs: &mut ChangeSet, item: &str, old: &Datapoint, wanted: &Datapoint) -> bool {
    let flag = |b: bool| (b as u8).to_string();
    let mut changed = cs.change(item, tr("Name", "Name"), &old.name, &wanted.name);
    changed |= cs.change(item, tr("Einheit", "Unit"), &old.unit, &wanted.unit);
    changed |= cs.change(
        item,
        tr("Faktor", "Factor"),
        &old.factor.to_string(),
        &wanted.factor.to_string(),
    );
    changed |= cs.change(item, tr("Weitergabe", "Send"), &old.send, &wanted.send);
    changed |= cs.change(
        item,
        tr("Aktiv", "Active"),
        &flag(old.active),
        &flag(wanted.active),
    );
    for key in OUTPUTS {
        let (a, b) = (
            old.outputs.get(key).cloned().unwrap_or_default(),
            wanted.outputs.get(key).cloned().unwrap_or_default(),
        );
        changed |= cs.change(item, output_label(key), &flag(a.enabled), &flag(b.enabled));
        changed |= cs.change(
            item,
            address_label(key),
            a.address.as_deref().unwrap_or(""),
            b.address.as_deref().unwrap_or(""),
        );
    }
    changed
}

/// Neuer Datenpunkt: Zähler über Linie und Adresse finden, Kennung prüfen
fn meter_for_new<'a>(
    c: &[String],
    meters: &'a [(String, Meter)],
    wanted: &Datapoint,
) -> std::result::Result<&'a Meter, String> {
    let line = c.get(1).map(String::as_str).unwrap_or("");
    let address = c.get(3).map(|a| a.to_ascii_uppercase()).unwrap_or_default();
    let meter = meters
        .iter()
        .find(|(l, m)| l == line && m.address == address)
        .map(|(_, m)| m)
        .ok_or_else(|| {
            format!(
                "{} „{line}“ / „{address}“",
                tr("Zähler nicht gefunden", "meter not found")
            )
        })?;
    if parts(&wanted.key).is_none() {
        return Err(format!(
            "{} „{}“",
            tr("Kennung ungültig", "invalid key"),
            wanted.key
        ));
    }
    Ok(meter)
}
