//! Typen des Masters: Ziel, Antwort, Auslesung, Scan-Treffer, Fortschritt

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Primary(u8),
    /// 16 Hex-Zeichen, `F` = Platzhalter
    Secondary(String),
}

impl Target {
    /// "5" → Primär, "12345678" → Sekundär (Rest Platzhalter), 16 Zeichen → Sekundär.
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim().to_ascii_uppercase();
        match s.len() {
            8 | 16 if s.chars().all(|c| c.is_ascii_hexdigit()) => {
                Ok(Target::Secondary(format!("{s:F<16}")))
            }
            _ => {
                let a: u8 = s
                    .parse()
                    .map_err(|_| anyhow!("{}: {s}", tr("Ungültige Adresse", "Invalid address")))?;
                if a > 250 {
                    bail!(tr(
                        "Primäradresse muss 0..250 sein",
                        "Primary address must be 0..250"
                    ));
                }
                Ok(Target::Primary(a))
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Target::Primary(a) => format!("{a}"),
            Target::Secondary(s) => s.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Reply {
    None,
    Frame(Frame),
    /// Unlesbare Daten, meist mehrere Slaves gleichzeitig
    Collision(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MeterReading {
    pub timestamp: String,
    pub target: String,
    pub primary_address: u8,
    pub header: Option<Header>,
    pub records: Vec<Record>,
    pub raw_frames: Vec<String>,
    /// Uhrzeit beim Auslesen nicht bestätigt (siehe `clock`): Zeitstempel kann falsch sein
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub time_unsure: bool,
    /// Zeitpunkt in UTC (Sekunden seit 1970); eindeutig auch bei Sommerzeit-Ende
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc: Option<i64>,
}

impl MeterReading {
    /// Antworttelegramme decodieren; Zeitstempel = jetzt
    pub(super) fn decode(frames: Vec<RawTelegram>, primary: u8) -> Result<MeterReading> {
        let mut header = None;
        let mut records = Vec::new();
        let mut raw_frames = Vec::new();
        for (ci, data, bytes) in frames {
            raw_frames.push(hex(&bytes));
            let t = decode_rsp_ud(ci, &data).map_err(|e| {
                // Fehlertelegramm: der Zähler meldet selbst einen Fehler, kein Decodierproblem
                if ci == 0x70 {
                    anyhow::Error::new(MeterRefused {
                        code: data.first().copied().unwrap_or(0),
                        text: e,
                    })
                } else {
                    anyhow!("{}: {e}", tr("Decodierfehler", "Decoding error"))
                }
            })?;
            if header.is_none() {
                header = t.header;
            }
            records.extend(t.records);
        }
        Ok(MeterReading {
            timestamp: crate::clock::stamp(),
            utc: Some(crate::clock::utc_secs()),
            time_unsure: !crate::clock::confirmed(),
            target: String::new(),
            primary_address: primary,
            header,
            records,
            raw_frames,
        })
    }

    /// Rohtelegramme hintereinander, so wie sie kompakt gespeichert werden
    pub fn raw_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for f in &self.raw_frames {
            let clean: Vec<u8> = f.bytes().filter(|b| b.is_ascii_hexdigit()).collect();
            for pair in clean.chunks(2) {
                if let Ok(b) = u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("0"), 16) {
                    out.push(b);
                }
            }
        }
        out
    }

    /// Gegenstück zu `raw_bytes`: Auslesung aus gespeicherten Rohtelegrammen wiederherstellen.
    /// Ein späterer, besserer Decoder gilt so auch für alte Daten.
    pub fn from_raw(
        raw: &[u8],
        timestamp: String,
        utc: Option<i64>,
        target: String,
        time_unsure: bool,
    ) -> Result<MeterReading> {
        let mut frames = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            match frame::parse(&raw[i..]) {
                frame::Parse::Complete(
                    Frame::Long {
                        address, ci, data, ..
                    },
                    n,
                ) => {
                    frames.push((ci, data, raw[i..i + n].to_vec(), address));
                    i += n;
                }
                _ => bail!(
                    "{}",
                    tr(
                        "Gespeichertes Telegramm unlesbar",
                        "Stored telegram unreadable"
                    )
                ),
            }
        }
        let primary = frames.first().map_or(0, |f| f.3);
        let mut r = MeterReading::decode(
            frames.into_iter().map(|(c, d, b, _)| (c, d, b)).collect(),
            primary,
        )?;
        r.timestamp = timestamp;
        r.utc = utc;
        r.target = target;
        r.time_unsure = time_unsure;
        Ok(r)
    }
}

/// Der Zähler meldet selbst einen Fehler (Fehlertelegramm CI 70), z.B. 08 beschäftigt,
/// 09 zu viele Auslesungen
#[derive(Debug)]
pub struct MeterRefused {
    pub code: u8,
    pub text: String,
}

impl std::fmt::Display for MeterRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::error::Error for MeterRefused {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanHit {
    pub primary_address: Option<u8>,
    pub secondary_address: Option<String>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub medium: Option<String>,
    pub version: Option<u8>,
    /// "ok", "collision", "no_data"
    pub status: String,
    /// Baudrate, auf der der Zähler gefunden wurde
    pub baud: Option<u32>,
    /// Ursache bei "collision"/"no_data" als Schlüssel, z.B. `checksum`, `app_error:08`
    pub reason: Option<String>,
    /// zuletzt empfangene Bytes dazu, für die Telegramm-Analyse
    pub raw: Option<String>,
}

impl ScanHit {
    pub(super) fn from_header(
        primary: Option<u8>,
        h: Option<&Header>,
        status: &'static str,
    ) -> Self {
        ScanHit {
            primary_address: primary,
            secondary_address: h.map(|h| h.secondary_address.clone()),
            id: h.map(|h| h.id.clone()),
            manufacturer: h.map(|h| h.manufacturer.clone()),
            medium: h.map(|h| h.medium.clone()),
            version: h.map(|h| h.version),
            status: status.into(),
            baud: None,
            reason: None,
            raw: None,
        }
    }
}

pub enum Progress {
    Step {
        done: u32,
        total: u32,
        message: String,
    },
    Hit(ScanHit),
    /// Abgebrochen: hier ginge die Suche weiter (Sekundärscan: Maske, Baudrate setzt scan_bauds)
    Stopped {
        mask: String,
        baud: Option<u32>,
    },
}

/// Rückgabe `false` = Abbruch.
pub type ProgressFn<'a> = &'a mut dyn FnMut(Progress) -> bool;
