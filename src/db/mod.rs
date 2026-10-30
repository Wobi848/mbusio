//! Datenbank (SQLite): Projekte → Gateways → Linien → Zähler, dazu Auslesungen (mit Verlauf),
//! der letzte Scan pro Linie und das Änderungsprotokoll.
//!
//! Alles, was mit dem Bus zu tun hat, gehört zu einer Linie. Eine Linie ist immer aktiv;
//! Zählerliste, Scan und Auslesungen der Weboberfläche beziehen sich auf sie.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::i18n::tr;
use crate::mbus::master::{check_baud, mask_matches, MeterReading, Target};
use crate::plan::{self, Plan};

/// Name des Ordners neben dem Programm für den portablen Betrieb
pub const PORTABLE_DIR: &str = "mbusio-data";

/// Portabel: Ordner `mbusio-data` neben der .exe, falls vorhanden (USB-Stick, ohne Installation).
/// Sonst %APPDATA%\mbusio, $XDG_DATA_HOME/mbusio oder ~/.local/share/mbusio.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(PORTABLE_DIR)))
        .filter(|d| d.is_dir())
    {
        return dir;
    }
    let env = |k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
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

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ---------------------------------------------------------------- Adressen vergleichen

/// Gleicher Zähler? Eine ID (Rest Platzhalter) passt auch auf die volle Sekundäradresse.
fn same_meter(a: &Target, b: &Target) -> bool {
    match (a, b) {
        (Target::Primary(x), Target::Primary(y)) => x == y,
        (Target::Secondary(x), Target::Secondary(y)) => mask_matches(x, y) || mask_matches(y, x),
        _ => false,
    }
}

pub(crate) fn listed(m: &Meter, target: &Target) -> bool {
    Target::parse(&m.address).is_ok_and(|t| same_meter(&t, target))
}

/// Wichtigster Wert für die Übersicht: aktueller Zählerstand (Energie, Volumen, HKV) vor allem anderen.
pub(crate) fn main_value(r: &MeterReading) -> Option<&crate::mbus::decode::Record> {
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

mod lines;
mod meters;
mod projects;
mod readings;
mod report;
mod schedule;
mod schema;
mod thinning;
mod types;

pub use thinning::StorageSettings;

pub use self::types::*;
use self::types::{default_plan, gateway_of, line_of, meter_of, project_of, valid_plan};

// ---------------------------------------------------------------- Datenbank

pub struct Db {
    pub(crate) conn: Connection,
    path: Option<PathBuf>,
    /// Speicher fast voll: keine neuen Auslesungen mehr speichern (Zählerliste wird weiter
    /// aktualisiert), damit die Datenbank intakt und das Gerät bedienbar bleibt
    pub readings_paused: bool,
    /// Meldungen beim Speichern (z.B. Zählerstand gesunken); der Aufrufer schreibt sie ins Protokoll
    pub notes: Vec<crate::i18n::Text>,
    /// Zählt jede Änderung an Werten und Datenpunkten; wer mitläuft (Node-RED), gleicht dann ab
    pub changes: tokio::sync::watch::Sender<u64>,
}

/// Etwas mit dieser ID gibt es nicht (Projekt, Linie, Zähler …); die API antwortet dann mit 404
#[derive(Debug)]
pub struct NotFound(String);

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotFound {}

fn not_found(what: &str, id: i64) -> anyhow::Error {
    NotFound(format!(
        "{what} {id}: {}",
        tr("nicht gefunden", "not found")
    ))
    .into()
}

impl Db {
    /// Öffnet `mbusio.db` im Ordner und übernimmt beim ersten Start eine alte `meters.json`.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
        let path = dir.join("mbusio.db");
        let conn = Connection::open(&path).with_context(|| path.display().to_string())?;
        let mut db = Db {
            conn,
            path: Some(path),
            readings_paused: false,
            notes: Vec::new(),
            changes: tokio::sync::watch::channel(0).0,
        };
        db.init()?;
        db.import_legacy(dir)?;
        // für Sicherungen: eine ältere Version spielt keine Sicherung einer neueren ein
        db.set_meta("app_version", env!("CARGO_PKG_VERSION"))?;
        Ok(db)
    }

    /// Nur im Speicher, für Tests
    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        let mut db = Db {
            conn: Connection::open_in_memory()?,
            path: None,
            readings_paused: false,
            notes: Vec::new(),
            changes: tokio::sync::watch::channel(0).0,
        };
        db.init()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn conn_for_tests(&self) -> &Connection {
        &self.conn
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Zählerliste aus Version 0.2/0.3 (meters.json) in die aktive Linie übernehmen
    fn import_legacy(&mut self, dir: &Path) -> Result<()> {
        let file = dir.join("meters.json");
        let Ok(text) = std::fs::read_to_string(&file) else {
            return Ok(());
        };
        #[derive(Deserialize)]
        struct Old {
            meters: Vec<Meter>,
        }
        let old: Old = serde_json::from_str(&text).with_context(|| file.display().to_string())?;
        let line = self.active_line()?.id;
        self.import_meters(line, old.meters)?;
        std::fs::rename(&file, dir.join("meters.json.imported"))?;
        Ok(())
    }

    /// Werte oder Datenpunkte haben sich (vielleicht) geändert: Empfänger gleichen ab
    pub fn notify(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    /// Einstellung als JSON lesen (z.B. "mqtt"); fehlt sie oder ist unlesbar: Vorgabe
    pub(crate) fn meta_json<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> T {
        self.meta(key)
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    /// Einstellung lesen (z.B. "mqtt"); Fehler gelten als nicht gesetzt
    pub(crate) fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub(crate) fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Es gibt immer eine aktive Linie; notfalls wird ein Projekt "Standard" angelegt.
    fn ensure_active(&mut self) -> Result<i64> {
        if let Some(id) = self
            .meta("active_line")?
            .and_then(|v| v.parse::<i64>().ok())
        {
            if self.line(id).is_ok() {
                return Ok(id);
            }
        }
        let first: Option<i64> = self
            .conn
            .query_row("SELECT id FROM lines ORDER BY id LIMIT 1", [], |r| r.get(0))
            .optional()?;
        let id = match first {
            Some(id) => id,
            None => {
                let p = self.create_project(Project {
                    name: tr("Schnellscan", "Quick scan"),
                    ..Default::default()
                })?;
                self.lines_of_project(p.id)?[0].id
            }
        };
        self.set_meta("active_line", &id.to_string())?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
