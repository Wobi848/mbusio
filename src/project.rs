//! Zählerliste: gemerkte Zähler mit Name, Ort und Notiz, als JSON-Datei gespeichert.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::mbus::master::{mask_matches, MeterReading, Target};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meter {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub note: String,
    /// Wie beim Auslesen: Primäradresse, ID (8) oder Sekundäradresse (16)
    pub address: String,
    /// Eigene Baudrate, falls der Zähler nicht mit der Bus-Baudrate läuft
    #[serde(default)]
    pub baud: Option<u32>,
    // Aus Scan bzw. letzter Auslesung
    #[serde(default)]
    pub meter_id: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub medium: Option<String>,
    #[serde(default)]
    pub last_read: Option<String>,
    /// Hauptwert der letzten Auslesung, z.B. "123.456 m³"
    #[serde(default)]
    pub last_value: Option<String>,
    /// Schlüssel der Messgrösse zu `last_value`
    #[serde(default)]
    pub last_quantity: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct File {
    meters: Vec<Meter>,
}

/// `Store::default()` hält die Liste nur im Speicher (Tests)
#[derive(Default)]
pub struct Store {
    path: Option<PathBuf>,
    meters: Vec<Meter>,
}

/// Standardordner: %APPDATA%\mbusio, $XDG_DATA_HOME/mbusio oder ~/.local/share/mbusio
pub fn default_dir() -> PathBuf {
    let env = |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(p) = env("APPDATA") {
            return p.join("mbusio");
        }
    }
    if let Some(p) = env("XDG_DATA_HOME") {
        return p.join("mbusio");
    }
    if let Some(p) = env("HOME") {
        return p.join(".local/share/mbusio");
    }
    PathBuf::from(".")
}

fn normalize(address: &str) -> Result<Target> {
    Target::parse(address)
}

/// Gleicher Zähler? Eine ID (Rest Platzhalter) passt auch auf die volle Sekundäradresse.
fn same_meter(a: &Target, b: &Target) -> bool {
    match (a, b) {
        (Target::Primary(x), Target::Primary(y)) => x == y,
        (Target::Secondary(x), Target::Secondary(y)) => mask_matches(x, y) || mask_matches(y, x),
        _ => false,
    }
}

fn listed(m: &Meter, target: &Target) -> bool {
    normalize(&m.address).is_ok_and(|t| same_meter(&t, target))
}

