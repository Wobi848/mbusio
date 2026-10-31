//! Zählerliste schnell füllen: Liste vom Kunden oder Installateur einlesen (Zähler anlegen und
//! benennen, Zuordnung über die ID auf dem Typenschild), Namen nach Muster erzeugen, Liste als
//! CSV ausgeben (zum Ausfüllen lassen und wieder Einlesen).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::params;
use serde::Deserialize;

use crate::changeset::ChangeSet;
use crate::db::{Db, Meter};
use crate::i18n::{self, tr, Lang};

/// Eine Zeile der eingelesenen Liste (Spalten hat der Benutzer zugeordnet)
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub pa: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub note: String,
}

/// ID vom Typenschild: nur Ziffern, auf 8 Stellen aufgefüllt
pub fn normalize_id(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 8 {
        return None;
    }
    Some(format!("{digits:0>8}"))
}

/// ID eines Zählers der Liste: aus der Auslesung, sonst aus der Adresse (ID oder Sekundäradresse)
pub fn meter_ident(m: &Meter) -> Option<String> {
    if let Some(id) = &m.meter_id {
        return Some(id.clone());
    }
    let a = &m.address;
    if a.len() == 8 && a.chars().all(|c| c.is_ascii_digit()) {
        return Some(a.clone());
    }
    if a.len() == 16 && a[..8].chars().all(|c| c.is_ascii_digit()) {
        return Some(a[..8].to_string());
    }
    m.secondary_address
        .as_ref()
        .map(|s| s[..8.min(s.len())].to_string())
}

/// Name nach Muster: {ort} {medium} {id} {pa} {name} {nr}
pub fn render_name(pattern: &str, m: &Meter, nr: usize, lang: Lang) -> String {
    let medium = m
        .medium
        .as_deref()
        .map(|k| i18n::label(k, lang))
        .unwrap_or_default();
    let pa = m.primary_address.map(|p| p.to_string()).unwrap_or_default();
    let out = pattern
        .replace("{ort}", &m.location)
        .replace("{location}", &m.location)
        .replace("{medium}", &medium)
        .replace("{id}", &meter_ident(m).unwrap_or_default())
        .replace("{pa}", &pa)
        .replace("{name}", &m.name)
        .replace("{nr}", &nr.to_string());
    // leere Platzhalter hinterlassen keine doppelten Abstände oder Trenner am Rand
    let mut s = out.split_whitespace().collect::<Vec<_>>().join(" ");
    for sep in ["·", "-", "/", ","] {
        s = s
            .trim_matches(|c: char| c.is_whitespace() || sep.contains(c))
            .to_string();
    }
    s
}

