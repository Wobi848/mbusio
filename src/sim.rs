//! Simulator: virtueller M-Bus mit mehreren Slaves, zum Testen ohne Hardware.
//!
//! Belegung:
//! - Adr. 1: Wärmezähler, ID 12345678, Antwort in zwei Telegrammen
//! - Adr. 2: Elektrizitätszähler, ID 87654321
//! - Adr. 3: Wasserzähler, ID 30000001
//! - Adr. 0: zwei unkonfigurierte Wasserzähler (ID 12345699 und 55500011),
//!   kollidieren bei Primäradressierung, nur per Sekundäradresse erreichbar
//! - Adr. 4: Zähler mit falscher Prüfsumme im Datentelegramm
//! - Adr. 5: Zähler, dessen Datentelegramm abgeschnitten ist
//! - Adr. 6: Wasserzähler, ID 60000006, läuft mit 9600 Bd (bzw. 2400 Bd, wenn mit 9600 verbunden)
//! - Adr. 7: alter Wärmezähler, ID 70000007, feste Datenstruktur (CI 73)
//!
//! Typische Fehler aus der Praxis:
//! - Adr. 9: uralter Wasserzähler, ID 90000009, läuft mit 300 Bd
//! - Adr. 10: zwei Wärmezähler (ID 10000010 und 10000011) mit derselben Primäradresse
//! - Adr. 11: Zähler meldet "beschäftigt" (Fehlertelegramm CI 70), ID 11000011
//! - Adr. 12: Zähler antwortet mit falscher Adresse (13), ID 12000012
//! - Adr. 14 und 15: zwei Zähler mit derselben ID 77777777, verschiedene Hersteller
//! - Adr. 16: Wackelkontakt, antwortet nur auf jedes zweite Telegramm, ID 16000016
//! - Adr. 17 und 18: zwei Zähler mit derselben ID 88888888 und demselben Hersteller (echter Fehler)
//! - Adr. 19: Batteriezähler mit Kontingent, ID 19000019: liefert 2 Auslesungen, danach
//!   „zu viele Auslesungen“ (Fehlertelegramm CI 70, Code 09)
//!
//! Zähler hören nur Telegramme mit ihrer eigenen Baudrate. Baudratenwechsel (CI B8..BF)
//! und Application Reset (CI 50) werden quittiert.
//!
//! Das ist die Simulation „Praxis mit Fehlern“ (Anschluss `SIMULATOR`). Weitere Simulationen sind
//! Listen von [`SimMeter`]: eingebaute Vorlagen (z.B. „Alles in Ordnung – 10 Zähler“) und eigene,
//! in der Oberfläche angelegte (Anschluss `SIMULATOR:<name>`).

use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use chrono::{Datelike, Local, Timelike};

use crate::link::Link;
use crate::mbus::frame::{
    self, Frame, Parse, ACK, ADDR_BROADCAST, ADDR_BROADCAST_REPLY, ADDR_SELECTED, CI_APP_RESET,
    CI_DATA_SEND, CI_SELECT,
};
use crate::mbus::master::BAUD_RATES;
use crate::i18n::tr;
use crate::mbus::tables::manufacturer_code;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    Checksum,
    Truncated,
    /// antwortet mit Fehlertelegramm CI 70 "beschäftigt"
    Busy,
    /// trägt die falsche Primäradresse ins Antworttelegramm ein
    WrongAddress,
    /// Wackelkontakt: jedes zweite Telegramm kommt nicht an
    Flaky,
    /// Kontingent aufgebraucht: nach 2 Auslesungen Fehlertelegramm CI 70 "zu viele Auslesungen"
    TooMany,
}

#[derive(Clone, Copy)]
enum Model {
    Heat,
    Electricity,
    Water,
    /// Feste Datenstruktur (CI 73), wie sehr alte Zähler
    Fixed,
}

struct Slave {
    primary: u8,
    id: u32,
    manufacturer: u16,
    version: u8,
    medium: u8,
    model: Model,
    fault: Fault,
    access: u8,
    baud: u32,
    selected: bool,
    /// für Fault::Flaky: hat das letzte Telegramm gehört
    heard_last: bool,
    part: usize,
    last_fcb: Option<bool>,
    /// für Fault::TooMany: bisherige Auslesungen
    readouts: u32,
    /// Versatz der Werte, damit nicht alle Zähler gleich viel anzeigen
    seed: u32,
}

impl Slave {
    fn new(primary: u8, id: u32, man: &str, version: u8, medium: u8, model: Model) -> Self {
        Slave {
            primary,
            id,
            manufacturer: manufacturer_code(man),
            version,
            medium,
            model,
            fault: Fault::None,
            access: 0,
            baud: 0,
            selected: false,
            // so beginnt der Wackelkontakt mit einem verpassten Telegramm
            heard_last: true,
            part: 0,
            last_fcb: None,
            readouts: 0,
            seed: 0,
        }
    }

    fn with_baud(mut self, baud: u32) -> Self {
        self.baud = baud;
        self
    }

    fn with_fault(mut self, fault: Fault) -> Self {
        self.fault = fault;
        self
    }

    /// Kommt das Telegramm an? Bei Wackelkontakt nur jedes zweite.
    fn hears(&mut self) -> bool {
        if self.fault != Fault::Flaky {
            return true;
        }
        self.heard_last = !self.heard_last;
        self.heard_last
    }

    fn reset(&mut self) {
        self.part = 0;
        self.last_fcb = None;
    }

    fn matches(&self, sel: &[u8]) -> bool {
        if sel.len() < 8 {
            return false;
        }
        let id = self.id.to_le_bytes();
        let id_ok = (0..4).all(|k| {
            let (m, v) = (sel[k], id[k]);
            (m >> 4 == 0xF || m >> 4 == v >> 4) && (m & 0xF == 0xF || m & 0xF == v & 0xF)
        });
        let man = u16::from_le_bytes([sel[4], sel[5]]);
        id_ok
            && (man == 0xFFFF || man == self.manufacturer)
            && (sel[6] == 0xFF || sel[6] == self.version)
            && (sel[7] == 0xFF || sel[7] == self.medium)
    }