impl Store {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join("meters.json");
        let meters = match std::fs::read_to_string(&path) {
            Ok(text) => {
                serde_json::from_str::<File>(&text)
                    .with_context(|| format!("{}", path.display()))?
                    .meters
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("{}", path.display())),
        };
        Ok(Store {
            path: Some(path),
            meters,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&File {
            meters: self.meters.clone(),
        })?;
        // erst in Hilfsdatei schreiben, damit ein Absturz die Liste nicht zerstört
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<Meter> {
        self.meters.clone()
    }

    fn check(&self, m: &Meter, skip_id: u64) -> Result<()> {
        let target = normalize(&m.address)?;
        if self
            .meters
            .iter()
            .any(|x| x.id != skip_id && listed(x, &target))
        {
            bail!(
                "{}: {}",
                tr(
                    "Zähler mit dieser Adresse ist schon in der Liste",
                    "A meter with this address is already in the list"
                ),
                m.address
            );
        }
        if let Some(b) = m.baud {
            if !crate::mbus::master::BAUD_RATES.contains(&b) {
                bail!(
                    "{}: {b}",
                    tr("Keine Standard-Baudrate", "Not a standard baud rate")
                );
            }
        }
        Ok(())
    }

    pub fn add(&mut self, mut m: Meter) -> Result<Meter> {
        m.address = m.address.trim().to_ascii_uppercase();
        self.check(&m, 0)?;
        m.id = self.meters.iter().map(|x| x.id).max().unwrap_or(0) + 1;
        self.meters.push(m.clone());
        self.save()?;
        Ok(m)
    }

    /// Ändert die vom Benutzer gepflegten Felder, Messinfos bleiben.
    pub fn update(&mut self, id: u64, new: Meter) -> Result<Meter> {
        let mut new = new;
        new.address = new.address.trim().to_ascii_uppercase();
        self.check(&new, id)?;
        let m = self.get_mut(id)?;
        m.name = new.name;
        m.location = new.location;
        m.note = new.note;
        if m.address != new.address {
            m.address = new.address;
            m.last_error = None;
        }
        m.baud = new.baud;
        let out = m.clone();
        self.save()?;
        Ok(out)
    }

    pub fn remove(&mut self, id: u64) -> Result<()> {
        self.get_mut(id)?;
        self.meters.retain(|m| m.id != id);
        self.save()
    }

    fn get_mut(&mut self, id: u64) -> Result<&mut Meter> {
        self.meters.iter_mut().find(|m| m.id == id).ok_or_else(|| {
            anyhow::anyhow!(
                "{} {id}",
                tr("Zähler nicht in der Liste:", "Meter not in list:")
            )
        })
    }

    /// Ergebnis einer Auslesung beim passenden Zähler eintragen (falls vorhanden).
    pub fn record(&mut self, target: &Target, result: Result<&MeterReading, String>) {
        let Some(m) = self
            .meters
            .iter_mut()
            .find(|m| listed(m, target))
        else {
            return;
        };
        match result {
            Ok(r) => {
                m.last_read = Some(r.timestamp.clone());
                m.last_error = None;
                if let Some(h) = &r.header {
                    m.meter_id = Some(h.id.clone());
                    m.manufacturer = Some(h.manufacturer.clone()).filter(|s| !s.is_empty());
                    m.medium = Some(h.medium.clone());
                }
                if let Some(x) = main_value(r) {
                    m.last_value = Some(format!("{} {}", x.value_text, x.unit).trim().into());
                    m.last_quantity = Some(x.quantity.clone());
                }
            }
            Err(e) => m.last_error = Some(e),
        }
        // Speicherfehler hier nicht fatal, die Auslesung selbst hat geklappt
        if let Err(e) = self.save() {
            eprintln!("meters.json: {e:#}");
        }
    }

    /// Fügt Zähler hinzu, deren Adresse noch nicht in der Liste ist. Liefert die Anzahl.
    pub fn import(&mut self, meters: Vec<Meter>) -> Result<usize> {
        let mut n = 0;
        for mut m in meters {
            m.address = m.address.trim().to_ascii_uppercase();
            if self.check(&m, 0).is_err() {
                continue;
            }
            m.id = self.meters.iter().map(|x| x.id).max().unwrap_or(0) + 1;
            self.meters.push(m);
            n += 1;
        }
        self.save()?;
        Ok(n)
    }
}

/// Wichtigster Wert für die Übersicht: aktueller Zählerstand (Energie, Volumen, HKV) vor allem anderen.
fn main_value(r: &MeterReading) -> Option<&crate::mbus::decode::Record> {
    let current = || {
        r.records
            .iter()
            .filter(|x| x.storage == 0 && x.function == "instantaneous" && x.value.is_number())
    };
    ["energy", "volume", "hca_units", "mass"]
        .iter()
        .find_map(|q| current().find(|x| x.quantity == *q))
        .or_else(|| current().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter(address: &str) -> Meter {
        Meter {
            address: address.into(),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_duplicates_in_any_notation() {
        let mut s = Store::default();
        s.add(meter("12345678")).unwrap();
        // ID, Maske mit Platzhaltern und volle Sekundäradresse sind derselbe Zähler
        assert!(s.add(meter("12345678ffffffff")).is_err());
        assert!(s.add(meter("123456782C2D1B04")).is_err());
        assert!(s.add(meter("x")).is_err());
        let b = s.add(meter("5")).unwrap();
        assert_eq!(b.id, 2);
        assert!(s.update(b.id, meter("12345678")).is_err());
        assert_eq!(s.import(vec![meter("5"), meter("6")]).unwrap(), 1);
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("mbusio-test-{}", std::process::id()));
        let mut s = Store::load(&dir).unwrap();
        s.add(Meter {
            name: "Whg 3".into(),
            baud: Some(9600),
            ..meter("3")
        })
        .unwrap();
        let s = Store::load(&dir).unwrap();
        assert_eq!(s.list()[0].name, "Whg 3");
        assert_eq!(s.list()[0].baud, Some(9600));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
