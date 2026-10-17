//! Zählerliste schnell füllen: Liste vom Kunden oder Installateur einlesen (Zähler anlegen und
//! benennen, Zuordnung über die ID auf dem Typenschild), Namen nach Muster erzeugen, Liste als
//! CSV ausgeben (zum Ausfüllen lassen und wieder Einlesen).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

#[derive(Debug, Default, Serialize)]
pub struct ListImport {
    /// neu anzulegen: ID, Adresse, Name, Ort
    pub added: Vec<Value>,
    /// geänderte Felder: ID, Feld, alt, neu
    pub changed: Vec<Value>,
    /// schon so in der Liste
    pub unchanged: usize,
    /// Hinweise, die den Import nicht verhindern
    pub warnings: Vec<String>,
    /// Zeilen, die nicht übernommen werden
    pub errors: Vec<String>,
    pub applied: bool,
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

fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

impl Db {
    /// Liste einlesen: zuerst Vorschau (`apply` = false), dann übernehmen. Zuordnung über die ID;
    /// leere Zellen ändern nichts.
    pub fn import_list(
        &mut self,
        line_id: i64,
        rows: &[ListRow],
        apply: bool,
    ) -> Result<ListImport> {
        let line = self.line(line_id)?;
        let meters = self.meters(line_id)?;
        let by_id: HashMap<String, &Meter> = meters
            .iter()
            .filter_map(|m| Some((meter_ident(m)?, m)))
            .collect();
        // Zähler auf anderen Linien des Projekts (Hinweis, falls die Liste dorthin gehört)
        let project = self.gateway(line.gateway_id)?.project_id;
        let mut elsewhere: HashMap<String, String> = HashMap::new();
        for p in self.tree()? {
            if p.project.id != project {
                continue;
            }
            for g in &p.gateways {
                for l in g.lines.iter().filter(|l| l.line.id != line_id) {
                    for m in self.meters(l.line.id)? {
                        if let Some(id) = meter_ident(&m) {
                            elsewhere.insert(id, format!("{} › {}", g.gateway.name, l.line.name));
                        }
                    }
                }
            }
        }
        // IDs aus dem letzten Scan dieser Linie (für „nicht auf dem Bus gefunden“)
        let scanned: Option<HashSet<String>> = self.last_scan(line_id)?.and_then(|s| {
            let hits = s["hits"].as_array()?;
            (!hits.is_empty()).then(|| {
                hits.iter()
                    .filter_map(|h| h["id"].as_str().map(String::from))
                    .collect()
            })
        });
        let mut res = ListImport::default();
        let mut seen = HashSet::new();
        let mut creates: Vec<Meter> = Vec::new();
        let mut updates: Vec<(i64, Meter)> = Vec::new();
        for (n, r) in rows.iter().enumerate() {
            let row = n + 1;
            if r.id.trim().is_empty() && r.name.trim().is_empty() && r.location.trim().is_empty() {
                continue;
            }
            let Some(id) = normalize_id(&r.id) else {
                res.errors.push(format!(
                    "{} {row}: {} „{}“",
                    tr("Zeile", "Row"),
                    tr("ID ungültig (bis 8 Ziffern)", "invalid ID (up to 8 digits)"),
                    r.id
                ));
                continue;
            };
            if !seen.insert(id.clone()) {
                res.errors.push(format!(
                    "{} {row}: ID {id} {}",
                    tr("Zeile", "Row"),
                    tr("kommt mehrfach vor", "appears more than once")
                ));
                continue;
            }
            let pa = match r.pa.trim() {
                "" => None,
                s => match s.parse::<u8>() {
                    Ok(p) if p <= 250 => Some(p),
                    _ => {
                        res.errors.push(format!(
                            "{} {row}: PA „{s}“ {}",
                            tr("Zeile", "Row"),
                            tr("ungültig (0..250)", "invalid (0..250)")
                        ));
                        continue;
                    }
                },
            };
            if let Some(l) = elsewhere.get(&id) {
                res.warnings.push(format!(
                    "ID {id}: {} {l}",
                    tr("ist schon auf der Linie", "is already on line")
                ));
            }
            if scanned.as_ref().is_some_and(|s| !s.contains(&id)) {
                res.warnings.push(format!(
                    "ID {id}: {}",
                    tr(
                        "im letzten Scan nicht gefunden",
                        "not found in the last scan"
                    )
                ));
            }
            match by_id.get(&id) {
                Some(m) => {
                    let mut new = (*m).clone();
                    let mut changed = false;
                    for (field, old, val) in [
                        ("name", &m.name, r.name.trim()),
                        ("location", &m.location, r.location.trim()),
                        ("note", &m.note, r.note.trim()),
                    ] {
                        if !val.is_empty() && old != val {
                            res.changed
                                .push(json!({ "id": id, "field": field, "old": old, "new": val }));
                            match field {
                                "name" => new.name = val.into(),
                                "location" => new.location = val.into(),
                                _ => new.note = val.into(),
                            }
                            changed = true;
                        }
                    }
                    if changed {
                        updates.push((m.id, new));
                    } else {
                        res.unchanged += 1;
                    }
                }
                None => {
                    // gemerkt wie die Linie: über SA (die ID genügt) oder, wenn angegeben, über PA
                    let address = match pa {
                        Some(p) if line.addressing == "primary" && p > 0 => p.to_string(),
                        _ => id.clone(),
                    };
                    if line.addressing == "primary" && address == id {
                        res.warnings.push(format!(
                            "ID {id}: {}",
                            tr(
                                "ohne PA, wird über die ID gemerkt",
                                "no PA, remembered by its ID"
                            )
                        ));
                    }
                    res.added.push(json!({ "id": id, "address": address, "name": r.name.trim(), "location": r.location.trim() }));
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
        if apply && res.errors.is_empty() {
            self.tx(|db| {
                for (id, m) in &updates {
                    db.update_meter(*id, m.clone())?;
                }
                for m in &creates {
                    db.add_meter(line_id, m.clone())?;
                }
                Ok(())
            })?;
            res.applied = true;
        }
        Ok(res)
    }

    /// Namen nach Muster setzen (alle oder nur die markierten / ohne Namen). Liefert je Zähler
    /// alt und neu; `apply` = false nur Vorschau.
    pub fn auto_names(
        &mut self,
        line_id: i64,
        pattern: &str,
        ids: Option<&[i64]>,
        only_empty: bool,
        apply: bool,
        lang: Lang,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let mut nr = 0;
        for m in self.meters(line_id)? {
            if ids.is_some_and(|ids| !ids.contains(&m.id)) || (only_empty && !m.name.is_empty()) {
                continue;
            }
            nr += 1;
            let name = render_name(pattern, &m, nr, lang);
            if name.is_empty() || name == m.name {
                continue;
            }
            out.push(json!({ "id": m.id, "ident": meter_ident(&m), "old": m.name, "new": name }));
            if apply {
                self.conn.execute(
                    "UPDATE meters SET name = ?2 WHERE id = ?1",
                    params![m.id, name],
                )?;
            }
        }
        Ok(out)
    }

    /// Zählerliste als CSV (Semikolon, Excel): zum Ausfüllen lassen und wieder Einlesen
    pub fn meters_csv(&self, line_id: i64, lang: Lang) -> Result<String> {
        let de = lang == Lang::De;
        let mut out = String::from(if de {
            "ID;PA;Hersteller;Medium;Name;Ort;Notiz;Sekundäradresse;Letzter Wert\r\n"
        } else {
            "ID;PA;Manufacturer;Medium;Name;Location;Note;Secondary address;Last value\r\n"
        });
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
            out.push_str(
                &row.iter()
                    .map(|x| csv_field(x))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
            out.push_str("\r\n");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Line;

    fn row(id: &str, name: &str, loc: &str) -> ListRow {
        ListRow {
            id: id.into(),
            name: name.into(),
            location: loc.into(),
            ..Default::default()
        }
    }

    #[test]
    fn ids_are_normalized() {
        assert_eq!(normalize_id("71000001").as_deref(), Some("71000001"));
        assert_eq!(normalize_id("KAM 1234").as_deref(), Some("00001234"));
        assert_eq!(normalize_id("123456789"), None);
        assert_eq!(normalize_id("abc"), None);
    }

    #[test]
    fn list_creates_and_renames_with_preview() {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap().id;
        db.add_meter(
            line,
            Meter {
                address: "71000001".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let rows = vec![
            row("71000001", "Whg 1 Wärme", "Haus A · Whg 1"),
            row("72000001", "Whg 1 Kaltwasser", "Haus A · Whg 1"),
            row("", "", ""),
        ];
        let p = db.import_list(line, &rows, false).unwrap();
        assert_eq!((p.added.len(), p.changed.len(), p.errors.len()), (1, 2, 0));
        assert!(!p.applied);
        assert_eq!(db.meters(line).unwrap().len(), 1, "Vorschau ändert nichts");
        let done = db.import_list(line, &rows, true).unwrap();
        assert!(done.applied);
        let ms = db.meters(line).unwrap();
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].name, "Whg 1 Wärme");
        assert_eq!(ms[1].address, "72000001");
        assert_eq!(ms[1].location, "Haus A · Whg 1");
        // nochmal: nichts mehr zu tun; leere Zellen überschreiben nichts
        let again = db
            .import_list(line, &[row("72000001", "", "")], false)
            .unwrap();
        assert_eq!(
            (again.added.len(), again.changed.len(), again.unchanged),
            (0, 0, 1)
        );
        // Fehler: nichts wird übernommen
        let bad = db
            .import_list(
                line,
                &[row("1234567890", "x", ""), row("73000001", "y", "")],
                true,
            )
            .unwrap();
        assert!(!bad.applied && bad.errors.len() == 1);
        assert_eq!(db.meters(line).unwrap().len(), 2);
    }

    #[test]
    fn primary_line_uses_pa_from_list() {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap();
        db.set_addressing(line.id, "primary").unwrap();
        let mut r = row("71000001", "A", "");
        r.pa = "5".into();
        db.import_list(line.id, &[r], true).unwrap();
        assert_eq!(db.meters(line.id).unwrap()[0].address, "5");
        let _ = Line::default();
    }

    #[test]
    fn names_from_pattern() {
        let m = Meter {
            location: "Whg 3".into(),
            medium: Some("medium_water".into()),
            address: "71000003".into(),
            ..Default::default()
        };
        assert_eq!(
            render_name("{ort} {medium}", &m, 1, Lang::De),
            "Whg 3 Wasser"
        );
        // leerer Ort: kein Trenner am Anfang
        let m2 = Meter {
            location: String::new(),
            ..m.clone()
        };
        assert_eq!(render_name("{ort} · {medium}", &m2, 1, Lang::De), "Wasser");
        assert_eq!(render_name("Z{nr} {id}", &m, 7, Lang::De), "Z7 71000003");
    }
}