    fn rsp_ud(&mut self, fcb: bool, elapsed: f64) -> Frame {
        let parts = match self.model {
            Model::Heat => 2,
            _ => 1,
        };
        match self.last_fcb {
            None => self.part = 0,
            Some(last) if last != fcb => self.part = (self.part + 1) % parts,
            Some(_) => {} // gleicher FCB: Wiederholung
        }
        self.last_fcb = Some(fcb);
        self.access = self.access.wrapping_add(1);

        if self.fault == Fault::Busy {
            return Frame::Long {
                control: 0x08,
                address: self.primary,
                ci: 0x70,
                data: vec![0x08],
            };
        }
        if self.fault == Fault::TooMany {
            self.readouts += 1;
            if self.readouts > 2 {
                return Frame::Long {
                    control: 0x08,
                    address: self.primary,
                    ci: 0x70,
                    data: vec![0x09],
                };
            }
        }

        if let Model::Fixed = self.model {
            // ID, Zugriffsnummer, Status (BCD, aktuelle Werte), Medium/Einheiten, 2 Zähler
            let mut d = self.id.to_le_bytes().to_vec();
            // Medium Wärme (4): Bits 7..6 beider Einheitenbytes; Einheiten kWh (05) und m³ (2C)
            d.extend_from_slice(&[self.access, 0x00, 0x05, 0x40 | 0x2C]);
            d.extend_from_slice(&bcd_bytes(12_345 + self.seed as u64 + (elapsed as u64) / 60, 4));
            d.extend_from_slice(&bcd_bytes(678, 4));
            return Frame::Long {
                control: 0x08,
                address: self.primary,
                ci: 0x73,
                data: d,
            };
        }

        let mut d = Vec::new();
        d.extend_from_slice(&self.id.to_le_bytes());
        d.extend_from_slice(&self.manufacturer.to_le_bytes());
        d.extend_from_slice(&[self.version, self.medium, self.access, 0x00, 0x00, 0x00]);
        let tick = elapsed as u32;
        let seed = self.seed;
        match (self.model, self.part) {
            (Model::Heat, 0) => {
                rec(&mut d, &[0x04, 0x06], &(123_456 + seed + tick / 10).to_le_bytes()); // kWh
                rec(&mut d, &[0x04, 0x14], &(456_789 + seed * 3 + tick / 4).to_le_bytes()); // 0.01 m³
                rec(&mut d, &[0x04, 0x2B], &(12_345 + tick % 50).to_le_bytes()); // W
                rec(&mut d, &[0x02, 0x59], &6523i16.to_le_bytes()); // 0.01 °C
                rec(&mut d, &[0x02, 0x5D], &4210i16.to_le_bytes());
                rec(&mut d, &[0x02, 0x61], &2313i16.to_le_bytes()); // 0.01 K
                rec(&mut d, &[0x04, 0x3B], &1234u32.to_le_bytes()); // l/h
                rec(&mut d, &[0x04, 0x6D], &now_type_f());
                rec(&mut d, &[0x44, 0x06], &120_000u32.to_le_bytes()); // Stichtag
                rec(&mut d, &[0x42, 0x6C], &type_g(2026, 9, 30));
                rec(&mut d, &[0x01, 0xFD, 0x17], &[0x00]);
                rec(&mut d, &[0x0C, 0x78], &self.id.to_le_bytes());
                d.push(0x1F); // weitere Daten folgen
            }
            (Model::Heat, _) => {
                rec(&mut d, &[0x84, 0x01, 0x06], &110_000u32.to_le_bytes());
                rec(&mut d, &[0x82, 0x01, 0x6C], &type_g(2025, 9, 30));
                d.extend_from_slice(&[0x0F, 0x01, 0x02, 0x03]);
            }
            (Model::Electricity, _) => {
                rec(
                    &mut d,
                    &[0x0E, 0x03],
                    &bcd_bytes(12_345_678 + seed as u64 * 1000 + tick as u64, 6),
                ); // Wh
                rec(&mut d, &[0x8E, 0x10, 0x03], &bcd_bytes(8_000_000, 6));
                rec(&mut d, &[0x8E, 0x20, 0x03], &bcd_bytes(4_345_678, 6));
                rec(&mut d, &[0x04, 0x2B], &(2_300 + tick % 100).to_le_bytes());
                rec(&mut d, &[0x05, 0x2E], &3.5f32.to_le_bytes()); // kW, real
                rec(&mut d, &[0x04, 0xFD, 0x48], &2301u32.to_le_bytes()); // 0.1 V
                rec(&mut d, &[0x04, 0xFD, 0x59], &5123u32.to_le_bytes()); // mA
                let fw: Vec<u8> = b"1.02".iter().rev().copied().collect();
                rec(&mut d, &[0x0D, 0xFD, 0x0E, fw.len() as u8], &fw);
            }
            (Model::Water | Model::Fixed, _) => {
                rec(&mut d, &[0x0C, 0x13], &bcd_bytes(123_456 + seed as u64 * 7 + tick as u64, 4)); // l
                rec(&mut d, &[0x4C, 0x13], &bcd_bytes(100_000, 4));
                rec(&mut d, &[0x42, 0x6C], &type_g(2025, 12, 31));
                rec(&mut d, &[0x04, 0x6D], &now_type_f());
            }
        }
        Frame::Long {
            control: 0x08,
            address: if self.fault == Fault::WrongAddress {
                self.primary.wrapping_add(1)
            } else {
                self.primary
            },
            ci: 0x72,
            data: d,
        }
    }
}

fn rec(d: &mut Vec<u8>, head: &[u8], value: &[u8]) {
    d.extend_from_slice(head);
    d.extend_from_slice(value);
}

fn bcd_bytes(mut v: u64, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| {
            let lo = (v % 10) as u8;
            v /= 10;
            let hi = (v % 10) as u8;
            v /= 10;
            (hi << 4) | lo
        })
        .collect()
}

fn type_g(year: u32, month: u8, day: u8) -> [u8; 2] {
    let y = (year % 100) as u8;
    [day | ((y & 0x07) << 5), month | ((y >> 3) << 4)]
}

fn now_type_f() -> [u8; 4] {
    let t = Local::now();
    let g = type_g(t.year() as u32, t.month() as u8, t.day() as u8);
    [t.minute() as u8, t.hour() as u8, g[0], g[1]]
}