impl Db {
    /// Liste einlesen: zuerst Vorschau (`apply` = false), dann übernehmen. Zuordnung über die ID;
    /// leere Zellen ändern nichts.
    pub fn import_list(
        &mut self,
        line_id: i64,
        rows: &[ListRow],
        apply: bool,
    ) -> Result<ChangeSet> {
        let line = self.line(line_id)?;
        let meters = self.meters(line_id)?;
        let by_id: HashMap<String, &Meter> = meters
            .iter()
            .filter_map(|m| Some((meter_ident(m)?, m)))
            .collect();
        // Zähler auf anderen Linien des Projekts (Hinweis, falls die Liste dorthin gehört)
        let project = self.gateway(line.gateway_id)?.project_id;
        let elsewhere: HashMap<String, String> = self
            .meters_with_line_label(project)?
            .into_iter()
            .filter(|(_, m)| m.line_id != line_id)
            .filter_map(|(label, m)| Some((meter_ident(&m)?, label)))
            .collect();
        // IDs aus dem letzten Scan dieser Linie (für „nicht auf dem Bus gefunden“)
        let scanned: Option<HashSet<String>> = self.last_scan(line_id)?.and_then(|s| {
            let hits = s["hits"].as_array()?;
            (!hits.is_empty()).then(|| {
                hits.iter()
                    .filter_map(|h| h["id"].as_str().map(String::from))
                    .collect()
            })
        });
        let mut cs = ChangeSet::default();
        let mut seen = HashSet::new();
        let mut creates: Vec<Meter> = Vec::new();
        let mut updates: Vec<(i64, Meter)> = Vec::new();
        for (n, r) in rows.iter().enumerate() {
            let row = n + 1;
            if r.id.trim().is_empty() && r.name.trim().is_empty() && r.location.trim().is_empty() {
                continue;
            }
            let Some(id) = normalize_id(&r.id) else {
                cs.row_error(
                    row,
                    format!(
                        "{} „{}“",
                        tr("ID ungültig (bis 8 Ziffern)", "invalid ID (up to 8 digits)"),
                        r.id
                    ),
                );
                continue;
            };
            if !seen.insert(id.clone()) {
                cs.row_error(
                    row,
                    format!(
                        "ID {id} {}",
                        tr("kommt mehrfach vor", "appears more than once")
                    ),
                );
                continue;
            }
            let pa = match r.pa.trim() {
                "" => None,
                s => match s.parse::<u8>() {
                    Ok(p) if p <= 250 => Some(p),
                    _ => {
                        cs.row_error(
                            row,
                            format!("PA „{s}“ {}", tr("ungültig (0..250)", "invalid (0..250)")),
                        );
                        continue;
                    }
                },
            };
            if let Some(l) = elsewhere.get(&id) {
                cs.warn(format!(
                    "ID {id}: {} {l}",
                    tr("ist schon auf der Linie", "is already on line")
                ));
            }
            if scanned.as_ref().is_some_and(|s| !s.contains(&id)) {
                cs.warn(format!(
                    "ID {id}: {}",
                    tr(
                        "im letzten Scan nicht gefunden",
                        "not found in the last scan"
                    )
                ));
            }
            match by_id.get(&id) {
                Some(m) => {
                    // leere Zellen ändern nichts
                    let pick = |cell: &str, old: &str| {
                        let v = cell.trim();
                        if v.is_empty() {
                            old.to_string()
                        } else {
                            v.to_string()
                        }
                    };
                    let new = Meter {
                        name: pick(&r.name, &m.name),
                        location: pick(&r.location, &m.location),
                        note: pick(&r.note, &m.note),
                        ..(*m).clone()
                    };
                    let mut changed = cs.change(&id, tr("Name", "Name"), &m.name, &new.name);
                    changed |= cs.change(&id, tr("Ort", "Location"), &m.location, &new.location);
                    changed |= cs.change(&id, tr("Notiz", "Note"), &m.note, &new.note);
                    if changed {
                        updates.push((m.id, new));
                    } else {
                        cs.unchanged += 1;
                    }
                }
                None => {
                    // gemerkt wie die Linie: über SA (die ID genügt) oder, wenn angegeben, über PA
                    let address = match pa {
                        Some(p) if line.addressing == "primary" && p > 0 => p.to_string(),
                        _ => id.clone(),
                    };
                    if line.addressing == "primary" && address == id {
                        cs.warn(format!(
                            "ID {id}: {}",
                            tr(
                                "ohne PA, wird über die ID gemerkt",
                                "no PA, remembered by its ID"
                            )
                        ));
                    }
                    let detail = [r.name.trim(), r.location.trim()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    cs.add(id.clone(), detail);
                    creates.push(Meter {
                        address,
                        name: r.name.trim().into(),
                        location: r.location.trim().into(),
                        note: r.note.trim().into(),
                        meter_id: Some(id),
                        primary_address: pa,
                        ..Default::default()
                    });
                }
            }
        }
        cs.finish(apply, || {
            self.tx(|db| {
                for (id, m) in &updates {
                    db.update_meter(*id, m.clone())?;
                }
                for m in &creates {
                    db.add_meter(line_id, m.clone())?;
                }
                Ok(())
            })
        })
    }

    /// Namen nach Muster setzen (alle oder nur die markierten / ohne Namen); zuerst Vorschau
    pub fn auto_names(
        &mut self,
        line_id: i64,
        pattern: &str,
        ids: Option<&[i64]>,
        only_empty: bool,
        apply: bool,
        lang: Lang,
    ) -> Result<ChangeSet> {
        let mut cs = ChangeSet::default();
        let mut names = Vec::new();
        let mut nr = 0;
        for m in self.meters(line_id)? {
            if ids.is_some_and(|ids| !ids.contains(&m.id)) || (only_empty && !m.name.is_empty()) {
                continue;
            }
            nr += 1;
            let name = render_name(pattern, &m, nr, lang);
            let item = meter_ident(&m).unwrap_or_else(|| m.address.clone());
            if !name.is_empty() && cs.change(&item, tr("Name", "Name"), &m.name, &name) {
                names.push((m.id, name));
            } else {
                cs.unchanged += 1;
            }
        }
        cs.finish(apply, || {
            for (id, name) in &names {
                self.conn.execute(
                    "UPDATE meters SET name = ?2 WHERE id = ?1",
                    params![id, name],
                )?;
            }
            Ok(())
        })
    }

    /// Zählerliste als CSV (Semikolon, Excel): zum Ausfüllen lassen und wieder Einlesen
    pub fn meters_csv(&self, line_id: i64, lang: Lang) -> Result<String> {
        let mut out = i18n::tr_in(
            lang,
            "ID;PA;Hersteller;Medium;Name;Ort;Notiz;Sekundäradresse;Letzter Wert",
            "ID;PA;Manufacturer;Medium;Name;Location;Note;Secondary address;Last value",
        );
        out.push_str("\r\n");
        for m in self.meters(line_id)? {
            let row = [
                meter_ident(&m).unwrap_or_default(),
                m.primary_address.map(|p| p.to_string()).unwrap_or_default(),
                m.manufacturer.clone().unwrap_or_default(),
                m.medium
                    .as_deref()
                    .map(|k| i18n::label(k, lang))
                    .unwrap_or_default(),
                m.name.clone(),
                m.location.clone(),
                m.note.clone(),
                m.secondary_address.clone().unwrap_or_default(),
                m.last_value.clone().unwrap_or_default(),
            ];
            out.push_str(&crate::export::row(&row));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
