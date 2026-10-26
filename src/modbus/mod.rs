//! Modbus-TCP-Server (Konzept Modbus): stellt die Datenpunkte eines Projekts als Register bereit,
//! nur zum Lesen (Funktionen 03 und 04).
//!
//! - Je Datenpunkt ein Block aus 10 Registern ab (Nummer − 1) × 10: Wert als float64, float32 und
//!   int32 × 100, Qualität, Alter in Minuten; ab 60000 der Zustand von mbusio (`image`).
//! - Anfragen werden aus einem Registerbild im Speicher beantwortet, das bei jeder Änderung
//!   (`Db.changes`) und einmal pro Minute neu entsteht: ein Leitsystem, das jede Sekunde liest,
//!   hält mbusio nicht auf.
//! - Eigener kleiner Server (`protocol`), keine Bibliothek.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::db::Db;
use crate::i18n::{both, tr};

mod image;
mod protocol;
#[cfg(test)]
mod tests;

pub use image::{block_start, BLOCK, STATUS_START};

/// Bild neu aufbauen spätestens nach dieser Zeit (Alter, „veraltet“)
const REFRESH: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------- Einstellungen

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModbusConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Netzwerkschnittstelle; Modbus hat keine Zugangskontrolle
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Projekt, dessen Datenpunkte bereitstehen; 0 = das aktive
    #[serde(default)]
    pub project: i64,
    /// Niederwertiges Wort zuerst (CDAB) statt Modbus-üblich höherwertiges zuerst (ABCD)
    #[serde(default)]
    pub swap_words: bool,
}

fn default_bind() -> String {
    "0.0.0.0".into()
}

fn default_port() -> u16 {
    502
}

impl Default for ModbusConfig {
    fn default() -> Self {
        ModbusConfig {
            enabled: false,
            bind: default_bind(),
            port: default_port(),
            project: 0,
            swap_words: false,
        }
    }
}

impl ModbusConfig {
    pub fn check(&self) -> Result<()> {
        if self.bind.trim().parse::<std::net::IpAddr>().is_err() {
            bail!(tr(
                "Adresse ungültig (IP-Adresse, z.B. 0.0.0.0 für alle)",
                "Invalid address (IP address, e.g. 0.0.0.0 for all)"
            ));
        }
        if self.port == 0 {
            bail!(tr("Port ungültig", "Invalid port"));
        }
        Ok(())
    }
}

impl Db {
    pub fn modbus_config(&self) -> ModbusConfig {
        self.meta_json("modbus")
    }

    pub fn set_modbus_config(&mut self, c: &ModbusConfig) -> Result<()> {
        c.check()?;
        if c.project != 0 {
            self.project(c.project)?;
        }
        self.set_meta("modbus", &serde_json::to_string(c)?)
    }

    /// Projekt, dessen Register der Server bereitstellt
    pub fn modbus_project(&self) -> Result<i64> {
        match self.modbus_config().project {
            0 => Ok(self.active()?.project.id),
            id => Ok(id),
        }
    }
}

// ---------------------------------------------------------------- Zustand

#[derive(Debug, Clone, Default, Serialize)]
pub struct ModbusStatus {
    /// "off", "listening", "error"
    pub state: String,
    /// worauf der Server hört, z.B. 0.0.0.0:502
    pub listen: String,
    pub error: Option<String>,
    /// verbundene Leitsysteme
    pub clients: u64,
    /// beantwortete Anfragen seit dem Start
    pub requests: u64,
}

pub struct ModbusState {
    pub status: Mutex<ModbusStatus>,
    /// erhöht bei jeder Änderung der Einstellungen: neu starten
    pub version: AtomicU64,
    /// aktuelles Registerbild (65536 Register)
    image: RwLock<Arc<Vec<u16>>>,
    start: Instant,
}

impl Default for ModbusState {
    fn default() -> Self {
        ModbusState {
            status: Mutex::default(),
            version: AtomicU64::default(),
            image: RwLock::new(Arc::new(vec![0; 65536])),
            start: Instant::now(),
        }
    }
}

fn set_status(st: &crate::web::AppState, f: impl FnOnce(&mut ModbusStatus)) {
    f(&mut st.modbus.status.lock().unwrap());
}

/// Registerbild aus dem aktuellen Stand der Datenbank
fn rebuild(st: &crate::web::AppState) {
    let uptime_min = (st.modbus.start.elapsed().as_secs() / 60) as u16;
    let built = {
        let db = st.db.lock().unwrap();
        let cfg = db.modbus_config();
        db.modbus_project()
            .and_then(|p| db.dp_list(None, Some(p)))
            .map(|rows| image::build(&rows, cfg.swap_words, crate::clock::utc_secs(), uptime_min))
    };
    match built {
        Ok(regs) => *st.modbus.image.write().unwrap() = Arc::new(regs),
        Err(e) => st.log.push("INFO", format!("Modbus: {e:#}")),
    }
}