// ---------------------------------------------------------------- Simulationen

/// Ein simulierter Zähler, wie er in der Oberfläche angelegt wird
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimMeter {
    /// Primäradresse 0..250
    pub primary: u8,
    /// Identnummer, 8 Ziffern
    pub id: String,
    /// Herstellerkennung, 3 Buchstaben
    pub manufacturer: String,
    #[serde(default = "one")]
    pub version: u8,
    /// Schlüssel aus [`MEDIA`]
    pub medium: String,
    /// eigene Baudrate; None = wie der Bus
    #[serde(default)]
    pub baud: Option<u32>,
    /// Schlüssel aus [`FAULTS`]
    #[serde(default = "no_fault")]
    pub fault: String,
}

fn one() -> u8 {
    1
}
fn no_fault() -> String {
    "none".into()
}

/// Medien zum Auswählen: Schlüssel und Code im Telegramm
pub const MEDIA: [(&str, u8); 8] = [
    ("heat", 0x04),
    ("cooling", 0x0A),
    ("water", 0x07),
    ("cold_water", 0x16),
    ("warm_water", 0x06),
    ("gas", 0x03),
    ("electricity", 0x02),
    // alter Wärmezähler mit fester Datenstruktur (CI 73)
    ("heat_old", 0x04),
];

/// Fehler zum Einbauen
pub const FAULTS: [&str; 7] = ["none", "checksum", "truncated", "busy", "wrong_address", "flaky", "too_many"];

fn medium_code(m: &str) -> u8 {
    MEDIA.iter().find(|(k, _)| *k == m).map_or(0x07, |(_, c)| *c)
}

fn model_of(m: &str) -> Model {
    match m {
        "heat" | "cooling" => Model::Heat,
        "electricity" => Model::Electricity,
        "heat_old" => Model::Fixed,
        _ => Model::Water,
    }
}

fn fault_of(f: &str) -> Fault {
    match f {
        "checksum" => Fault::Checksum,
        "truncated" => Fault::Truncated,
        "busy" => Fault::Busy,
        "wrong_address" => Fault::WrongAddress,
        "flaky" => Fault::Flaky,
        "too_many" => Fault::TooMany,
        _ => Fault::None,
    }
}

impl SimMeter {
    pub fn new(primary: u8, id: &str, manufacturer: &str, medium: &str) -> Self {
        SimMeter {
            primary,
            id: id.into(),
            manufacturer: manufacturer.into(),
            version: 1,
            medium: medium.into(),
            baud: None,
            fault: no_fault(),
        }
    }

    fn baud(mut self, b: u32) -> Self {
        self.baud = Some(b);
        self
    }

    fn fault(mut self, f: &str) -> Self {
        self.fault = f.into();
        self
    }

    fn version(mut self, v: u8) -> Self {
        self.version = v;
        self
    }

    pub fn check(&self) -> Result<()> {
        let who = format!("PA {} / ID {}", self.primary, self.id);
        if self.primary > 250 {
            bail!("{who}: {}", tr("Primäradresse 0..250", "primary address 0..250"));
        }
        if self.id.len() != 8 || !self.id.chars().all(|c| c.is_ascii_digit()) {
            bail!("{who}: {}", tr("ID muss 8 Ziffern haben", "ID must have 8 digits"));
        }
        if self.manufacturer.len() != 3 || !self.manufacturer.chars().all(|c| c.is_ascii_uppercase()) {
            bail!("{who}: {}", tr("Hersteller: 3 Grossbuchstaben, z.B. KAM", "manufacturer: 3 capital letters, e.g. KAM"));
        }
        if !MEDIA.iter().any(|(k, _)| *k == self.medium) {
            bail!("{who}: medium {}", self.medium);
        }
        if let Some(b) = self.baud {
            if !crate::mbus::master::BAUD_RATES.contains(&b) {
                bail!("{who}: {} {b}", tr("keine Standard-Baudrate", "not a standard baud rate"));
            }
        }
        if !FAULTS.contains(&self.fault.as_str()) {
            bail!("{who}: fault {}", self.fault);
        }
        Ok(())
    }
}

/// „Praxis mit Fehlern“: die bisherige feste Belegung (siehe Kopf der Datei)
pub fn praxis() -> Vec<SimMeter> {
    let m = SimMeter::new;
    vec![
        m(1, "12345678", "KAM", "heat").version(0x1B),
        m(2, "87654321", "ABB", "electricity").version(0x02),
        m(3, "30000001", "SEN", "water").version(0x10),
        m(0, "12345699", "SEN", "cold_water").version(0x10),
        m(0, "55500011", "ZRI", "warm_water").version(0x01),
        m(4, "40000004", "SEN", "water").version(0x10).fault("checksum"),
        m(5, "50000005", "SEN", "water").version(0x10).fault("truncated"),
        SimMeter { manufacturer: String::new(), version: 0, ..m(7, "70000007", "", "heat_old") },
        m(10, "10000010", "KAM", "heat").version(0x1B),
        m(10, "10000011", "KAM", "heat").version(0x1B),
        m(11, "11000011", "SEN", "water").version(0x10).fault("busy"),
        m(12, "12000012", "SEN", "water").version(0x10).fault("wrong_address"),
        m(14, "77777777", "SEN", "water").version(0x10),
        m(15, "77777777", "ZRI", "water").version(0x01),
        m(16, "16000016", "SEN", "cold_water").version(0x10).fault("flaky"),
        m(17, "88888888", "SEN", "water").version(0x10),
        m(18, "88888888", "SEN", "water").version(0x10),
        m(19, "19000019", "SEN", "water").version(0x10).fault("too_many"),
        m(9, "90000009", "SEN", "water").version(0x01).baud(300),
        m(6, "60000006", "SEN", "water").version(0x10).baud(9600),
    ]
}

/// Eine Simulation: Name und Zähler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Teil des Anschlusses: `SIMULATOR:<key>`
    pub key: String,
    pub name: String,
    /// eingebaute Vorlage (nicht änderbar, aber kopierbar)
    #[serde(default)]
    pub builtin: bool,
    pub meters: Vec<SimMeter>,
}

