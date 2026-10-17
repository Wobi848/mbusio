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
use crate::mbus::master::{mask_matches, MeterReading, Target, BAUD_RATES};
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

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ---------------------------------------------------------------- Datentypen

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub site: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub order_no: String,
    #[serde(default)]
    pub remarks: String,
    #[serde(default)]
    pub created: String,
    /// Ausleseplan als Vorgabe für alle Linien (siehe `plan::Plan`), Standard "off"
    #[serde(default = "default_plan")]
    pub plan: String,
}

fn default_plan() -> String {
    "off".into()
}

/// Gespeicherter Plan in der Normalform; Unbekanntes (z.B. aus einer Datei) fällt weg
fn valid_plan(s: Option<&str>) -> Option<String> {
    Some(Plan::parse(s?).ok()?.key())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Gateway {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub project_id: i64,
    pub name: String,
    /// z.B. IP-Adresse; dient als Vorgabe für neue Linien
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Line {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub gateway_id: i64,
    pub name: String,
    /// wie beim Verbinden: COM3, /dev/ttyUSB0, tcp://192.168.1.50:10001 oder SIMULATOR; leer = noch nicht festgelegt
    #[serde(default)]
    pub connection: String,
    #[serde(default = "default_baud")]
    pub baud: u32,
    #[serde(default)]
    pub timeout_ms: u64,
    /// Wie Zähler dieser Linie gemerkt werden: "secondary" (SA) oder "primary" (PA)
    #[serde(default = "default_addressing")]
    pub addressing: String,
    /// Eigener Ausleseplan; None = wie Projekt
    #[serde(default)]
    pub plan: Option<String>,
}

fn default_addressing() -> String {
    "secondary".into()
}

fn default_baud() -> u32 {
    2400
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meter {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub line_id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub note: String,
    /// Wie beim Auslesen: Primäradresse, ID (8) oder Sekundäradresse (16)
    pub address: String,
    /// Eigene Baudrate, falls der Zähler nicht mit der Baudrate der Linie läuft
    #[serde(default)]
    pub baud: Option<u32>,
    #[serde(default)]
    pub meter_id: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub medium: Option<String>,
    /// Primäradresse bei der letzten Auslesung
    #[serde(default)]
    pub primary_address: Option<u8>,
    /// Volle Sekundäradresse (16 Hex), sobald bekannt; erlaubt das Umstellen PA ↔ SA
    #[serde(default)]
    pub secondary_address: Option<String>,
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
    /// Eigener Ausleseplan; None = wie Linie
    #[serde(default)]
    pub plan: Option<String>,
    /// Zustand im Plan (nur lesen): Plan und zuletzt erledigter Termin dazu
    #[serde(default, skip_deserializing)]
    pub plan_key: Option<String>,
    #[serde(default, skip_deserializing)]
    pub plan_slot: Option<String>,
    /// ausgesetzt bis, weil der Zähler abgelehnt hat (beschäftigt, zu viele Auslesungen)
    #[serde(default, skip_deserializing)]
    pub plan_pause: Option<String>,
    /// Termine nacheinander nicht gelesen; ab 3 gilt der Zähler als gestört
    #[serde(default, skip_deserializing)]
    pub plan_fails: i64,
    /// gemessene Dauer einer Auslesung (gleitender Mittelwert), für die Buslast
    #[serde(default, skip_deserializing)]
    pub plan_ms: Option<f64>,
}

/// Ab so vielen verpassten Terminen nacheinander gilt ein Zähler als gestört
pub const DISTURBED_AFTER: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub time: String,
    #[serde(default)]
    pub line: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct LineInfo {
    #[serde(flatten)]
    pub line: Line,
    pub meters: i64,
    pub last_read: Option<String>,
    pub scanned: Option<String>,
    /// Auffälligkeiten des letzten Scans: {"count": n, "items": [...]}
    pub issues: Option<Value>,
    /// Ausleseplan der Linie mit nächstem Termin und letztem Ergebnis
    pub plan_info: Value,
}

#[derive(Debug, Serialize)]
pub struct GatewayNode {
    #[serde(flatten)]
    pub gateway: Gateway,
    pub lines: Vec<LineInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProjectNode {
    #[serde(flatten)]
    pub project: Project,
    pub gateways: Vec<GatewayNode>,
}

/// Eine Linie mit dem gültigen Plan je Zähler
pub struct PlannedLine {
    pub line: Line,
    /// "Gateway › Linie" fürs Protokoll
    pub label: String,
    pub meters: Vec<(Meter, Plan)>,
}

/// Ausgang eines Termins für einen Zähler
pub enum PlanOutcome {
    /// gelesen, so lange hat es gedauert
    Read { ms: f64 },
    /// nicht gelesen (auch nach Wiederholung)
    Failed,
    /// Zähler hat abgelehnt (beschäftigt, zu viele Auslesungen): ausgesetzt bis
    Refused { until: String },
}

/// Wo die aktive Linie hängt, für Kopfzeile und Bericht
#[derive(Debug, Clone, Serialize)]
pub struct Active {
    pub project: Project,
    pub gateway: Gateway,
    pub line: Line,
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

fn listed(m: &Meter, target: &Target) -> bool {
    Target::parse(&m.address).is_ok_and(|t| same_meter(&t, target))
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

// ---------------------------------------------------------------- Datenbank

pub struct Db {
    conn: Connection,
    path: Option<PathBuf>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    customer TEXT NOT NULL DEFAULT '',
    site TEXT NOT NULL DEFAULT '',
    technician TEXT NOT NULL DEFAULT '',
    order_no TEXT NOT NULL DEFAULT '',
    remarks TEXT NOT NULL DEFAULT '',
    created TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS gateways (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    address TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS lines (
    id INTEGER PRIMARY KEY,
    gateway_id INTEGER NOT NULL REFERENCES gateways(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    connection TEXT NOT NULL DEFAULT '',
    baud INTEGER NOT NULL DEFAULT 2400,
    timeout_ms INTEGER NOT NULL DEFAULT 0,
    last_scan TEXT,
    scanned TEXT
);
CREATE TABLE IF NOT EXISTS meters (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL,
    baud INTEGER,
    meter_id TEXT,
    manufacturer TEXT,
    medium TEXT,
    primary_address INTEGER,
    last_read TEXT,
    last_value TEXT,
    last_quantity TEXT,
    last_error TEXT
);
CREATE TABLE IF NOT EXISTS readings (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    meter INTEGER REFERENCES meters(id) ON DELETE SET NULL,
    timestamp TEXT NOT NULL,
    data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS readings_line ON readings(line_id, id);
CREATE TABLE IF NOT EXISTS changes (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    time TEXT NOT NULL,
    text TEXT NOT NULL
);
";

fn project_of(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        customer: r.get("customer")?,
        site: r.get("site")?,
        technician: r.get("technician")?,
        order_no: r.get("order_no")?,
        remarks: r.get("remarks")?,
        created: r.get("created")?,
        plan: r.get("plan")?,
    })
}

fn gateway_of(r: &Row) -> rusqlite::Result<Gateway> {
    Ok(Gateway {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        name: r.get("name")?,
        address: r.get("address")?,
        note: r.get("note")?,
    })
}

fn line_of(r: &Row) -> rusqlite::Result<Line> {
    Ok(Line {
        id: r.get("id")?,
        gateway_id: r.get("gateway_id")?,
        name: r.get("name")?,
        connection: r.get("connection")?,
        baud: r.get("baud")?,
        timeout_ms: r.get::<_, i64>("timeout_ms")? as u64,
        addressing: r.get("addressing")?,
        plan: r.get("plan")?,
    })
}

fn meter_of(r: &Row) -> rusqlite::Result<Meter> {
    Ok(Meter {
        id: r.get("id")?,
        line_id: r.get("line_id")?,
        name: r.get("name")?,
        location: r.get("location")?,
        note: r.get("note")?,
        address: r.get("address")?,
        baud: r.get("baud")?,
        meter_id: r.get("meter_id")?,
        manufacturer: r.get("manufacturer")?,
        medium: r.get("medium")?,
        primary_address: r.get("primary_address")?,
        secondary_address: r.get("secondary_address")?,
        last_read: r.get("last_read")?,
        last_value: r.get("last_value")?,
        last_quantity: r.get("last_quantity")?,
        last_error: r.get("last_error")?,
        plan: r.get("plan")?,
        plan_key: r.get("plan_key")?,
        plan_slot: r.get("plan_slot")?,
        plan_pause: r.get("plan_pause")?,
        plan_fails: r.get("plan_fails")?,
        plan_ms: r.get("plan_ms")?,
    })
}

fn not_found(what: &str, id: i64) -> anyhow::Error {
    anyhow!("{what} {id}: {}", tr("nicht gefunden", "not found"))
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
        };
        db.init()?;
        db.import_legacy(dir)?;
        Ok(db)
    }

    /// Nur im Speicher, für Tests
    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        let mut db = Db {
            conn: Connection::open_in_memory()?,
            path: None,
        };
        db.init()?;
        Ok(db)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn init(&mut self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        self.conn.execute_batch(SCHEMA)?;
        self.migrate()?;
        self.ensure_active()?;
        Ok(())
    }

    /// Spalten, die nach der ersten Version dazugekommen sind
    fn migrate(&mut self) -> Result<()> {
        let has = |conn: &Connection, table: &str, col: &str| -> Result<bool> {
            let mut st = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let names = st.query_map([], |r| r.get::<_, String>(1))?;
            for n in names {
                if n? == col {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        if !has(&self.conn, "lines", "issues")? {
            // Auffälligkeiten des letzten Scans: {"count": n, "items": [...]}
            self.conn.execute_batch("ALTER TABLE lines ADD COLUMN issues TEXT")?;
        }
        if !has(&self.conn, "lines", "addressing")? {
            self.conn.execute_batch(
                "ALTER TABLE lines ADD COLUMN addressing TEXT NOT NULL DEFAULT 'secondary'",
            )?;
        }
        for table in ["gateways", "lines"] {
            if !has(&self.conn, table, "position")? {
                // Reihenfolge zum Sortieren; bisher galt die Reihenfolge des Anlegens
                self.conn.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
                     UPDATE {table} SET position = id;"
                ))?;
            }
        }
        if !has(&self.conn, "meters", "secondary_address")? {
            // aus der Adresse selbst oder der letzten Auslesung nachtragen
            self.conn.execute_batch(
                "ALTER TABLE meters ADD COLUMN secondary_address TEXT;
                 UPDATE meters SET secondary_address = address
                   WHERE length(address) = 16 AND address NOT LIKE '%F%';
                 UPDATE meters SET secondary_address = (
                     SELECT json_extract(data, '$.header.secondary_address') FROM readings
                     WHERE readings.meter = meters.id ORDER BY id DESC LIMIT 1)
                   WHERE secondary_address IS NULL;",
            )?;
        }
        if !has(&self.conn, "projects", "plan")? {
            // Ausleseplan (Dauerbetrieb); Standard überall aus
            self.conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN plan TEXT NOT NULL DEFAULT 'off';
                 ALTER TABLE lines ADD COLUMN plan TEXT;
                 ALTER TABLE lines ADD COLUMN plan_result TEXT;
                 ALTER TABLE meters ADD COLUMN plan TEXT;
                 ALTER TABLE meters ADD COLUMN plan_key TEXT;
                 ALTER TABLE meters ADD COLUMN plan_slot TEXT;
                 ALTER TABLE meters ADD COLUMN plan_pause TEXT;
                 ALTER TABLE meters ADD COLUMN plan_fails INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE meters ADD COLUMN plan_ms REAL;",
            )?;
        }
        Ok(())
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

    fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Es gibt immer eine aktive Linie; notfalls wird ein Projekt "Standard" angelegt.
    fn ensure_active(&mut self) -> Result<i64> {
        if let Some(id) = self.meta("active_line")?.and_then(|v| v.parse::<i64>().ok()) {
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
                    name: tr("Standard", "Default"),
                    ..Default::default()
                })?;
                self.lines_of_project(p.id)?[0].id
            }
        };
        self.set_meta("active_line", &id.to_string())?;
        Ok(id)
    }

    // ------------------------------------------------------------ Projekte, Gateways, Linien

    /// Neues Projekt, gleich mit einem Gateway und einer Linie, damit man sofort scannen kann
    pub fn create_project(&mut self, p: Project) -> Result<Project> {
        let name = p.name.trim();
        if name.is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        self.conn.execute(
            "INSERT INTO projects (name, customer, site, technician, order_no, remarks, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, p.customer, p.site, p.technician, p.order_no, p.remarks, now()],
        )?;
        let id = self.conn.last_insert_rowid();
        self.create_gateway(
            id,
            Gateway {
                name: "Gateway 1".into(),
                ..Default::default()
            },
        )?;
        self.project(id)
    }

    pub fn project(&self, id: i64) -> Result<Project> {
        self.conn
            .query_row("SELECT * FROM projects WHERE id = ?1", [id], project_of)
            .optional()?
            .ok_or_else(|| not_found(&tr("Projekt", "Project"), id))
    }

    pub fn update_project(&mut self, id: i64, p: Project) -> Result<Project> {
        if p.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let n = self.conn.execute(
            "UPDATE projects SET name = ?2, customer = ?3, site = ?4, technician = ?5,
             order_no = ?6, remarks = ?7 WHERE id = ?1",
            params![id, p.name.trim(), p.customer, p.site, p.technician, p.order_no, p.remarks],
        )?;
        if n == 0 {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        self.project(id)
    }

    pub fn delete_project(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM projects WHERE id = ?1", [id])? == 0 {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        self.ensure_active()?;
        Ok(())
    }

    /// Neues Gateway mit einer ersten Linie
    pub fn create_gateway(&mut self, project_id: i64, g: Gateway) -> Result<Gateway> {
        self.project(project_id)?;
        let name = if g.name.trim().is_empty() {
            "Gateway".to_string()
        } else {
            g.name.trim().to_string()
        };
        self.conn.execute(
            "INSERT INTO gateways (project_id, name, address, note, position)
             VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM gateways WHERE project_id = ?1))",
            params![project_id, name, g.address.trim(), g.note],
        )?;
        let id = self.conn.last_insert_rowid();
        self.create_line(
            id,
            Line {
                name: format!("{} 1", tr("Linie", "Line")),
                baud: 2400,
                ..Default::default()
            },
        )?;
        self.gateway(id)
    }

    pub fn gateway(&self, id: i64) -> Result<Gateway> {
        self.conn
            .query_row("SELECT * FROM gateways WHERE id = ?1", [id], gateway_of)
            .optional()?
            .ok_or_else(|| not_found("Gateway", id))
    }

    pub fn update_gateway(&mut self, id: i64, g: Gateway) -> Result<Gateway> {
        if g.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let n = self.conn.execute(
            "UPDATE gateways SET name = ?2, address = ?3, note = ?4 WHERE id = ?1",
            params![id, g.name.trim(), g.address.trim(), g.note],
        )?;
        if n == 0 {
            return Err(not_found("Gateway", id));
        }
        self.gateway(id)
    }

    /// Gateway (table "gateways", Eltern "project_id") bzw. Linie ("lines", "gateway_id") um eine
    /// Stelle nach oben (-1) oder unten (+1) schieben
    fn move_row(&mut self, table: &str, parent: &str, id: i64, dir: i64) -> Result<()> {
        let parent_id: i64 = self
            .conn
            .query_row(&format!("SELECT {parent} FROM {table} WHERE id = ?1"), [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| not_found(table, id))?;
        let mut st = self
            .conn
            .prepare(&format!("SELECT id FROM {table} WHERE {parent} = ?1 ORDER BY position, id"))?;
        let mut ids: Vec<i64> = st.query_map([parent_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        drop(st);
        let k = ids.iter().position(|x| *x == id).unwrap_or(0);
        let j = k as i64 + dir.signum();
        if j < 0 || j >= ids.len() as i64 {
            return Ok(());
        }
        ids.swap(k, j as usize);
        let tx = self.conn.transaction()?;
        for (pos, row) in ids.iter().enumerate() {
            tx.execute(&format!("UPDATE {table} SET position = ?2 WHERE id = ?1"), params![row, pos as i64 + 1])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_gateway(&mut self, id: i64, dir: i64) -> Result<()> {
        self.move_row("gateways", "project_id", id, dir)
    }

    pub fn move_line(&mut self, id: i64, dir: i64) -> Result<()> {
        self.move_row("lines", "gateway_id", id, dir)
    }

    pub fn delete_gateway(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM gateways WHERE id = ?1", [id])? == 0 {
            return Err(not_found("Gateway", id));
        }
        self.ensure_active()?;
        Ok(())
    }

    fn check_line(l: &Line) -> Result<()> {
        if l.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        if !BAUD_RATES.contains(&l.baud) {
            bail!(
                "{}: {}",
                tr("Keine Standard-Baudrate", "Not a standard baud rate"),
                l.baud
            );
        }
        Ok(())
    }

    pub fn create_line(&mut self, gateway_id: i64, l: Line) -> Result<Line> {
        self.gateway(gateway_id)?;
        Self::check_line(&l)?;
        self.conn.execute(
            "INSERT INTO lines (gateway_id, name, connection, baud, timeout_ms, position)
             VALUES (?1, ?2, ?3, ?4, ?5, (SELECT COALESCE(MAX(position), 0) + 1 FROM lines WHERE gateway_id = ?1))",
            params![gateway_id, l.name.trim(), l.connection.trim(), l.baud, l.timeout_ms as i64],
        )?;
        self.line(self.conn.last_insert_rowid())
    }

    pub fn line(&self, id: i64) -> Result<Line> {
        self.conn
            .query_row("SELECT * FROM lines WHERE id = ?1", [id], line_of)
            .optional()?
            .ok_or_else(|| not_found(&tr("Linie", "Line"), id))
    }

    pub fn update_line(&mut self, id: i64, l: Line) -> Result<Line> {
        Self::check_line(&l)?;
        let n = self.conn.execute(
            "UPDATE lines SET name = ?2, connection = ?3, baud = ?4, timeout_ms = ?5 WHERE id = ?1",
            params![id, l.name.trim(), l.connection.trim(), l.baud, l.timeout_ms as i64],
        )?;
        if n == 0 {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        self.line(id)
    }

    /// Merken über SA oder PA umstellen und die Zählerliste mit umstellen, wo das eindeutig geht.
    /// Liefert die Anzahl umgestellter Zähler und die, die bleiben mussten (mit Grund).
    pub fn set_addressing(&mut self, id: i64, mode: &str) -> Result<(usize, Vec<String>)> {
        if mode != "secondary" && mode != "primary" {
            bail!("addressing: {mode}");
        }
        self.line(id)?;
        self.conn
            .execute("UPDATE lines SET addressing = ?2 WHERE id = ?1", params![id, mode])?;
        let meters = self.meters(id)?;
        let mut changed = 0;
        let mut kept = Vec::new();
        for m in &meters {
            let name = m.meter_id.clone().unwrap_or_else(|| m.address.clone());
            let target = if mode == "secondary" {
                match &m.secondary_address {
                    Some(sa) if meters.iter().filter(|x| x.secondary_address.as_ref() == Some(sa)).count() == 1 => {
                        Some(sa.clone())
                    }
                    Some(_) => {
                        kept.push(format!("{name}: {}", tr("Sekundäradresse nicht eindeutig", "secondary address not unique")));
                        None
                    }
                    None => {
                        kept.push(format!("{name}: {}", tr("Sekundäradresse unbekannt, einmal auslesen", "secondary address unknown, read once")));
                        None
                    }
                }
            } else {
                match m.primary_address {
                    Some(0) => {
                        kept.push(format!("{name}: {}", tr("PA 0 (Werkszustand), zuerst PA ändern", "PA 0 (factory default), change PA first")));
                        None
                    }
                    Some(pa) if meters.iter().filter(|x| x.primary_address == Some(pa)).count() == 1 => {
                        Some(pa.to_string())
                    }
                    Some(pa) => {
                        kept.push(format!("{name}: PA {pa} {}", tr("doppelt, zuerst PA ändern", "duplicate, change PA first")));
                        None
                    }
                    None => {
                        kept.push(format!("{name}: {}", tr("PA unbekannt, einmal auslesen", "PA unknown, read once")));
                        None
                    }
                }
            };
            if let Some(t) = target {
                if t != m.address {
                    self.conn
                        .execute("UPDATE meters SET address = ?2 WHERE id = ?1", params![m.id, t])?;
                    changed += 1;
                }
            }
        }
        Ok((changed, kept))
    }

    pub fn delete_line(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM lines WHERE id = ?1", [id])? == 0 {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        self.ensure_active()?;
        Ok(())
    }

    /// Verbindungsdaten der Linie beim Verbinden übernehmen
    pub fn set_line_connection(&mut self, id: i64, connection: &str, baud: u32, timeout_ms: u64) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET connection = ?2, baud = ?3, timeout_ms = ?4 WHERE id = ?1",
            params![id, connection, baud, timeout_ms as i64],
        )?;
        Ok(())
    }

    fn lines_of_project(&self, project_id: i64) -> Result<Vec<Line>> {
        let mut st = self.conn.prepare(
            "SELECT lines.* FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
        )?;
        let rows = st.query_map([project_id], line_of)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn active_line(&self) -> Result<Line> {
        let id = self
            .meta("active_line")?
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or_else(|| anyhow!(tr("Keine Linie aktiv", "No line active")))?;
        self.line(id)
    }

    pub fn set_active(&mut self, line_id: i64) -> Result<Line> {
        let line = self.line(line_id)?;
        self.set_meta("active_line", &line_id.to_string())?;
        Ok(line)
    }

    pub fn active(&self) -> Result<Active> {
        let line = self.active_line()?;
        let gateway = self.gateway(line.gateway_id)?;
        let project = self.project(gateway.project_id)?;
        Ok(Active {
            project,
            gateway,
            line,
        })
    }

    /// Alle Projekte mit Gateways und Linien, je mit Anzahl Zähler und letzter Auslesung
    pub fn tree(&self) -> Result<Vec<ProjectNode>> {
        let mut out = Vec::new();
        let mut ps = self.conn.prepare("SELECT * FROM projects ORDER BY name COLLATE NOCASE, id")?;
        let projects: Vec<Project> = ps.query_map([], project_of)?.collect::<rusqlite::Result<_>>()?;
        for project in projects {
            let mut gs = self
                .conn
                .prepare("SELECT * FROM gateways WHERE project_id = ?1 ORDER BY position, id")?;
            let gateways: Vec<Gateway> =
                gs.query_map([project.id], gateway_of)?.collect::<rusqlite::Result<_>>()?;
            let mut gnodes = Vec::new();
            for gateway in gateways {
                let mut ls = self.conn.prepare(
                    "SELECT lines.*,
                       (SELECT COUNT(*) FROM meters WHERE line_id = lines.id) AS n,
                       (SELECT MAX(last_read) FROM meters WHERE line_id = lines.id) AS lr
                     FROM lines WHERE gateway_id = ?1 ORDER BY position, id",
                )?;
                let mut lines: Vec<LineInfo> = ls
                    .query_map([gateway.id], |r| {
                        Ok(LineInfo {
                            line: line_of(r)?,
                            meters: r.get("n")?,
                            last_read: r.get("lr")?,
                            scanned: r.get("scanned")?,
                            issues: r
                                .get::<_, Option<String>>("issues")?
                                .and_then(|t| serde_json::from_str(&t).ok()),
                            plan_info: r
                                .get::<_, Option<String>>("plan_result")?
                                .and_then(|t| serde_json::from_str(&t).ok())
                                .unwrap_or(Value::Null),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                for l in &mut lines {
                    let result = std::mem::take(&mut l.plan_info);
                    l.plan_info = self.line_plan_info(&l.line, &project.plan, result)?;
                }
                gnodes.push(GatewayNode { gateway, lines });
            }
            out.push(ProjectNode {
                project,
                gateways: gnodes,
            });
        }
        Ok(out)
    }

    // ------------------------------------------------------------ Zähler

    pub fn meters(&self, line_id: i64) -> Result<Vec<Meter>> {
        let mut st = self.conn.prepare("SELECT * FROM meters WHERE line_id = ?1 ORDER BY id")?;
        let rows = st.query_map([line_id], meter_of)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn meter(&self, id: i64) -> Result<Meter> {
        self.conn
            .query_row("SELECT * FROM meters WHERE id = ?1", [id], meter_of)
            .optional()?
            .ok_or_else(|| {
                anyhow!(
                    "{} {id}",
                    tr("Zähler nicht in der Liste:", "Meter not in list:")
                )
            })
    }

    fn check_meter(&self, line_id: i64, m: &Meter, skip_id: i64) -> Result<()> {
        let target = Target::parse(&m.address)?;
        if self
            .meters(line_id)?
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
            if !BAUD_RATES.contains(&b) {
                bail!("{}: {b}", tr("Keine Standard-Baudrate", "Not a standard baud rate"));
            }
        }
        Ok(())
    }

    fn insert_meter(&mut self, line_id: i64, m: &Meter) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO meters (line_id, name, location, note, address, baud, meter_id, manufacturer,
               medium, primary_address, last_read, last_value, last_quantity, last_error, secondary_address, plan)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                line_id,
                m.name,
                m.location,
                m.note,
                m.address,
                m.baud,
                m.meter_id,
                m.manufacturer,
                m.medium,
                m.primary_address,
                m.last_read,
                m.last_value,
                m.last_quantity,
                m.last_error,
                m.secondary_address,
                valid_plan(m.plan.as_deref())
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_meter(&mut self, line_id: i64, mut m: Meter) -> Result<Meter> {
        m.address = m.address.trim().to_ascii_uppercase();
        self.check_meter(line_id, &m, 0)?;
        let id = self.insert_meter(line_id, &m)?;
        self.meter(id)
    }

    /// Ändert die vom Benutzer gepflegten Felder, Messinfos bleiben.
    pub fn update_meter(&mut self, id: i64, mut new: Meter) -> Result<Meter> {
        let old = self.meter(id)?;
        new.address = new.address.trim().to_ascii_uppercase();
        self.check_meter(old.line_id, &new, id)?;
        let error = if old.address != new.address {
            None
        } else {
            old.last_error
        };
        self.conn.execute(
            "UPDATE meters SET name = ?2, location = ?3, note = ?4, address = ?5, baud = ?6, last_error = ?7
             WHERE id = ?1",
            params![id, new.name, new.location, new.note, new.address, new.baud, error],
        )?;
        self.meter(id)
    }

    pub fn remove_meter(&mut self, id: i64) -> Result<()> {
        self.meter(id)?;
        self.conn.execute("DELETE FROM meters WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_meters(&mut self, line_id: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM meters WHERE line_id = ?1", [line_id])?)
    }

    /// Fügt Zähler hinzu, deren Adresse noch nicht in der Liste ist. Liefert die Anzahl.
    pub fn import_meters(&mut self, line_id: i64, meters: Vec<Meter>) -> Result<usize> {
        let mut n = 0;
        for mut m in meters {
            m.address = m.address.trim().to_ascii_uppercase();
            if self.check_meter(line_id, &m, 0).is_err() {
                continue;
            }
            self.insert_meter(line_id, &m)?;
            n += 1;
        }
        Ok(n)
    }

    /// Zähler der Linie, der zur Adresse passt
    fn find_meter(&self, line_id: i64, target: &Target) -> Result<Option<Meter>> {
        Ok(self.meters(line_id)?.into_iter().find(|m| listed(m, target)))
    }

    /// Ergebnis einer Auslesung speichern: Verlauf und, falls in der Liste, beim Zähler.
    pub fn record(&mut self, line_id: i64, target: &Target, result: Result<&MeterReading, String>) -> Result<()> {
        let meter = self.find_meter(line_id, target)?;
        match result {
            Ok(r) => {
                self.conn.execute(
                    "INSERT INTO readings (line_id, meter, timestamp, data) VALUES (?1, ?2, ?3, ?4)",
                    params![line_id, meter.as_ref().map(|m| m.id), r.timestamp, serde_json::to_string(r)?],
                )?;
                let Some(m) = meter else {
                    return Ok(());
                };
                let h = r.header.as_ref();
                let main = main_value(r);
                self.conn.execute(
                    "UPDATE meters SET last_read = ?2, last_error = NULL, primary_address = ?3,
                       meter_id = COALESCE(?4, meter_id), manufacturer = COALESCE(?5, manufacturer),
                       medium = COALESCE(?6, medium), last_value = COALESCE(?7, last_value),
                       last_quantity = COALESCE(?8, last_quantity),
                       secondary_address = COALESCE(?9, secondary_address)
                     WHERE id = ?1",
                    params![
                        m.id,
                        r.timestamp,
                        r.primary_address,
                        h.map(|h| h.id.clone()),
                        h.map(|h| h.manufacturer.clone()).filter(|s| !s.is_empty()),
                        h.map(|h| h.medium.clone()),
                        main.map(|x| format!("{} {}", x.value_text, x.unit).trim().to_string()),
                        main.map(|x| x.quantity.clone()),
                        h.map(|h| h.secondary_address.clone())
                    ],
                )?;
            }
            Err(e) => {
                if let Some(m) = meter {
                    self.conn
                        .execute("UPDATE meters SET last_error = ?2 WHERE id = ?1", params![m.id, e])?;
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ Auslesungen

    /// Auslesungen der Linie, älteste zuerst (höchstens `limit` neueste)
    pub fn readings(&self, line_id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        let mut st = self.conn.prepare(
            "SELECT data FROM (SELECT id, data FROM readings WHERE line_id = ?1 ORDER BY id DESC LIMIT ?2)
             ORDER BY id",
        )?;
        let rows = st.query_map(params![line_id, limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for text in rows {
            out.push(serde_json::from_str(&text?)?);
        }
        Ok(out)
    }

    pub fn reading_count(&self, line_id: i64) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM readings WHERE line_id = ?1", [line_id], |r| r.get(0))?)
    }

    pub fn clear_readings(&mut self, line_id: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM readings WHERE line_id = ?1", [line_id])?)
    }

    /// Verlauf eines Zählers: seine Auslesungen, älteste zuerst (höchstens `limit` neueste)
    pub fn meter_history(&self, meter_id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        let mut st = self.conn.prepare(
            "SELECT data FROM (SELECT id, data FROM readings WHERE meter = ?1 ORDER BY id DESC LIMIT ?2)
             ORDER BY id",
        )?;
        let rows = st.query_map(params![meter_id, limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for text in rows {
            out.push(serde_json::from_str(&text?)?);
        }
        Ok(out)
    }

    /// Neueste Auslesung eines Zählers der Liste
    pub fn latest_reading(&self, meter_id: i64) -> Result<Option<MeterReading>> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM readings WHERE meter = ?1 ORDER BY id DESC LIMIT 1",
                [meter_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match text {
            Some(t) => Some(serde_json::from_str(&t)?),
            None => None,
        })
    }

    // ------------------------------------------------------------ Letzter Scan, Änderungen

    pub fn set_last_scan(&mut self, line_id: i64, scan: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2, scanned = ?3 WHERE id = ?1",
            params![line_id, scan.to_string(), now()],
        )?;
        Ok(())
    }

    /// Trefferliste nachführen (z.B. nach "PA ändern"), ohne den Zeitpunkt des Scans zu ändern
    pub fn replace_last_scan(&mut self, line_id: i64, scan: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2 WHERE id = ?1",
            params![line_id, scan.to_string()],
        )?;
        Ok(())
    }

    pub fn last_scan(&self, line_id: i64) -> Result<Option<Value>> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT last_scan FROM lines WHERE id = ?1", [line_id], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(match text {
            Some(t) => Some(serde_json::from_str(&t)?),
            None => None,
        })
    }

    pub fn set_issues(&mut self, line_id: i64, issues: &Value) -> Result<()> {
        if self
            .conn
            .execute("UPDATE lines SET issues = ?2 WHERE id = ?1", params![line_id, issues.to_string()])?
            == 0
        {
            return Err(not_found(&tr("Linie", "Line"), line_id));
        }
        Ok(())
    }

    pub fn add_change(&mut self, line_id: i64, text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO changes (line_id, time, text) VALUES (?1, ?2, ?3)",
            params![line_id, now(), text],
        )?;
        Ok(())
    }

    /// Änderungen eines Projekts, mit Name der Linie
    pub fn changes(&self, project_id: i64) -> Result<Vec<Change>> {
        let mut st = self.conn.prepare(
            "SELECT changes.time, gateways.name || ' · ' || lines.name AS line, changes.text
             FROM changes JOIN lines ON lines.id = changes.line_id JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 ORDER BY changes.id",
        )?;
        let rows = st.query_map([project_id], |r| {
            Ok(Change {
                time: r.get(0)?,
                line: r.get(1)?,
                text: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_changes(&mut self, project_id: i64) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM changes WHERE line_id IN
               (SELECT lines.id FROM lines JOIN gateways ON gateways.id = lines.gateway_id WHERE gateways.project_id = ?1)",
            [project_id],
        )?)
    }

    // ------------------------------------------------------------ Ausleseplan

    pub fn set_project_plan(&mut self, id: i64, plan: &str) -> Result<()> {
        let key = Plan::parse(plan)?.key();
        if self.conn.execute("UPDATE projects SET plan = ?2 WHERE id = ?1", params![id, key])? == 0 {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        Ok(())
    }

    /// None = wie Projekt
    pub fn set_line_plan(&mut self, id: i64, plan: Option<&str>) -> Result<()> {
        let key = plan.map(|p| Plan::parse(p).map(|p| p.key())).transpose()?;
        if self.conn.execute("UPDATE lines SET plan = ?2 WHERE id = ?1", params![id, key])? == 0 {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        Ok(())
    }

    /// None = wie Linie
    pub fn set_meter_plan(&mut self, id: i64, plan: Option<&str>) -> Result<()> {
        let key = plan.map(|p| Plan::parse(p).map(|p| p.key())).transpose()?;
        if self.conn.execute("UPDATE meters SET plan = ?2 WHERE id = ?1", params![id, key])? == 0 {
            return Err(anyhow!("{} {id}", tr("Zähler nicht in der Liste:", "Meter not in list:")));
        }
        Ok(())
    }

    /// Vorgabe des Projekts, zu dem die Linie gehört
    pub fn project_plan_of_line(&self, line_id: i64) -> Result<String> {
        Ok(self.conn.query_row(
            "SELECT projects.plan FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             JOIN projects ON projects.id = gateways.project_id WHERE lines.id = ?1",
            [line_id],
            |r| r.get(0),
        )?)
    }

    /// Alle Linien aller Projekte mit dem gültigen Plan je Zähler (für den Zeitplan)
    pub fn planned(&self) -> Result<Vec<PlannedLine>> {
        let mut st = self.conn.prepare(
            "SELECT lines.*, projects.plan AS project_plan, gateways.name AS gateway_name
             FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             JOIN projects ON projects.id = gateways.project_id ORDER BY lines.id",
        )?;
        let rows: Vec<(Line, String, String)> = st
            .query_map([], |r| Ok((line_of(r)?, r.get("project_plan")?, r.get("gateway_name")?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (line, project_plan, gateway) in rows {
            let meters = self
                .meters(line.id)?
                .into_iter()
                .map(|m| {
                    let (p, _) = plan::effective(m.plan.as_deref(), line.plan.as_deref(), &project_plan);
                    (m, p)
                })
                .collect();
            out.push(PlannedLine {
                label: format!("{gateway} › {}", line.name),
                line,
                meters,
            });
        }
        Ok(out)
    }

    /// Plan neu (oder Uhr weit zurück): auf diesen Termin setzen, ohne auszulesen
    pub fn plan_start(&mut self, meter_id: i64, key: &str, slot: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE meters SET plan_key = ?2, plan_slot = ?3, plan_pause = NULL, plan_fails = 0 WHERE id = ?1",
            params![meter_id, key, slot],
        )?;
        Ok(())
    }

    /// Plan aus: Zustand vergessen
    pub fn plan_reset(&mut self, meter_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE meters SET plan_key = NULL, plan_slot = NULL, plan_pause = NULL, plan_fails = 0 WHERE id = ?1",
            [meter_id],
        )?;
        Ok(())
    }

    /// Termin erledigt; liefert die Anzahl verpasster Termine nacheinander
    pub fn plan_done(&mut self, meter_id: i64, slot: &str, outcome: PlanOutcome) -> Result<i64> {
        match outcome {
            PlanOutcome::Read { ms } => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_fails = 0, plan_pause = NULL,
                   plan_ms = CASE WHEN plan_ms IS NULL THEN ?3 ELSE plan_ms * 0.7 + ?3 * 0.3 END
                 WHERE id = ?1",
                params![meter_id, slot, ms],
            )?,
            PlanOutcome::Failed => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_fails = plan_fails + 1 WHERE id = ?1",
                params![meter_id, slot],
            )?,
            PlanOutcome::Refused { until } => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_pause = ?3 WHERE id = ?1",
                params![meter_id, slot, until],
            )?,
        };
        Ok(self
            .conn
            .query_row("SELECT plan_fails FROM meters WHERE id = ?1", [meter_id], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }

    /// Ergebnis des letzten Termins einer Linie: {"time", "ok", "total", "error"}
    pub fn set_plan_result(&mut self, line_id: i64, result: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET plan_result = ?2 WHERE id = ?1",
            params![line_id, result.to_string()],
        )?;
        Ok(())
    }

    /// Plan eines Zählers für die Oberfläche: gültiger Plan, woher, nächster Termin, Zustand
    pub fn meter_plan_info(m: &Meter, line_plan: Option<&str>, project_plan: &str, now: chrono::NaiveDateTime) -> Value {
        let (p, source) = plan::effective(m.plan.as_deref(), line_plan, project_plan);
        let pause = m.plan_pause.as_deref().and_then(plan::parse_time).filter(|t| *t > now);
        let next = p.next_slot(now).map(|n| match pause {
            Some(t) => p.next_slot(t - chrono::Duration::seconds(1)).unwrap_or(n).max(n),
            None => n,
        });
        json!({
            "effective": p.key(),
            "source": source,
            "next": next.map(|t| t.format(plan::SLOT_FORMAT).to_string()),
            "paused_until": pause.map(|t| t.format(plan::SLOT_FORMAT).to_string()),
            "fails": m.plan_fails,
            "disturbed": p != Plan::Off && m.plan_fails >= DISTURBED_AFTER,
        })
    }

    /// Plan einer Linie für die Projekt-Karte: eigener bzw. geerbter Plan, nächster Termin,
    /// Ergebnis des letzten Termins, gestörte und ausgesetzte Zähler, Buslast
    fn line_plan_info(&self, line: &Line, project_plan: &str, result: Value) -> Result<Value> {
        let now = crate::clock::now();
        let (own, source) = plan::effective(None, line.plan.as_deref(), project_plan);
        let mut planned = 0;
        let mut disturbed = 0;
        let mut paused = 0;
        let mut next: Option<String> = None;
        let mut items = Vec::new();
        for m in self.meters(line.id)? {
            let info = Self::meter_plan_info(&m, line.plan.as_deref(), project_plan, now);
            let p = Plan::parse(info["effective"].as_str().unwrap_or("off")).unwrap_or(Plan::Off);
            if p == Plan::Off {
                continue;
            }
            planned += 1;
            disturbed += info["disturbed"].as_bool().unwrap_or(false) as i64;
            paused += !info["paused_until"].is_null() as i64;
            if let Some(n) = info["next"].as_str() {
                if next.as_deref().is_none_or(|x| n < x) {
                    next = Some(n.to_string());
                }
            }
            let baud = m.baud.unwrap_or(line.baud);
            items.push((p, m.plan_ms.unwrap_or_else(|| plan::estimate_ms(baud))));
        }
        let load = plan::bus_load(items);
        Ok(json!({
            "own": line.plan,
            "effective": own.key(),
            "source": source,
            "meters": planned,
            "next": next,
            "result": result,
            "disturbed": disturbed,
            "paused": paused,
            "load": load,
            "load_warn": load > plan::LOAD_WARN,
        }))
    }

    // ------------------------------------------------------------ Bericht, Export, Import

    /// Alles für den Bericht: Projekt, Gateways, Linien, Zähler mit neuester Auslesung, Änderungen
    pub fn report(&self, project_id: i64) -> Result<Value> {
        let project = self.project(project_id)?;
        let tree = self.tree()?;
        let node = tree
            .into_iter()
            .find(|p| p.project.id == project_id)
            .ok_or_else(|| not_found(&tr("Projekt", "Project"), project_id))?;
        let mut gateways = Vec::new();
        for g in node.gateways {
            let mut lines = Vec::new();
            for l in g.lines {
                let mut meters = Vec::new();
                for m in self.meters(l.line.id)? {
                    let reading = self.latest_reading(m.id)?;
                    meters.push(json!({ "meter": m, "reading": reading }));
                }
                lines.push(json!({ "line": l.line, "meters": meters, "issues": l.issues }));
            }
            gateways.push(json!({ "gateway": g.gateway, "lines": lines }));
        }
        Ok(json!({
            "project": project,
            "gateways": gateways,
            "changes": self.changes(project_id)?,
        }))
    }

    /// Projekt als Datei: Struktur, Zähler, neueste Auslesung je Zähler, Änderungen
    pub fn export_project(&self, project_id: i64) -> Result<Value> {
        let mut r = self.report(project_id)?;
        r["format"] = json!("mbusio-project");
        r["version"] = json!(1);
        Ok(r)
    }

    /// Exportiertes Projekt als neues Projekt einlesen; liefert die neue Projekt-ID
    pub fn import_project(&mut self, v: &Value) -> Result<i64> {
        if v["format"] != "mbusio-project" {
            bail!(tr("Datei ist kein mbusio-Projekt", "File is not an mbusio project"));
        }
        let mut project: Project = serde_json::from_value(v["project"].clone())?;
        // gleicher Name schon vorhanden: in der Auswahl unterscheidbar machen
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1)",
            [&project.name],
            |r| r.get(0),
        )?;
        if taken {
            project.name = format!("{} ({})", project.name, tr("importiert", "imported"));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO projects (name, customer, site, technician, order_no, remarks, created, plan)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.name,
                project.customer,
                project.site,
                project.technician,
                project.order_no,
                project.remarks,
                if project.created.is_empty() { now() } else { project.created.clone() },
                valid_plan(Some(&project.plan)).unwrap_or_else(default_plan)
            ],
        )?;
        let pid = tx.last_insert_rowid();
        let mut line_ids = std::collections::HashMap::new();
        for g in v["gateways"].as_array().into_iter().flatten() {
            let gw: Gateway = serde_json::from_value(g["gateway"].clone())?;
            tx.execute(
                "INSERT INTO gateways (project_id, name, address, note, position)
                 VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM gateways WHERE project_id = ?1))",
                params![pid, gw.name, gw.address, gw.note],
            )?;
            let gid = tx.last_insert_rowid();
            for l in g["lines"].as_array().into_iter().flatten() {
                let line: Line = serde_json::from_value(l["line"].clone())?;
                tx.execute(
                    "INSERT INTO lines (gateway_id, name, connection, baud, timeout_ms, addressing, plan, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, (SELECT COALESCE(MAX(position), 0) + 1 FROM lines WHERE gateway_id = ?1))",
                    params![gid, line.name, line.connection, line.baud, line.timeout_ms as i64, line.addressing,
                        valid_plan(line.plan.as_deref())],
                )?;
                let lid = tx.last_insert_rowid();
                line_ids.insert(format!("{} · {}", gw.name, line.name), lid);
                for entry in l["meters"].as_array().into_iter().flatten() {
                    let m: Meter = serde_json::from_value(entry["meter"].clone())?;
                    tx.execute(
                        "INSERT INTO meters (line_id, name, location, note, address, baud, meter_id, manufacturer,
                           medium, primary_address, last_read, last_value, last_quantity, last_error, secondary_address, plan)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                        params![
                            lid, m.name, m.location, m.note, m.address, m.baud, m.meter_id, m.manufacturer,
                            m.medium, m.primary_address, m.last_read, m.last_value, m.last_quantity, m.last_error,
                            m.secondary_address, valid_plan(m.plan.as_deref())
                        ],
                    )?;
                    let mid = tx.last_insert_rowid();
                    if let Some(r) = entry.get("reading").filter(|r| !r.is_null()) {
                        tx.execute(
                            "INSERT INTO readings (line_id, meter, timestamp, data) VALUES (?1, ?2, ?3, ?4)",
                            params![lid, mid, r["timestamp"].as_str().unwrap_or(""), r.to_string()],
                        )?;
                    }
                }
            }
        }
        for c in v["changes"].as_array().into_iter().flatten() {
            let c: Change = serde_json::from_value(c.clone())?;
            if let Some(lid) = line_ids.get(&c.line) {
                tx.execute(
                    "INSERT INTO changes (line_id, time, text) VALUES (?1, ?2, ?3)",
                    params![lid, c.time, c.text],
                )?;
            }
        }
        tx.commit()?;
        Ok(pid)
    }
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
    fn starts_with_default_project_and_active_line() {
        let db = Db::memory().unwrap();
        let a = db.active().unwrap();
        assert_eq!(a.gateway.name, "Gateway 1");
        assert_eq!(a.line.baud, 2400);
        assert_eq!(db.tree().unwrap().len(), 1);
    }

    #[test]
    fn meters_are_per_line_and_reject_duplicates() {
        let mut db = Db::memory().unwrap();
        let l1 = db.active_line().unwrap().id;
        let gw = db.active().unwrap().gateway.id;
        let l2 = db
            .create_line(gw, Line { name: "Linie 2".into(), baud: 9600, ..Default::default() })
            .unwrap()
            .id;
        db.add_meter(l1, meter("12345678")).unwrap();
        // ID, Maske und volle Sekundäradresse sind derselbe Zähler
        assert!(db.add_meter(l1, meter("12345678ffffffff")).is_err());
        assert!(db.add_meter(l1, meter("123456782C2D1B04")).is_err());
        assert!(db.add_meter(l1, meter("x")).is_err());
        // auf einer anderen Linie erlaubt
        db.add_meter(l2, meter("12345678")).unwrap();
        assert_eq!(db.import_meters(l1, vec![meter("5"), meter("6"), meter("5")]).unwrap(), 2);
        assert_eq!(db.meters(l1).unwrap().len(), 3);
        assert_eq!(db.clear_meters(l1).unwrap(), 3);
        assert_eq!(db.meters(l2).unwrap().len(), 1);
    }

    #[test]
    fn gateways_and_lines_can_be_reordered() {
        let mut db = Db::memory().unwrap();
        let p = db.active().unwrap().project.id;
        let g2 = db.create_gateway(p, Gateway { name: "B".into(), ..Default::default() }).unwrap();
        let g3 = db.create_gateway(p, Gateway { name: "C".into(), ..Default::default() }).unwrap();
        let names = |db: &Db| -> Vec<String> {
            db.tree().unwrap()[0].gateways.iter().map(|g| g.gateway.name.clone()).collect()
        };
        assert_eq!(names(&db), ["Gateway 1", "B", "C"]);
        db.move_gateway(g3.id, -1).unwrap();
        assert_eq!(names(&db), ["Gateway 1", "C", "B"]);
        db.move_gateway(g3.id, -1).unwrap();
        db.move_gateway(g3.id, -1).unwrap(); // schon oben: nichts passiert
        assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
        db.move_gateway(g2.id, 1).unwrap(); // schon unten
        assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
        let l2 = db.create_line(g2.id, Line { name: "L2".into(), baud: 2400, ..Default::default() }).unwrap();
        db.move_line(l2.id, -1).unwrap();
        let lines: Vec<String> = db.tree().unwrap()[0].gateways[2].lines.iter().map(|l| l.line.name.clone()).collect();
        assert_eq!(lines[0], "L2");
    }

    #[test]
    fn deleting_the_active_project_keeps_a_line_active() {
        let mut db = Db::memory().unwrap();
        let p2 = db.create_project(Project { name: "B".into(), ..Default::default() }).unwrap();
        let first = db.active().unwrap().project.id;
        db.delete_project(first).unwrap();
        assert_eq!(db.active().unwrap().project.id, p2.id);
        // letztes Projekt gelöscht: neues Standardprojekt
        db.delete_project(p2.id).unwrap();
        assert_eq!(db.tree().unwrap().len(), 1);
    }

    #[test]
    fn export_and_import_round_trip() {
        let mut db = Db::memory().unwrap();
        let a = db.active().unwrap();
        db.update_project(a.project.id, Project { name: "MFH".into(), customer: "Muster AG".into(), ..Default::default() })
            .unwrap();
        db.add_meter(a.line.id, Meter { name: "Whg 1".into(), ..meter("3") }).unwrap();
        db.add_change(a.line.id, "3: PA 0 → 3").unwrap();
        let file = db.export_project(a.project.id).unwrap();
        let id = db.import_project(&file).unwrap();
        let r = db.report(id).unwrap();
        assert_eq!(r["project"]["customer"], "Muster AG");
        assert_eq!(r["gateways"][0]["lines"][0]["meters"][0]["meter"]["name"], "Whg 1");
        assert_eq!(r["changes"][0]["text"], "3: PA 0 → 3");
        assert!(db.import_project(&json!({ "format": "x" })).is_err());
    }

    #[test]
    fn imports_old_meter_list() {
        let dir = std::env::temp_dir().join(format!("mbusio-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("meters.json"), r#"{"meters":[{"address":"5","name":"Alt"}]}"#).unwrap();
        let mut db = Db::open(&dir).unwrap();
        let line = db.active_line().unwrap().id;
        assert_eq!(db.meters(line).unwrap()[0].name, "Alt");
        db.set_issues(line, &json!({ "count": 2, "items": ["a", "b"] })).unwrap();
        assert_eq!(db.tree().unwrap()[0].gateways[0].lines[0].issues.as_ref().unwrap()["count"], 2);
        assert!(dir.join("meters.json.imported").exists());
        drop(db);
        // zweiter Start: nichts doppelt
        let db = Db::open(&dir).unwrap();
        assert_eq!(db.meters(line).unwrap().len(), 1);
        drop(db);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
