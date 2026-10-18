//! Simulationen: Zähler als Daten, Vorlagen, Liste der aktiven Simulationen

use super::*;

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
pub const FAULTS: [&str; 7] = [
    "none",
    "checksum",
    "truncated",
    "busy",
    "wrong_address",
    "flaky",
    "too_many",
];

pub(super) fn medium_code(m: &str) -> u8 {
    MEDIA
        .iter()
        .find(|(k, _)| *k == m)
        .map_or(0x07, |(_, c)| *c)
}

pub(super) fn model_of(m: &str) -> Model {
    match m {
        "heat" | "cooling" => Model::Heat,
        "electricity" => Model::Electricity,
        "heat_old" => Model::Fixed,
        _ => Model::Water,
    }
}

pub(super) fn fault_of(f: &str) -> Fault {
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
            bail!(
                "{who}: {}",
                tr("Primäradresse 0..250", "primary address 0..250")
            );
        }
        if self.id.len() != 8 || !self.id.chars().all(|c| c.is_ascii_digit()) {
            bail!(
                "{who}: {}",
                tr("ID muss 8 Ziffern haben", "ID must have 8 digits")
            );
        }
        if self.manufacturer.len() != 3
            || !self.manufacturer.chars().all(|c| c.is_ascii_uppercase())
        {
            bail!(
                "{who}: {}",
                tr(
                    "Hersteller: 3 Grossbuchstaben, z.B. KAM",
                    "manufacturer: 3 capital letters, e.g. KAM"
                )
            );
        }
        if !MEDIA.iter().any(|(k, _)| *k == self.medium) {
            bail!("{who}: medium {}", self.medium);
        }
        if let Some(b) = self.baud {
            if !crate::mbus::master::BAUD_RATES.contains(&b) {
                bail!(
                    "{who}: {} {b}",
                    tr("keine Standard-Baudrate", "not a standard baud rate")
                );
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
        m(4, "40000004", "SEN", "water")
            .version(0x10)
            .fault("checksum"),
        m(5, "50000005", "SEN", "water")
            .version(0x10)
            .fault("truncated"),
        SimMeter {
            manufacturer: String::new(),
            version: 0,
            ..m(7, "70000007", "", "heat_old")
        },
        m(10, "10000010", "KAM", "heat").version(0x1B),
        m(10, "10000011", "KAM", "heat").version(0x1B),
        m(11, "11000011", "SEN", "water")
            .version(0x10)
            .fault("busy"),
        m(12, "12000012", "SEN", "water")
            .version(0x10)
            .fault("wrong_address"),
        m(14, "77777777", "SEN", "water").version(0x10),
        m(15, "77777777", "ZRI", "water").version(0x01),
        m(16, "16000016", "SEN", "cold_water")
            .version(0x10)
            .fault("flaky"),
        m(17, "88888888", "SEN", "water").version(0x10),
        m(18, "88888888", "SEN", "water").version(0x10),
        m(19, "19000019", "SEN", "water")
            .version(0x10)
            .fault("too_many"),
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
    let b = |key: &str, name: String, meters: Vec<SimMeter>| Scenario {
        key: key.into(),
        name,
        builtin: true,
        meters,
    };
    vec![
        b(
            "praxis",
            tr(
                "Praxis mit typischen Fehlern (21 Zähler)",
                "Real life with typical faults (21 meters)",
            ),
            praxis(),
        ),
        b(
            "ok10",
            tr("Alles in Ordnung – 10 Zähler", "All OK – 10 meters"),
            ok10,
        ),
        b(
            "mfh30",
            tr(
                "Mehrfamilienhaus – 10 Wohnungen, 30 Zähler i.O.",
                "Apartment block – 10 flats, 30 meters OK",
            ),
            mfh,
        ),
        b(
            "bauds",
            tr(
                "Gemischte Baudraten (2400/9600/300), sonst i.O.",
                "Mixed baud rates (2400/9600/300), otherwise OK",
            ),
            bauds,
        ),
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
            format!(
                "{} „{key}“ {}",
                tr("Simulation", "Simulation"),
                tr("gibt es nicht", "does not exist")
            ),
        )),
    }
}