/// Eingebaute Vorlagen
pub fn builtins() -> Vec<Scenario> {
    let m = SimMeter::new;
    let ok10 = vec![
        m(1, "61000001", "KAM", "heat"),
        m(2, "61000002", "KAM", "heat"),
        m(3, "61000003", "KAM", "heat"),
        m(4, "62000004", "SEN", "cold_water"),
        m(5, "62000005", "SEN", "cold_water"),
        m(6, "62000006", "SEN", "cold_water"),
        m(7, "63000007", "ZRI", "warm_water"),
        m(8, "63000008", "ZRI", "warm_water"),
        m(9, "64000009", "ABB", "electricity"),
        m(10, "65000010", "ELS", "gas"),
    ];
    let mut mfh = Vec::new();
    for k in 1..=10u8 {
        mfh.push(m(k, &format!("710000{k:02}"), "KAM", "heat"));
        mfh.push(m(10 + k, &format!("720000{k:02}"), "SEN", "cold_water"));
        mfh.push(m(20 + k, &format!("730000{k:02}"), "SEN", "warm_water"));
    }
    let bauds = vec![
        m(1, "81000001", "KAM", "heat"),
        m(2, "81000002", "KAM", "heat"),
        m(3, "82000003", "SEN", "water"),
        m(4, "82000004", "SEN", "water"),
        m(5, "83000005", "ABB", "electricity").baud(9600),
        m(6, "83000006", "ABB", "electricity").baud(9600),
        m(7, "84000007", "SEN", "water").baud(300),
        m(8, "84000008", "SEN", "water").baud(300),
    ];
    let b = |key: &str, name: String, meters: Vec<SimMeter>| Scenario { key: key.into(), name, builtin: true, meters };
    vec![
        b("praxis", tr("Praxis mit typischen Fehlern (21 Zähler)", "Real life with typical faults (21 meters)"), praxis()),
        b("ok10", tr("Alles in Ordnung – 10 Zähler", "All OK – 10 meters"), ok10),
        b("mfh30", tr("Mehrfamilienhaus – 10 Wohnungen, 30 Zähler i.O.", "Apartment block – 10 flats, 30 meters OK"), mfh),
        b("bauds", tr("Gemischte Baudraten (2400/9600/300), sonst i.O.", "Mixed baud rates (2400/9600/300), otherwise OK"), bauds),
    ]
}

/// Eigene Simulationen (aus der Datenbank); gelten für jede neue Verbindung
static USER: Mutex<Vec<Scenario>> = Mutex::new(Vec::new());

pub fn set_user(list: Vec<Scenario>) {
    *USER.lock().unwrap() = list;
}

/// Vorlagen und eigene Simulationen
pub fn all() -> Vec<Scenario> {
    let mut v = builtins();
    v.extend(USER.lock().unwrap().iter().cloned());
    v
}

/// Simulator für `SIMULATOR` (leer = Praxis) oder `SIMULATOR:<key>`
pub fn open(key: &str, baud: u32) -> io::Result<Simulator> {
    if key.is_empty() || key == "praxis" {
        return Ok(Simulator::new(baud));
    }
    match all().into_iter().find(|s| s.key == key) {
        Some(s) => Ok(Simulator::from_meters(baud, &s.meters, false)),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} „{key}“ {}", tr("Simulation", "Simulation"), tr("gibt es nicht", "does not exist")),
        )),
    }
}

// ---------------------------------------------------------------- Eigene Simulationen speichern

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sim_scenarios (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    meters TEXT NOT NULL
);
";

