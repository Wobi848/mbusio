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
    /// Verlauf ausdünnen: nach so vielen Tagen nur noch ein Wert pro Zähler und Tag; None = alles behalten
    #[serde(default)]
    pub thin_days: Option<i64>,
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
    /// Dauerbetrieb eingeschaltet: nur dann liest der Plan und wird weitergegeben (§13).
    /// Wird nie importiert: eine übernommene Anlage liest erst, wenn man sie einschaltet.
    #[serde(default)]
    pub operation: bool,
}

/// Nur für Projekt-Exporte ohne das Feld: die stammen aus der Zeit, als über SA gemerkt wurde.
/// Neue Linien merken über PA (create_line).
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
    /// Uhr im Zähler weicht so viele Minuten ab (ab 5 min, bei der letzten Auslesung)
    #[serde(default, skip_deserializing)]
    pub clock_off: Option<i64>,
}

/// Abweichung der Uhr im Zähler von der eigenen Uhr in Minuten, ab `CLOCK_OFF_MIN`. Nur wenn die
/// eigene Uhr bestätigt ist; Stichtagswerte und Monatswerte hängen an der Zähleruhr.
pub(crate) fn clock_offset(r: &MeterReading) -> Option<i64> {
    if r.time_unsure {
        return None;
    }
    let ours = chrono::NaiveDateTime::parse_from_str(&r.timestamp, "%Y-%m-%d %H:%M:%S").ok()?;
    let rec = r
        .records
        .iter()
        .find(|x| x.quantity == "date_time" && x.storage == 0 && x.function == "instantaneous")?;
    let theirs = chrono::NaiveDateTime::parse_from_str(&rec.value_text, "%Y-%m-%d %H:%M").ok()?;
    let off = (theirs - ours).num_seconds() as f64 / 60.0;
    (off.abs() >= CLOCK_OFF_MIN as f64).then_some(off.round() as i64)
}

const CLOCK_OFF_MIN: i64 = 5;

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
    /// Datenpunkte der Linie: {"count": n, "invalid": n}
    pub datapoints: Value,
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

mod meters;
mod projects;
mod readings;
mod report;
mod schedule;
mod schema;

// ---------------------------------------------------------------- Datenbank

pub struct Db {
    pub(crate) conn: Connection,
    path: Option<PathBuf>,
    /// Speicher fast voll: keine neuen Auslesungen mehr speichern (Zählerliste wird weiter
    /// aktualisiert), damit die Datenbank intakt und das Gerät bedienbar bleibt
    pub readings_paused: bool,
    /// Meldungen beim Speichern (z.B. Zählerstand gesunken); der Aufrufer schreibt sie ins Protokoll
    pub notes: Vec<String>,
    /// MQTT eingeschaltet: nur dann Werte für den Broker vormerken
    pub mqtt_enabled: bool,
}

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
        thin_days: r.get("thin_days")?,
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
        operation: r.get("operation")?,
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
        clock_off: r.get("clock_off")?,
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
            readings_paused: false,
            notes: Vec::new(),
            mqtt_enabled: false,
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
            readings_paused: false,
            notes: Vec::new(),
            mqtt_enabled: false,
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
mod tests {
    use super::*;

    fn meter(address: &str) -> Meter {
        Meter {
            address: address.into(),
            ..Default::default()
        }
    }

    #[test]
    fn update_keeps_gateway_lines_with_plan_in_operation() {
        let setup = || {
            let mut db = Db::memory().unwrap();
            let line = db.active_line().unwrap();
            db.update_line(
                line.id,
                Line {
                    connection: "COM3".into(),
                    ..line.clone()
                },
            )
            .unwrap();
            db.add_meter(line.id, meter("5")).unwrap();
            db.set_project_plan(db.active().unwrap().project.id, "daily 00:05")
                .unwrap();
            // Datenbank von vor dem Schalter
            db.conn
                .execute_batch("ALTER TABLE lines DROP COLUMN operation")
                .unwrap();
            (db, line.id)
        };
        // Laptop (Fenster): alles aus
        let (mut db, id) = setup();
        db.migrate().unwrap();
        assert!(!db.line(id).unwrap().operation);
        // Gateway (Dienst): Linie mit wirksamem Plan liest weiter
        let (mut db, id) = setup();
        crate::service::set_background_for_tests(true);
        let r = db.migrate();
        crate::service::set_background_for_tests(false);
        r.unwrap();
        assert!(db.line(id).unwrap().operation);
    }