/// Läuft als Task, solange mbusio läuft: Server nach den Einstellungen, Registerbild aktuell halten
pub async fn run(st: crate::web::AppState) {
    let mut changes = st.db.lock().unwrap().changes.subscribe();
    loop {
        let version = st.modbus.version.load(Ordering::SeqCst);
        let cfg = st.db.lock().unwrap().modbus_config();
        if !cfg.enabled {
            set_status(&st, |s| {
                *s = ModbusStatus {
                    state: "off".into(),
                    ..Default::default()
                }
            });
            wait_for_new_settings(&st, version, Duration::from_secs(3600)).await;
            continue;
        }
        let listen = format!("{}:{}", cfg.bind.trim(), cfg.port);
        let listener = match TcpListener::bind(&listen).await {
            Ok(l) => l,
            Err(e) => {
                let msg = bind_error(&e, cfg.port);
                st.log.push(
                    "INFO",
                    both(|| format!("Modbus {listen}: {}", bind_error(&e, cfg.port))),
                );
                set_status(&st, |s| {
                    *s = ModbusStatus {
                        state: "error".into(),
                        listen: listen.clone(),
                        error: Some(msg),
                        ..Default::default()
                    }
                });
                wait_for_new_settings(&st, version, Duration::from_secs(10)).await;
                continue;
            }
        };
        st.log.push(
            "INFO",
            both(|| format!("Modbus {} {listen}", tr("hört auf", "listening on"))),
        );
        set_status(&st, |s| {
            *s = ModbusStatus {
                state: "listening".into(),
                listen: listen.clone(),
                ..Default::default()
            }
        });
        rebuild(&st);
        let mut tick = tokio::time::interval(REFRESH);
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    if let Ok((socket, _)) = accepted {
                        tokio::spawn(client(st.clone(), socket, version));
                    }
                }
                r = changes.changed() => {
                    if r.is_err() { return; }
                    rebuild(&st);
                }
                _ = tick.tick() => rebuild(&st),
                _ = wait_for_new_settings(&st, version, Duration::from_secs(3600)) => break,
            }
        }
    }
}

/// Bis die Einstellungen sich ändern oder `max` vorbei ist
async fn wait_for_new_settings(st: &crate::web::AppState, version: u64, max: Duration) {
    let until = Instant::now() + max;
    while Instant::now() < until && st.modbus.version.load(Ordering::SeqCst) == version {
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

fn bind_error(e: &std::io::Error, port: u16) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied if port < 1024 => tr(
            "Port unter 1024 braucht Rechte (Dienst als root oder setcap), sonst z.B. 1502 wählen",
            "Port below 1024 needs privileges (service as root or setcap), otherwise choose e.g. 1502",
        ),
        std::io::ErrorKind::AddrInUse => tr("Port ist schon belegt", "Port already in use"),
        std::io::ErrorKind::AddrNotAvailable => tr(
            "Adresse gibt es auf diesem Gerät nicht",
            "Address does not exist on this device",
        ),
        _ => e.to_string(),
    }
}

/// Ein verbundenes Leitsystem: Anfragen beantworten, bis es trennt oder die Einstellungen sich ändern
async fn client(st: crate::web::AppState, mut socket: TcpStream, version: u64) {
    set_status(&st, |s| s.clients += 1);
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    while st.modbus.version.load(Ordering::SeqCst) == version {
        let n = match tokio::time::timeout(Duration::from_secs(1), socket.read(&mut chunk)).await {
            Err(_) => continue, // nichts gekommen: Einstellungen erneut prüfen
            Ok(Ok(0)) | Ok(Err(_)) => break,
            Ok(Ok(n)) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        let image = st.modbus.image.read().unwrap().clone();
        let mut out = Vec::new();
        loop {
            match protocol::next_request(&buf) {
                protocol::Next::Need => break,
                protocol::Next::Garbage => {
                    buf.clear();
                    break;
                }
                protocol::Next::Request(len) => {
                    out.extend(protocol::answer(&buf[..len], &image));
                    buf.drain(..len);
                    set_status(&st, |s| s.requests += 1);
                }
            }
        }
        if !out.is_empty() && socket.write_all(&out).await.is_err() {
            break;
        }
    }
    set_status(&st, |s| s.clients = s.clients.saturating_sub(1));
}