impl crate::db::Db {
    pub fn sim_list(&self) -> Result<Vec<Scenario>> {
        let mut st = self.conn.prepare("SELECT key, name, meters FROM sim_scenarios ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            Ok(Scenario {
                key: r.get(0)?,
                name: r.get(1)?,
                builtin: false,
                meters: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Eigene Simulationen in die Liste für neue Verbindungen übernehmen
    pub fn sim_load(&self) -> Result<()> {
        set_user(self.sim_list()?);
        Ok(())
    }

    /// Neue Simulation, leer oder als Kopie; liefert den Schlüssel
    pub fn sim_create(&mut self, name: &str, from: Option<&str>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let meters = match from {
            Some(k) => all()
                .into_iter()
                .find(|s| s.key == k)
                .map(|s| s.meters)
                .ok_or_else(|| anyhow::anyhow!("{} „{k}“ {}", tr("Simulation", "Simulation"), tr("gibt es nicht", "does not exist")))?,
            None => Vec::new(),
        };
        let base = crate::mqtt::slug(name);
        let base = if base.is_empty() { "sim".to_string() } else { base };
        let mut key = base.clone();
        let mut n = 2;
        while all().iter().any(|s| s.key == key) {
            key = format!("{base}-{n}");
            n += 1;
        }
        self.conn.execute(
            "INSERT INTO sim_scenarios (key, name, meters) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, name, serde_json::to_string(&meters)?],
        )?;
        self.sim_load()?;
        Ok(key)
    }

    pub fn sim_save(&mut self, key: &str, name: &str, meters: &[SimMeter]) -> Result<()> {
        if builtins().iter().any(|s| s.key == key) {
            bail!(tr("Vorlagen lassen sich nicht ändern, zuerst kopieren", "Templates cannot be changed, copy them first"));
        }
        if name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        if meters.len() > 500 {
            bail!(tr("Höchstens 500 Zähler je Simulation", "At most 500 meters per simulation"));
        }
        for m in meters {
            m.check()?;
        }
        let n = self.conn.execute(
            "UPDATE sim_scenarios SET name = ?2, meters = ?3 WHERE key = ?1",
            rusqlite::params![key, name.trim(), serde_json::to_string(meters)?],
        )?;
        if n == 0 {
            bail!("{} „{key}“ {}", tr("Simulation", "Simulation"), tr("gibt es nicht", "does not exist"));
        }
        self.sim_load()
    }

    /// Löschen, wenn keine Linie sie verwendet
    pub fn sim_delete(&mut self, key: &str) -> Result<()> {
        let conn = format!("SIMULATOR:{key}");
        let users: Vec<String> = self
            .conn
            .prepare("SELECT gateways.name || ' › ' || lines.name FROM lines JOIN gateways ON gateways.id = lines.gateway_id WHERE lines.connection = ?1")?
            .query_map([&conn], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !users.is_empty() {
            bail!("{}: {}", tr("Wird noch verwendet von", "Still used by"), users.join(", "));
        }
        self.conn.execute("DELETE FROM sim_scenarios WHERE key = ?1", [key])?;
        self.sim_load()
    }
}

pub struct Simulator {
    slaves: Vec<Slave>,
    baud: u32,
    inbuf: Vec<u8>,
    rx: VecDeque<u8>,
    byte_time: Duration,
    start: Instant,
}

impl Simulator {
    /// Simulation „Praxis mit Fehlern“ (Anschluss `SIMULATOR`)
    pub fn new(baud: u32) -> Self {
        Self::from_meters(baud, &praxis(), true)
    }

    /// Simulator aus einer Liste von Zählern. `praxis`: Zähler mit eigener Baudrate laufen auf
    /// 2400 Bd, falls der Bus selbst mit ihrer Baudrate läuft (so bleibt immer einer „anders“),
    /// und alle Werte beginnen gleich (für die Tests).
    pub fn from_meters(baud: u32, meters: &[SimMeter], praxis: bool) -> Self {
        let slaves = meters
            .iter()
            .map(|m| {
                let id = u32::from_str_radix(&m.id, 16).unwrap_or(0);
                let mut s = Slave::new(m.primary, id, &m.manufacturer, m.version, medium_code(&m.medium), model_of(&m.medium))
                    .with_fault(fault_of(&m.fault))
                    .with_baud(match m.baud {
                        Some(b) if praxis && b == baud => 2400,
                        Some(b) => b,
                        None => baud,
                    });
                if !praxis {
                    // Zählerstände je Zähler verschieden, aber immer gleich für dieselbe ID
                    s.seed = (m.id.parse::<u64>().unwrap_or(0) % 997 * 53) as u32;
                }
                s
            })
            .collect();
        Simulator {
            slaves,
            baud,
            inbuf: Vec::new(),
            rx: VecDeque::new(),
            byte_time: Duration::from_secs_f64(11.0 / baud.max(300) as f64),
            start: Instant::now(),
        }
    }

    fn targets(&self, address: u8) -> Vec<usize> {
        (0..self.slaves.len())
            .filter(|&k| self.slaves[k].baud == self.baud)
            .filter(|&k| match address {
                ADDR_BROADCAST | ADDR_BROADCAST_REPLY => true,
                ADDR_SELECTED => self.slaves[k].selected,
                a => self.slaves[k].primary == a,
            })
            .collect()
    }

    fn handle(&mut self, f: Frame) -> Vec<u8> {
        let elapsed = self.start.elapsed().as_secs_f64();
        let mut replies: Vec<Vec<u8>> = Vec::new();
        match f {
            Frame::Short { control, address } if control == frame::C_SND_NKE => {
                for k in self.targets(address) {
                    let s = &mut self.slaves[k];
                    if !s.hears() {
                        continue;
                    }
                    s.reset();
                    if address == ADDR_SELECTED || address == ADDR_BROADCAST {
                        s.selected = false;
                    }
                    replies.push(vec![ACK]);
                }
            }
            Frame::Short { control, address } if control & 0xCF == 0x4B => {
                let fcb = control & frame::FCB != 0;
                for k in self.targets(address) {
                    let slave = &mut self.slaves[k];
                    if !slave.hears() {
                        continue;
                    }
                    let mut bytes = slave.rsp_ud(fcb, elapsed).encode();
                    match slave.fault {
                        Fault::None | Fault::Busy | Fault::WrongAddress | Fault::Flaky | Fault::TooMany => {}
                        Fault::Checksum => {
                            let n = bytes.len();
                            bytes[n - 2] ^= 0x5A;
                        }
                        Fault::Truncated => bytes.truncate(bytes.len() / 2),
                    }
                    replies.push(bytes);
                }
            }
            Frame::Long {
                control,
                address,
                ci,
                data,
            } if control & 0xCF == 0x43 => {
                if ci == CI_SELECT && address == ADDR_SELECTED {
                    let baud = self.baud;
                    for s in self.slaves.iter_mut().filter(|s| s.baud == baud) {
                        s.selected = s.matches(&data) && s.hears();
                        if s.selected {
                            s.reset();
                            replies.push(vec![ACK]);
                        }
                    }
                } else if ci == CI_DATA_SEND
                    && data.len() == 3
                    && data[0] == 0x01
                    && data[1] == 0x7A
                {
                    for k in self.targets(address) {
                        self.slaves[k].primary = data[2];
                        replies.push(vec![ACK]);
                    }
                } else if (0xB8..=0xBF).contains(&ci) && data.is_empty() {
                    let new = BAUD_RATES[(ci - 0xB8) as usize];
                    for k in self.targets(address) {
                        // ACK geht noch mit der alten Baudrate raus
                        self.slaves[k].baud = new;
                        replies.push(vec![ACK]);
                    }
                } else if ci == CI_APP_RESET {
                    for k in self.targets(address) {
                        self.slaves[k].reset();
                        replies.push(vec![ACK]);
                    }
                }
            }
            _ => {}
        }
        match replies.len() {
            0 => Vec::new(),
            1 => replies.pop().unwrap(),
            // Mehrere Sender: ACKs folgen aufeinander, Datentelegramme überlagern sich
            _ if replies.iter().all(|r| r == &[ACK]) => replies.concat(),
            _ => {
                let len = replies.iter().map(Vec::len).max().unwrap();
                (0..len)
                    .map(|k| {
                        replies
                            .iter()
                            .fold(0xFF, |acc, r| acc & r.get(k).copied().unwrap_or(0xFF))
                    })
                    .collect()
            }
        }
    }
}

impl Link for Simulator {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.inbuf.extend_from_slice(data);
        loop {
            match frame::parse(&self.inbuf) {
                Parse::Complete(f, used) => {
                    self.inbuf.drain(..used);
                    let broadcast = matches!(
                        f,
                        Frame::Short {
                            address: ADDR_BROADCAST,
                            ..
                        }
                    ) || matches!(
                        f,
                        Frame::Long {
                            address: ADDR_BROADCAST,
                            ..
                        }
                    );
                    let reply = self.handle(f);
                    if !broadcast {
                        self.rx.extend(reply);
                    }
                }
                Parse::Incomplete => break,
                Parse::Invalid(_) => {
                    self.inbuf.clear();
                    break;
                }
            }
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.rx.is_empty() {
            std::thread::sleep(timeout.min(Duration::from_millis(15)));
            return Ok(0);
        }
        let n = buf.len().min(self.rx.len()).min(32);
        for b in buf.iter_mut().take(n) {
            *b = self.rx.pop_front().unwrap();
        }
        // Übertragungszeit grob nachbilden, damit sich der Simulator wie ein Bus anfühlt
        std::thread::sleep(self.byte_time * n as u32 / 4);
        Ok(n)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.rx.clear();
        Ok(())
    }

    fn set_baud(&mut self, baud: u32) -> io::Result<()> {
        self.baud = baud;
        self.byte_time = Duration::from_secs_f64(11.0 / baud.max(300) as f64);
        self.inbuf.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbus::master::{Bauds, FrameLog, Master, MeterReading, Progress, Target};
    use std::sync::Arc;

    fn master() -> Master {
        Master::new(
            Box::new(Simulator::new(9600)),
            9600,
            50,
            Arc::new(FrameLog::default()),
        )
    }

    #[test]
    fn primary_scan_finds_meters_and_collision() {
        let mut m = master();
        let mut hits = Vec::new();
        m.scan_primary(0, 5, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                hits.push(h);
            }
            true
        })
        .unwrap();
        let addrs: Vec<_> = hits
            .iter()
            .map(|h| (h.primary_address.unwrap(), h.status))
            .collect();
        assert_eq!(
            addrs,
            vec![
                (0, "collision"),
                (1, "ok"),
                (2, "ok"),
                (3, "ok"),
                (4, "no_data"),
                (5, "no_data")
            ]
        );
        assert_eq!(hits[1].id.as_deref(), Some("12345678"));
        assert_eq!(hits[1].manufacturer.as_deref(), Some("KAM"));
    }

    #[test]
    fn secondary_scan_resolves_all() {
        let mut m = master();
        let mut hits = Vec::new();
        m.scan_secondary(&mut |p| {
            if let Progress::Hit(h) = p {
                hits.push((h.id.clone().unwrap_or_default(), h.status));
            }
            true
        })
        .unwrap();
        hits.sort();
        assert_eq!(
            hits,
            vec![
                ("10000010".into(), "ok"),
                ("10000011".into(), "ok"),
                // meldet "beschäftigt": gefunden, ID aus der Suche, aber keine Daten
                ("11000011".into(), "no_data"),
                ("12000012".into(), "ok"),
                ("12345678".into(), "ok"),
                ("12345699".into(), "ok"),
                // Kontingent: die ersten Auslesungen klappen noch
                ("19000019".into(), "ok"),
                ("30000001".into(), "ok"),
                // fehlerhafte Telegramme: gefunden, aber ohne Daten
                ("40000004".into(), "no_data"),
                ("50000005".into(), "no_data"),
                ("55500011".into(), "ok"),
                ("70000007".into(), "ok"),
                // gleiche ID bei zwei Zählern: über die ID allein nicht trennbar
                ("77777777".into(), "collision"),
                ("87654321".into(), "ok"),
                ("88888888".into(), "collision"),
                // 60000006 und 90000009 laufen mit anderer Baudrate, 16000016 (Wackelkontakt)
                // verpasst die Selektion: alle drei fehlen
            ]
        );
    }

    #[test]
    fn read_multi_telegram() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        assert_eq!(r.raw_frames.len(), 2);
        assert!(r.records.iter().any(|x| x.storage == 2 && x.unit == "kWh"));
        assert!(r
            .records
            .iter()
            .any(|x| x.quantity == "flow_temperature" && x.value_text == "65.23"));
    }

    #[test]
    fn raw_telegrams_round_trip() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        let back = MeterReading::from_raw(&r.raw_bytes(), r.timestamp.clone(), r.utc, r.target.clone(), false).unwrap();
        assert_eq!(back.raw_frames, r.raw_frames);
        assert_eq!(back.primary_address, r.primary_address);
        assert_eq!(serde_json::to_value(&back.records).unwrap(), serde_json::to_value(&r.records).unwrap());
        assert_eq!(back.header.unwrap().secondary_address, r.header.unwrap().secondary_address);
    }