    #[test]
    fn starts_with_default_project_and_active_line() {
        let db = Db::memory().unwrap();
        let a = db.active().unwrap();
        assert_eq!(a.gateway.name, "Gateway 1");
        assert_eq!(a.line.baud, 2400);
        assert_eq!(a.line.addressing, "primary", "neue Linien merken über PA");
        assert_eq!(db.tree().unwrap().len(), 1);
    }

    #[test]
    fn complete_scans_are_remembered_per_kind_and_scope() {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap().id;
        let all = "0-250 [2400]";
        let scan = |pa: u8| json!({ "kind": "primary", "scope": all, "complete": true, "hits": [{ "primary_address": pa }] });
        let prev = |db: &Db, scope: &str| db.previous_scan(line, "primary", scope).unwrap();
        assert!(
            prev(&db, all).is_none(),
            "erster Scan: nichts zu vergleichen"
        );
        db.set_last_scan(line, &scan(1), true).unwrap();
        // abgebrochener Scan wird nicht gemerkt
        db.set_last_scan(line, &scan(9), false).unwrap();
        assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 1);
        assert!(
            prev(&db, "0-12 [2400]").is_none(),
            "anderer Bereich: kein Vergleich"
        );
        // „PA ändern“ führt den gemerkten Scan nach: keine Änderung beim nächsten Vergleich
        db.replace_last_scan(line, &scan(5)).unwrap();
        assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 5);
        // nach einem abgebrochenen Scan verkürzt „PA ändern“ den gemerkten Scan nicht
        let mut cut = scan(7);
        cut["complete"] = json!(false);
        cut["hits"] = json!([]);
        db.replace_last_scan(line, &cut).unwrap();
        assert_eq!(prev(&db, all).unwrap()["hits"][0]["primary_address"], 5);
    }

    #[test]
    fn meters_are_per_line_and_reject_duplicates() {
        let mut db = Db::memory().unwrap();
        let l1 = db.active_line().unwrap().id;
        let gw = db.active().unwrap().gateway.id;
        let l2 = db
            .create_line(
                gw,
                Line {
                    name: "Linie 2".into(),
                    baud: 9600,
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        db.add_meter(l1, meter("12345678")).unwrap();
        // ID, Maske und volle Sekundäradresse sind derselbe Zähler
        assert!(db.add_meter(l1, meter("12345678ffffffff")).is_err());
        assert!(db.add_meter(l1, meter("123456782C2D1B04")).is_err());
        assert!(db.add_meter(l1, meter("x")).is_err());
        // auf einer anderen Linie erlaubt
        db.add_meter(l2, meter("12345678")).unwrap();
        assert_eq!(
            db.import_meters(l1, vec![meter("5"), meter("6"), meter("5")])
                .unwrap(),
            2
        );
        assert_eq!(db.meters(l1).unwrap().len(), 3);
        assert_eq!(db.clear_meters(l1).unwrap(), 3);
        assert_eq!(db.meters(l2).unwrap().len(), 1);
    }

    #[test]
    fn gateways_and_lines_can_be_reordered() {
        let mut db = Db::memory().unwrap();
        let p = db.active().unwrap().project.id;
        let g2 = db
            .create_gateway(
                p,
                Gateway {
                    name: "B".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let g3 = db
            .create_gateway(
                p,
                Gateway {
                    name: "C".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let names = |db: &Db| -> Vec<String> {
            db.tree().unwrap()[0]
                .gateways
                .iter()
                .map(|g| g.gateway.name.clone())
                .collect()
        };
        assert_eq!(names(&db), ["Gateway 1", "B", "C"]);
        db.move_gateway(g3.id, -1).unwrap();
        assert_eq!(names(&db), ["Gateway 1", "C", "B"]);
        db.move_gateway(g3.id, -1).unwrap();
        db.move_gateway(g3.id, -1).unwrap(); // schon oben: nichts passiert
        assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
        db.move_gateway(g2.id, 1).unwrap(); // schon unten
        assert_eq!(names(&db), ["C", "Gateway 1", "B"]);
        let l2 = db
            .create_line(
                g2.id,
                Line {
                    name: "L2".into(),
                    baud: 2400,
                    ..Default::default()
                },
            )
            .unwrap();
        db.move_line(l2.id, -1).unwrap();
        let lines: Vec<String> = db.tree().unwrap()[0].gateways[2]
            .lines
            .iter()
            .map(|l| l.line.name.clone())
            .collect();
        assert_eq!(lines[0], "L2");
    }

    #[test]
    fn deleting_the_active_project_keeps_a_line_active() {
        let mut db = Db::memory().unwrap();
        let p2 = db
            .create_project(Project {
                name: "B".into(),
                ..Default::default()
            })
            .unwrap();
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
        db.update_project(
            a.project.id,
            Project {
                name: "MFH".into(),
                customer: "Muster AG".into(),
                ..Default::default()
            },
        )
        .unwrap();
        db.add_meter(
            a.line.id,
            Meter {
                name: "Whg 1".into(),
                ..meter("3")
            },
        )
        .unwrap();
        db.add_change(a.line.id, "3: PA 0 → 3").unwrap();
        let file = db.export_project(a.project.id).unwrap();
        let id = db.import_project(&file).unwrap();
        let r = db.report(id).unwrap();
        assert_eq!(r["project"]["customer"], "Muster AG");
        assert_eq!(
            r["gateways"][0]["lines"][0]["meters"][0]["meter"]["name"],
            "Whg 1"
        );
        assert_eq!(r["changes"][0]["text"], "3: PA 0 → 3");
        assert!(db.import_project(&json!({ "format": "x" })).is_err());
    }

    #[test]
    fn thinning_keeps_one_reading_per_day() {
        let mut db = Db::memory().unwrap();
        let line = db.active_line().unwrap().id;
        let m = db.add_meter(line, meter("1")).unwrap();
        for day in 1..=10 {
            for hour in [0, 6, 12, 18] {
                db.conn
                    .execute(
                        "INSERT INTO readings (line_id, meter, timestamp, data) VALUES (?1, ?2, ?3, '{}')",
                        params![line, m.id, format!("2026-10-{day:02} {hour:02}:00:00")],
                    )
                    .unwrap();
            }
        }
        let now =
            chrono::NaiveDateTime::parse_from_str("2026-10-10 12:00", "%Y-%m-%d %H:%M").unwrap();
        // ausgeschaltet: nichts löschen
        assert_eq!(db.thin_readings(now).unwrap(), 0);
        db.set_thin_days(1, Some(5)).unwrap();
        // älter als 5 Tage (vor dem 5.): Tag 1–4 je 3 von 4 weg
        assert_eq!(db.thin_readings(now).unwrap(), 12);
        let left: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 40 - 12);
        let first: String = db
            .conn
            .query_row(
                "SELECT timestamp FROM readings ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(first, "2026-10-01 00:00:00");
    }

    #[test]
    fn imports_old_meter_list() {
        let dir = std::env::temp_dir().join(format!("mbusio-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("meters.json"),
            r#"{"meters":[{"address":"5","name":"Alt"}]}"#,
        )
        .unwrap();
        let mut db = Db::open(&dir).unwrap();
        let line = db.active_line().unwrap().id;
        assert_eq!(db.meters(line).unwrap()[0].name, "Alt");
        db.set_issues(line, &json!({ "count": 2, "items": ["a", "b"] }))
            .unwrap();
        assert_eq!(
            db.tree().unwrap()[0].gateways[0].lines[0]
                .issues
                .as_ref()
                .unwrap()["count"],
            2
        );
        assert!(dir.join("meters.json.imported").exists());
        drop(db);
        // zweiter Start: nichts doppelt
        let db = Db::open(&dir).unwrap();
        assert_eq!(db.meters(line).unwrap().len(), 1);
        drop(db);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