    #[test]
    fn read_electricity_values() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(2)).unwrap();
        let find = |d: &str| {
            r.records
                .iter()
                .find(|x| x.quantity == d)
                .unwrap()
                .value_text
                .clone()
        };
        assert_eq!(find("voltage"), "230.1");
        assert_eq!(find("current"), "5.123");
        assert_eq!(find("firmware_version"), "1.02");
        assert!(r.records.iter().any(|x| x.tariff == 2));
    }

    #[test]
    fn corrupt_telegrams_are_rejected() {
        let mut m = master();
        let e = m.read_meter(&Target::Primary(4)).unwrap_err().to_string();
        assert!(e.contains("Kollision"), "{e}");
        assert!(m.read_meter(&Target::Primary(5)).is_err());
    }

    #[test]
    fn set_address_via_secondary() {
        let mut m = master();
        m.set_primary(&Target::parse("12345699").unwrap(), 20)
            .unwrap();
        let r = m.read_meter(&Target::Primary(20)).unwrap();
        assert_eq!(r.header.unwrap().id, "12345699");
        // Adresse 0 ist jetzt eindeutig
        let r = m.read_meter(&Target::Primary(0)).unwrap();
        assert_eq!(r.header.unwrap().id, "55500011");
    }

    #[test]
    fn set_address_refuses_collision() {
        let mut m = master();
        let e = m.set_primary(&Target::Primary(0), 9).unwrap_err().to_string();
        assert!(e.contains("Mehrere Zähler"), "{e}");
        // keiner der beiden wurde umgestellt
        assert!(m.read_meter(&Target::Primary(9)).is_err());
        let r = m.read_meter(&Target::parse("55500011").unwrap()).unwrap();
        assert_eq!(r.primary_address, 0);
    }

    #[test]
    fn set_address_refuses_occupied() {
        let mut m = master();
        let e = m.set_primary(&Target::Primary(1), 2).unwrap_err().to_string();
        assert!(e.contains("schon belegt"), "{e}");
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        assert_eq!(r.header.unwrap().id, "12345678");
        let r = m.read_meter(&Target::Primary(2)).unwrap();
        assert_eq!(r.header.unwrap().id, "87654321");
    }

    #[test]
    fn set_address_to_own_address_is_ok() {
        let mut m = master();
        m.set_primary(&Target::Primary(1), 1).unwrap();
        m.set_primary(&Target::parse("87654321").unwrap(), 2).unwrap();
    }

    #[test]
    fn detect_and_switch_baud() {
        let mut m = master();
        // Adresse 6 läuft mit 2400 Bd, der Master mit 9600
        assert!(m.read_meter(&Target::Primary(6)).is_err());
        assert_eq!(m.detect_baud(&Target::Primary(6)).unwrap(), vec![2400]);
        assert_eq!(m.baud(), 9600);
        m.set_baud(2400).unwrap();
        m.switch_baud(&Target::parse("60000006").unwrap(), 9600)
            .unwrap();
        m.set_baud(9600).unwrap();
        let r = m.read_meter(&Target::Primary(6)).unwrap();
        assert_eq!(r.header.unwrap().id, "60000006");
        // Zähler 1 läuft weiter mit 9600
        assert_eq!(m.detect_baud(&Target::Primary(1)).unwrap(), vec![9600]);
    }

    #[test]
    fn switch_baud_refuses_collision() {
        let mut m = master();
        assert!(m.switch_baud(&Target::Primary(0), 2400).is_err());
        assert_eq!(m.detect_baud(&Target::Primary(0)).unwrap(), vec![9600]);
    }

    #[test]
    fn application_reset_is_acknowledged() {
        let mut m = master();
        m.application_reset(&Target::Primary(1), None).unwrap();
        m.application_reset(&Target::parse("87654321").unwrap(), Some(0x10))
            .unwrap();
        assert!(m.application_reset(&Target::Primary(99), None).is_err());
    }

    #[test]
    fn read_fixed_structure() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(7)).unwrap();
        let h = r.header.unwrap();
        assert_eq!((h.id.as_str(), h.medium.as_str()), ("70000007", "medium_heat_outlet"));
        assert_eq!(r.records.len(), 2);
        assert_eq!(
            (r.records[0].quantity.as_str(), r.records[0].unit.as_str()),
            ("energy", "kWh")
        );
        assert!(r.records[0].value_text.starts_with("123"));
        assert_eq!(
            (r.records[1].unit.as_str(), r.records[1].value_text.as_str()),
            ("m³", "678")
        );
    }

    #[test]
    fn raw_send_returns_reply() {
        let mut m = master();
        let ack = m.raw_send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
        assert!(matches!(ack, crate::mbus::master::Reply::Frame(crate::mbus::frame::Frame::Ack)));
        let none = m.raw_send(&[0x10, 0x40, 0x63, 0xA3, 0x16]).unwrap();
        assert!(matches!(none, crate::mbus::master::Reply::None));
    }

    #[test]
    fn bus_bauds_finds_mixed_bus() {
        let mut m = master();
        // 300: uralter Zähler auf Adresse 9
        assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 2400, 9600]);
        // einzeln abfragen wie die Oberfläche
        assert_eq!(m.bus_bauds(&[2400]).unwrap(), vec![2400]);
        assert_eq!(m.bus_bauds(&[4800]).unwrap(), Vec::<u32>::new());
        assert!(m.bus_bauds(&[1234]).is_err());
        assert_eq!(m.baud(), 9600);
    }

    #[test]
    fn primary_scan_on_all_bauds() {
        let mut m = master();
        let mut hits = Vec::new();
        m.scan_with(
            Bauds::Probe,
            &mut |p| {
                if let Progress::Hit(h) = p {
                    hits.push((h.primary_address.unwrap(), h.baud.unwrap(), h.status));
                }
                true
            },
            &mut |m, p| m.scan_primary(5, 7, 0, p),
        )
        .unwrap();
        assert_eq!(
            hits,
            // eingestellte Baudrate (9600) zuerst
            vec![(5, 9600, "no_data"), (7, 9600, "ok"), (6, 2400, "ok")]
        );
        assert_eq!(m.baud(), 9600);
        // nur eingestellte Baudrate: Zähler 6 bleibt unsichtbar, Treffer tragen trotzdem die Baudrate
        let mut bauds = Vec::new();
        m.scan_with(
            Bauds::Current,
            &mut |p| {
                if let Progress::Hit(h) = p {
                    bauds.push((h.primary_address.unwrap(), h.baud.unwrap()));
                }
                true
            },
            &mut |m, p| m.scan_primary(5, 7, 0, p),
        )
        .unwrap();
        assert_eq!(bauds, vec![(5, 9600), (7, 9600)]);
        // gewählte Liste ohne Prüfung, eingestellte Baudrate zuerst
        let mut order = Vec::new();
        m.scan_with(
            Bauds::List(vec![2400, 9600, 2400]),
            &mut |p| {
                if let Progress::Hit(h) = p {
                    order.push((h.primary_address.unwrap(), h.baud.unwrap()));
                }
                true
            },
            &mut |m, p| m.scan_primary(6, 6, 0, p),
        )
        .unwrap();
        assert_eq!(order, vec![(6, 2400)]);
        assert!(m
            .scan_with(Bauds::List(vec![1234]), &mut |_| true, &mut |m, p| m
                .scan_primary(6, 6, 0, p))
            .is_err());
    }

    #[test]
    fn free_primary_skips_meters_without_data() {
        let mut m = master();
        // 1..5 und 7 belegt (4 und 5 liefern keine Daten, antworten aber), 6 läuft mit anderer Baudrate
        assert_eq!(m.free_primary(1, &[]).unwrap(), 6);
        assert_eq!(m.free_primary(1, &[6]).unwrap(), 8);
    }

    #[test]
    fn switch_meter_on_other_baud_to_bus_baud() {
        let mut m = master();
        // Zähler 6 läuft mit 2400, Bus mit 9600: Befehl mit 2400 senden, Kontrolle mit 9600
        m.set_baud(2400).unwrap();
        m.switch_baud(&Target::Primary(6), 9600).unwrap();
        m.set_baud(9600).unwrap();
        assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 9600]);
    }

    #[test]
    fn faulty_meters_are_reported() {
        let mut m = master();
        let e = m.read_meter(&Target::Primary(11)).unwrap_err().to_string();
        assert!(e.contains("beschäftigt") && !e.contains("Decodier"), "{e}");
        let mut st = Vec::new();
        m.scan_primary(11, 11, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                st.push(h.status);
            }
            true
        })
        .unwrap();
        assert_eq!(st, vec!["no_data"]);
        let e = m.read_meter(&Target::Primary(12)).unwrap_err().to_string();
        assert!(e.contains("13"), "{e}");
        // zwei Zähler auf Adresse 10: Kollision. Nicht über read_meter prüfen: überlagerte
        // Telegramme ergeben je nach Zählerstand zufällig ein gültiges (wie am echten Bus)
        let mut st = Vec::new();
        m.scan_primary(10, 10, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                st.push(h.status);
            }
            true
        })
        .unwrap();
        assert_eq!(st, vec!["collision"]);
        let e = m.read_meter(&Target::Primary(10)).unwrap_err().to_string();
        assert!(e.contains("Mehrere Zähler"), "{e}");
        // über die ID geht es
        let r = m.read_meter(&Target::parse("10000011").unwrap()).unwrap();
        assert_eq!(r.header.unwrap().id, "10000011");
        // gleiche ID, verschiedene Hersteller: nur ID ist mehrdeutig, volle Adresse eindeutig
        let e = m.read_meter(&Target::parse("77777777").unwrap()).unwrap_err().to_string();
        assert!(e.contains("16-stellig"), "{e}");
        let r = m.read_meter(&Target::parse("777777776A490107").unwrap()).unwrap();
        assert_eq!(r.header.unwrap().manufacturer, "ZRI");
        // Wackelkontakt: ohne Wiederholung übersehen, mit Wiederholung gefunden
        let mut found = |retries| {
            let mut n = 0;
            m.scan_primary(16, 16, retries, &mut |p| {
                n += matches!(p, Progress::Hit(_)) as u32;
                true
            })
            .unwrap();
            n
        };
        assert_eq!(found(0), 0);
        assert_eq!(found(1), 1);
    }

    #[test]
    fn scan_explains_why_there_is_no_data() {
        let mut m = master();
        let mut why = Vec::new();
        m.scan_primary(0, 12, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                if h.status != "ok" {
                    why.push((h.primary_address.unwrap(), h.reason.unwrap_or_default(), h.raw.is_some()));
                }
            }
            true
        })
        .unwrap();
        assert_eq!(
            why,
            vec![
                (0, "collision".into(), true),
                (4, "checksum".into(), true),
                (5, "truncated".into(), true),
                (10, "collision".into(), true),
                (11, "app_error:08".into(), true),
                (12, "wrong_address:13".into(), true),
            ]
        );
    }

    fn master_for(key: &str, baud: u32) -> Master {
        Master::new(Box::new(super::open(key, baud).unwrap()), baud, 50, Arc::new(FrameLog::default()))
    }

    #[test]
    fn template_all_ok_has_ten_clean_meters() {
        let mut m = master_for("ok10", 2400);
        let mut hits = Vec::new();
        m.scan_primary(0, 15, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                hits.push((h.primary_address, h.status));
            }
            true
        })
        .unwrap();
        assert_eq!(hits.len(), 10);
        assert!(hits.iter().all(|(_, s)| *s == "ok"), "{hits:?}");
        // Werte je Zähler verschieden
        let a = m.read_meter(&Target::Primary(1)).unwrap();
        let b = m.read_meter(&Target::Primary(2)).unwrap();
        assert_ne!(a.records[0].value, b.records[0].value);
    }

    #[test]
    fn template_apartment_block_secondary_scan_finds_all() {
        let mut m = master_for("mfh30", 2400);
        let mut n = 0;
        m.scan_secondary(&mut |p| {
            if let Progress::Hit(h) = p {
                assert_eq!(h.status, "ok");
                n += 1;
            }
            true
        })
        .unwrap();
        assert_eq!(n, 30);
    }

    #[test]
    fn template_bauds_answers_on_three_rates() {
        let mut m = master_for("bauds", 2400);
        assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 2400, 9600]);
    }

    #[test]
    fn unknown_simulation_and_bad_meters_are_refused() {
        assert!(super::open("gibtsnicht", 2400).is_err());
        let ok = SimMeter::new(1, "12345678", "KAM", "heat");
        assert!(ok.check().is_ok());
        for bad in [
            SimMeter { id: "1234".into(), ..ok.clone() },
            SimMeter { manufacturer: "ka".into(), ..ok.clone() },
            SimMeter { primary: 251, ..ok.clone() },
            SimMeter { medium: "lava".into(), ..ok.clone() },
            SimMeter { baud: Some(1234), ..ok.clone() },
            SimMeter { fault: "kaputt".into(), ..ok.clone() },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn own_simulation_is_stored_and_used() {
        let dir = std::env::temp_dir().join(format!("mbusio-sim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut db = crate::db::Db::open(&dir).unwrap();
            let key = db.sim_create("Test Anlage", Some("ok10")).unwrap();
            assert_eq!(key, "test-anlage");
            let mut meters = super::all().into_iter().find(|s| s.key == key).unwrap().meters;
            meters.truncate(3);
            meters[2].fault = "busy".into();
            db.sim_save(&key, "Test Anlage", &meters).unwrap();
            // Vorlagen bleiben unverändert
            assert!(db.sim_save("ok10", "x", &meters).is_err());
        }
        // nach einem Neustart wieder da
        crate::sim::set_user(Vec::new());
        let db = crate::db::Db::open(&dir).unwrap();
        assert_eq!(db.sim_list().unwrap()[0].meters.len(), 3);
        let mut m = master_for("test-anlage", 2400);
        assert!(m.read_meter(&Target::Primary(1)).is_ok());
        assert!(format!("{:#}", m.read_meter(&Target::Primary(3)).unwrap_err()).contains("08"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
