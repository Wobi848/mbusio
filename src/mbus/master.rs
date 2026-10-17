//! M-Bus Master: Scan, Auslesen, Adresse setzen.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use super::decode::{decode_rsp_ud, Header, Record};
use super::frame::{
    self, hex, Frame, Parse, ADDR_BROADCAST, ADDR_BROADCAST_REPLY, ADDR_SELECTED, CI_APP_RESET, CI_DATA_SEND,
    CI_SELECT,
};
use crate::i18n::tr;
use crate::link::Link;

const MAX_TELEGRAMS: usize = 16;

/// Empfangenes Datentelegramm: CI-Feld, Nutzdaten, komplettes Telegramm
type RawTelegram = (u8, Vec<u8>, Vec<u8>);

// ---------------------------------------------------------------- Frame-Log

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub seq: u64,
    pub time: String,
    pub dir: &'static str,
    pub text: String,
}

#[derive(Default)]
pub struct FrameLog {
    inner: Mutex<(u64, VecDeque<LogEntry>)>,
}

impl FrameLog {
    pub fn push(&self, dir: &'static str, text: String) {
        let mut g = self.inner.lock().unwrap();
        g.0 += 1;
        let seq = g.0;
        g.1.push_back(LogEntry {
            seq,
            time: crate::clock::now().format("%H:%M:%S%.3f").to_string(),
            dir,
            text,
        });
        while g.1.len() > 1000 {
            g.1.pop_front();
        }
    }

    pub fn since(&self, seq: u64) -> Vec<LogEntry> {
        self.inner
            .lock()
            .unwrap()
            .1
            .iter()
            .filter(|e| e.seq > seq)
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().1.clear();
    }
}

// ---------------------------------------------------------------- Typen

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
    fn decode(frames: Vec<RawTelegram>, primary: u8) -> Result<MeterReading> {
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
    pub fn from_raw(raw: &[u8], timestamp: String, utc: Option<i64>, target: String, time_unsure: bool) -> Result<MeterReading> {
        let mut frames = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            match frame::parse(&raw[i..]) {
                frame::Parse::Complete(Frame::Long { address, ci, data, .. }, n) => {
                    frames.push((ci, data, raw[i..i + n].to_vec(), address));
                    i += n;
                }
                _ => bail!("{}", tr("Gespeichertes Telegramm unlesbar", "Stored telegram unreadable")),
            }
        }
        let primary = frames.first().map_or(0, |f| f.3);
        let mut r = MeterReading::decode(frames.into_iter().map(|(c, d, b, _)| (c, d, b)).collect(), primary)?;
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

#[derive(Debug, Clone, Serialize)]
pub struct ScanHit {
    pub primary_address: Option<u8>,
    pub secondary_address: Option<String>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub medium: Option<String>,
    pub version: Option<u8>,
    /// "ok", "collision", "no_data"
    pub status: &'static str,
    /// Baudrate, auf der der Zähler gefunden wurde
    pub baud: Option<u32>,
    /// Ursache bei "collision"/"no_data" als Schlüssel, z.B. `checksum`, `app_error:08`
    pub reason: Option<String>,
    /// zuletzt empfangene Bytes dazu, für die Telegramm-Analyse
    pub raw: Option<String>,
}

impl ScanHit {
    fn from_header(primary: Option<u8>, h: Option<&Header>, status: &'static str) -> Self {
        ScanHit {
            primary_address: primary,
            secondary_address: h.map(|h| h.secondary_address.clone()),
            id: h.map(|h| h.id.clone()),
            manufacturer: h.map(|h| h.manufacturer.clone()),
            medium: h.map(|h| h.medium.clone()),
            version: h.map(|h| h.version),
            status,
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
}

/// Rückgabe `false` = Abbruch.
pub type ProgressFn<'a> = &'a mut dyn FnMut(Progress) -> bool;

// ---------------------------------------------------------------- Master

pub struct Master {
    link: Box<dyn Link>,
    log: Arc<FrameLog>,
    baud: u32,
    /// Eingestellter Timeout, 0 = automatisch aus der Baudrate
    timeout_ms: u64,
    /// Wartezeit auf das erste Antwortbyte
    pub timeout: Duration,
    /// Pause zwischen Bytes, nach der ein Telegramm als beendet gilt
    inter_byte: Duration,
    /// Letzte erkannte Störung: Schlüssel und empfangene Bytes (für "Warum?")
    diag: Option<(String, Option<String>)>,
}

/// Auf welchen Baudraten gescannt wird
pub enum Bauds {
    /// nur die eingestellte Baudrate
    Current,
    /// erst prüfen, wo Geräte antworten, dann dort scannen
    Probe,
    List(Vec<u32>),
}

/// Standard-Baudraten, langsamste zuerst
pub const BAUD_RATES: [u32; 8] = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];

/// `timeout_ms = 0` → automatisch aus der Baudrate (330 Bitzeiten + 50 ms + Reserve für USB-Wandler).
fn timings(baud: u32, timeout_ms: u64) -> (Duration, Duration) {
    let bit = 1.0 / baud.max(1) as f64;
    let timeout = if timeout_ms == 0 {
        Duration::from_secs_f64(330.0 * bit + 0.25)
    } else {
        Duration::from_millis(timeout_ms)
    };
    let inter_byte = Duration::from_secs_f64((40.0 * 11.0 * bit).max(0.06));
    (timeout, inter_byte)
}

impl Master {
    pub fn new(link: Box<dyn Link>, baud: u32, timeout_ms: u64, log: Arc<FrameLog>) -> Self {
        let (timeout, inter_byte) = timings(baud, timeout_ms);
        Self {
            link,
            log,
            baud,
            timeout_ms,
            timeout,
            inter_byte,
            diag: None,
        }
    }

    fn note(&mut self, key: impl Into<String>, raw: Option<&[u8]>) {
        self.diag = Some((key.into(), raw.map(hex)));
    }

    /// Ursache der letzten Störung an einen Treffer hängen
    fn explain(&mut self, mut hit: ScanHit) -> ScanHit {
        if let Some((reason, raw)) = self.diag.take() {
            hit.reason = Some(reason);
            hit.raw = raw;
        }
        hit
    }

    pub fn baud(&self) -> u32 {
        self.baud
    }

    /// Baudrate des Masters wechseln; Timeouts werden mit angepasst.
    pub fn set_baud(&mut self, baud: u32) -> Result<()> {
        if baud == self.baud {
            return Ok(());
        }
        self.link.set_baud(baud)?;
        self.baud = baud;
        (self.timeout, self.inter_byte) = timings(baud, self.timeout_ms);
        self.log.push(
            "INFO",
            format!("{} {baud} Bd", tr("Baudrate", "Baud rate")),
        );
        Ok(())
    }

    fn send(&mut self, f: &Frame) -> Result<()> {
        self.send_bytes(&f.encode())
    }

    fn send_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.link.clear()?;
        self.log.push("TX", hex(bytes));
        self.link.send(bytes)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<Reply> {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 256];
        let start = Instant::now();
        loop {
            let wait = if buf.is_empty() {
                match self.timeout.checked_sub(start.elapsed()) {
                    Some(w) if !w.is_zero() => w,
                    _ => break,
                }
            } else {
                self.inter_byte
            };
            let n = self.link.recv(&mut tmp, wait)?;
            if n == 0 {
                if buf.is_empty() {
                    continue;
                }
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            match frame::parse(&buf) {
                Parse::Incomplete => continue,
                Parse::Complete(f, used) => {
                    // Nachlaufende Bytes deuten auf einen zweiten Sender hin
                    let extra = self.link.recv(&mut tmp, self.inter_byte)?;
                    buf.extend_from_slice(&tmp[..extra]);
                    self.log.push("RX", hex(&buf));
                    if used < buf.len() {
                        self.note("collision", Some(&buf));
                        self.log.push(
                            "INFO",
                            tr(
                                "Zusätzliche Bytes empfangen: Kollision",
                                "Extra bytes received: collision",
                            ),
                        );
                        return Ok(Reply::Collision(buf));
                    }
                    return Ok(Reply::Frame(f));
                }
                Parse::Invalid(why) => {
                    // Rest abwarten, damit der nächste Request sauber startet
                    while let n @ 1.. = self.link.recv(&mut tmp, self.inter_byte)? {
                        buf.extend_from_slice(&tmp[..n]);
                    }
                    let key = match why {
                        "Prüfsumme falsch" => "checksum",
                        "Stopbyte fehlt" => "stop",
                        _ => "garbled",
                    };
                    self.note(key, Some(&buf));
                    self.log.push("RX", hex(&buf));
                    self.log.push(
                        "INFO",
                        format!(
                            "{} ({why})",
                            tr(
                                "Ungültiges Telegramm: Kollision oder Störung",
                                "Invalid telegram: collision or noise"
                            )
                        ),
                    );
                    return Ok(Reply::Collision(buf));
                }
            }
        }
        if buf.is_empty() {
            self.log.push(
                "INFO",
                format!(
                    "{} ({} ms)",
                    tr("Keine Antwort", "No reply"),
                    self.timeout.as_millis()
                ),
            );
            Ok(Reply::None)
        } else {
            self.log.push("RX", hex(&buf));
            self.note("truncated", Some(&buf));
            self.log.push(
                "INFO",
                tr("Unvollständiges Telegramm", "Incomplete telegram"),
            );
            Ok(Reply::Collision(buf))
        }
    }

    fn transact(&mut self, f: &Frame) -> Result<Reply> {
        self.send(f)?;
        self.receive()
    }

    pub fn ping(&mut self, address: u8) -> Result<Reply> {
        self.transact(&Frame::snd_nke(address))
    }

    /// Sekundäradresse selektieren (Maske mit `F` als Platzhalter).
    pub fn select(&mut self, mask: &str) -> Result<Reply> {
        let data = select_data(mask)?;
        self.transact(&Frame::snd_ud(ADDR_SELECTED, CI_SELECT, data))
    }

    /// Adressiert das Ziel und liefert die zu verwendende Link-Adresse.
    fn address(&mut self, target: &Target) -> Result<u8> {
        match target {
            Target::Primary(a) => Ok(*a),
            Target::Secondary(mask) => match self.select(mask)? {
                Reply::Frame(Frame::Ack) => Ok(ADDR_SELECTED),
                Reply::None => bail!(
                    "{} {mask}",
                    tr(
                        "Kein Zähler mit Sekundäradresse",
                        "No meter with secondary address"
                    )
                ),
                // nur die ID angegeben: IDs sind nur je Hersteller eindeutig
                Reply::Collision(_) if mask.ends_with("FFFFFFFF") => bail!(
                    "{} {}: {}",
                    tr("Mehrere Zähler mit der ID", "Several meters with ID"),
                    &mask[..8],
                    tr(
                        "volle Sekundäradresse (16-stellig) verwenden, z.B. „Auslesen“ in der Trefferliste",
                        "use the full secondary address (16 digits), e.g. “Read” in the hit list"
                    )
                ),
                Reply::Collision(_) => bail!(
                    "{} {mask}",
                    tr("Mehrere Zähler passen auf", "Several meters match")
                ),
                Reply::Frame(f) => bail!(
                    "{}: {f:?}",
                    tr(
                        "Unerwartete Antwort auf Selektion",
                        "Unexpected reply to selection"
                    )
                ),
            },
        }
    }

    /// REQ_UD2, bei "weitere Daten folgen" mit wechselndem FCB nachfragen.
    fn request_data(&mut self, address: u8, all: bool) -> Result<(u8, Vec<RawTelegram>)> {
        let mut frames = Vec::new();
        let mut fcb = true;
        let mut primary = address;
        for _ in 0..MAX_TELEGRAMS {
            let mut reply = self.transact(&Frame::req_ud2(address, fcb))?;
            if matches!(reply, Reply::None) {
                // einmal wiederholen, gleicher FCB
                reply = self.transact(&Frame::req_ud2(address, fcb))?;
            }
            let f = match reply {
                Reply::Frame(f) if f.is_rsp_ud() => f,
                Reply::Frame(f) => {
                    self.note("unexpected", Some(&f.encode()));
                    bail!("{}: {f:?}", tr("Unerwartete Antwort", "Unexpected reply"))
                }
                Reply::None => {
                    // quittiert SND_NKE, schickt aber keine Daten
                    self.note("no_reply", None);
                    bail!(
                        "{} {address}",
                        tr("Keine Antwort von Adresse", "No reply from address")
                    )
                }
                Reply::Collision(b) => bail!(
                    "{}: {}",
                    tr(
                        "Gestörte Antwort (Kollision?)",
                        "Corrupted reply (collision?)"
                    ),
                    hex(&b)
                ),
            };
            let bytes = f.encode();
            let Frame::Long {
                address: a,
                ci,
                data,
                ..
            } = f
            else {
                unreachable!()
            };
            if address <= 250 && a != address {
                self.note(format!("wrong_address:{a}"), Some(&bytes));
                bail!(
                    "{} {a} ≠ {address}",
                    tr("Antwort von falscher Adresse", "Reply from wrong address")
                );
            }
            primary = a;
            let more = data_has_more(ci, &data);
            frames.push((ci, data, bytes));
            if !all || !more {
                break;
            }
            fcb = !fcb;
        }
        Ok((primary, frames))
    }

    pub fn read_meter(&mut self, target: &Target) -> Result<MeterReading> {
        if let Target::Primary(a) = target {
            // Kommunikation zurücksetzen; manche Zähler quittieren nicht, das ist ok.
            // Antworten mehrere, wären die Daten ein Gemisch, das zufällig gültig aussehen kann.
            if let Reply::Collision(_) = self.ping(*a)? {
                bail!(
                    "{} {a}: {}",
                    tr("Mehrere Zähler auf Adresse", "Several meters on address"),
                    tr(
                        "über die ID auslesen, oder Sekundärscan und „PA ändern“",
                        "read by ID, or run a secondary scan and “Change PA”"
                    )
                );
            }
        }
        let address = self.address(target)?;
        let (primary, frames) = self.request_data(address, true)?;
        let mut r = MeterReading::decode(frames, primary)?;
        r.target = target.label();
        Ok(r)
    }

    pub fn set_primary(&mut self, target: &Target, new: u8) -> Result<()> {
        if new > 250 {
            bail!(tr(
                "Neue Primäradresse muss 0..250 sein",
                "New primary address must be 0..250"
            ));
        }
        // Neue Adresse muss frei sein, sonst stehen danach zwei Zähler darauf
        if !matches!(self.ping(new)?, Reply::None) {
            if let Target::Secondary(mask) = target {
                // Steht der Zähler selbst schon dort, ist nichts zu tun
                if let (_, Some(h), _) = self.probe_header(new)? {
                    if mask_matches(mask, &h.secondary_address) {
                        return Ok(());
                    }
                }
            } else if *target == Target::Primary(new) {
                return Ok(());
            }
            bail!(
                "{}: {new}",
                tr(
                    "Neue Primäradresse ist schon belegt",
                    "New primary address is already in use"
                )
            );
        }
        // Mehrere Zähler auf derselben Primäradresse würden alle umgestellt
        if let Target::Primary(a) = target {
            self.ensure_single(*a)?;
        }
        let address = self.address(target)?;
        self.expect_ack(
            &Frame::snd_ud(address, CI_DATA_SEND, vec![0x01, 0x7A, new]),
            &tr(
                "Zähler hat die Adressänderung nicht quittiert",
                "Meter did not acknowledge the address change",
            ),
        )?;
        match self.ping(new)? {
            Reply::Frame(Frame::Ack) => Ok(()),
            _ => bail!(
                "{} {new}",
                tr(
                    "Adresse gesetzt, aber Zähler antwortet nicht auf neuer Adresse",
                    "Address set, but meter does not answer on new address"
                )
            ),
        }
    }

    /// Bricht ab, wenn auf der Primäradresse keiner oder mehrere Zähler antworten.
    fn ensure_single(&mut self, a: u8) -> Result<()> {
        match self.ping(a)? {
            Reply::Frame(_) => Ok(()),
            Reply::None => bail!(
                "{} {a}",
                tr("Keine Antwort von Adresse", "No reply from address")
            ),
            Reply::Collision(_) => bail!(
                "{} {a}: {}",
                tr("Mehrere Zähler auf Adresse", "Several meters on address"),
                tr(
                    "bitte über Sekundäradresse oder ID ändern",
                    "please use the secondary address or ID"
                )
            ),
        }
    }

    fn expect_ack(&mut self, f: &Frame, no_reply: &str) -> Result<()> {
        match self.transact(f)? {
            Reply::Frame(Frame::Ack) => Ok(()),
            Reply::None => bail!("{no_reply}"),
            other => bail!(
                "{}: {other:?}",
                tr("Unerwartete Antwort", "Unexpected reply")
            ),
        }
    }

    /// Antwortet das Ziel (Primär: SND_NKE, Sekundär: Selektion)?
    fn answers(&mut self, target: &Target) -> Result<bool> {
        Ok(match target {
            Target::Primary(a) => !matches!(self.ping(*a)?, Reply::None),
            Target::Secondary(mask) => matches!(self.select(mask)?, Reply::Frame(Frame::Ack)),
        })
    }

    /// Baudrate am Zähler umstellen (CI B8..BF) und mit der neuen Baudrate kontrollieren.
    /// Der Master bleibt danach auf seiner bisherigen Baudrate.
    pub fn switch_baud(&mut self, target: &Target, new: u32) -> Result<()> {
        let ci = baud_ci(new)?;
        let old = self.baud;
        // TCP-Gateways können nicht umschalten; vorher prüfen statt hinterher
        self.link.set_baud(old)?;
        if new == old {
            return Ok(());
        }
        if let Target::Primary(a) = target {
            self.ensure_single(*a)?;
        }
        let address = self.address(target)?;
        self.expect_ack(
            &Frame::snd_ud(address, ci, Vec::new()),
            &tr(
                "Zähler hat den Baudratenwechsel nicht quittiert",
                "Meter did not acknowledge the baud rate change",
            ),
        )?;
        // Zähler wechselt erst nach dem ACK
        std::thread::sleep(self.inter_byte);
        self.set_baud(new)?;
        let ok = self.answers(target);
        self.set_baud(old)?;
        if !ok? {
            bail!(
                "{} {new} Bd",
                tr(
                    "Zähler hat quittiert, antwortet aber nicht mit",
                    "Meter acknowledged, but does not answer at"
                )
            );
        }
        Ok(())
    }

    /// Probiert alle Standard-Baudraten durch und liefert die, auf denen das Ziel antwortet.
    pub fn detect_baud(&mut self, target: &Target) -> Result<Vec<u32>> {
        let old = self.baud;
        self.link.set_baud(old)?;
        let result = self.try_bauds(target);
        self.set_baud(old)?;
        result
    }

    fn try_bauds(&mut self, target: &Target) -> Result<Vec<u32>> {
        let mut found = Vec::new();
        for baud in BAUD_RATES {
            self.set_baud(baud)?;
            // einmal wiederholen, manche Zähler verpassen das erste Telegramm nach Pause
            if self.answers(target)? || self.answers(target)? {
                found.push(baud);
            }
        }
        Ok(found)
    }

    /// Auf welchen Baudraten antworten überhaupt Geräte? Je Baudrate ein Broadcast-Ping
    /// (alle Zähler quittieren) und eine Sekundär-Selektion mit lauter Platzhaltern.
    /// `bauds` leer = alle Standard-Baudraten. Die Oberfläche fragt einzeln ab, um den
    /// Fortschritt zu zeigen.
    pub fn bus_bauds(&mut self, bauds: &[u32]) -> Result<Vec<u32>> {
        let bauds = if bauds.is_empty() { &BAUD_RATES[..] } else { bauds };
        if let Some(b) = bauds.iter().find(|b| !BAUD_RATES.contains(b)) {
            bail!(
                "{}: {b}",
                tr("Keine Standard-Baudrate", "Not a standard baud rate")
            );
        }
        let old = self.baud;
        self.link.set_baud(old)?;
        let result = self.probe_bus_bauds(bauds);
        self.set_baud(old)?;
        result
    }

    fn probe_bus_bauds(&mut self, bauds: &[u32]) -> Result<Vec<u32>> {
        let mut found = Vec::new();
        for &baud in bauds {
            self.set_baud(baud)?;
            let any = !matches!(self.ping(ADDR_BROADCAST_REPLY)?, Reply::None)
                || !matches!(self.select("FFFFFFFFFFFFFFFF")?, Reply::None);
            // Selektion wieder aufheben
            self.send(&Frame::snd_nke(ADDR_BROADCAST))?;
            std::thread::sleep(self.inter_byte);
            if any {
                found.push(baud);
            }
        }
        Ok(found)
    }

    /// Führt `scan` nacheinander auf mehreren Baudraten aus. Treffer tragen ihre Baudrate,
    /// der Fortschritt läuft über alle Baudraten; danach gilt wieder die alte Baudrate.
    pub fn scan_bauds(
        &mut self,
        bauds: &[u32],
        progress: ProgressFn,
        scan: &mut dyn FnMut(&mut Master, ProgressFn) -> Result<()>,
    ) -> Result<()> {
        let home = self.baud;
        let n = bauds.len() as u32;
        let mut result = Ok(());
        for (k, &baud) in bauds.iter().enumerate() {
            if let Err(e) = self.set_baud(baud) {
                result = Err(e);
                break;
            }
            let mut cancelled = false;
            let mut wrapped = |p: Progress| {
                let p = match p {
                    Progress::Step {
                        done,
                        total,
                        message,
                    } => Progress::Step {
                        done: k as u32 * total + done,
                        total: total * n,
                        message: if n > 1 {
                            format!("{baud} Bd · {message}")
                        } else {
                            message
                        },
                    },
                    Progress::Hit(mut h) => {
                        h.baud = Some(baud);
                        Progress::Hit(h)
                    }
                };
                let go = progress(p);
                cancelled |= !go;
                go
            };
            result = scan(self, &mut wrapped);
            if result.is_err() || cancelled {
                break;
            }
        }
        let restore = self.set_baud(home);
        result.and(restore)
    }

    /// Scan auf den gewählten Baudraten (für Busse mit gemischten Baudraten).
    pub fn scan_with(
        &mut self,
        bauds: Bauds,
        progress: ProgressFn,
        scan: &mut dyn FnMut(&mut Master, ProgressFn) -> Result<()>,
    ) -> Result<()> {
        let mut list = match bauds {
            Bauds::Current => vec![self.baud],
            Bauds::List(list) => {
                if let Some(b) = list.iter().find(|b| !BAUD_RATES.contains(b)) {
                    bail!(
                        "{}: {b}",
                        tr("Keine Standard-Baudrate", "Not a standard baud rate")
                    );
                }
                list
            }
            Bauds::Probe => {
                if !progress(Progress::Step {
                    done: 0,
                    total: 1,
                    message: tr(
                        "Prüfe, auf welchen Baudraten Geräte antworten",
                        "Checking which baud rates have devices",
                    ),
                }) {
                    return Ok(());
                }
                self.bus_bauds(&[])?
            }
        };
        if list.is_empty() {
            progress(Progress::Step {
                done: 1,
                total: 1,
                message: tr(
                    "Auf keiner Baudrate antwortet ein Gerät",
                    "No device answers at any baud rate",
                ),
            });
            return Ok(());
        }
        // eingestellte Baudrate zuerst, dort sind meist die meisten Zähler
        let home = self.baud;
        list.sort_by_key(|&b| (b != home, b));
        list.dedup();
        self.scan_bauds(&list, progress, scan)
    }

    /// Erste Primäradresse ab `from`, auf der niemand antwortet (zweimal gefragt).
    /// `skip`: schon bekannte belegte Adressen, die gar nicht erst abgefragt werden.
    pub fn free_primary(&mut self, from: u8, skip: &[u8]) -> Result<u8> {
        for a in from.max(1)..=250 {
            if skip.contains(&a) {
                continue;
            }
            if matches!(self.ping(a)?, Reply::None) && matches!(self.ping(a)?, Reply::None) {
                return Ok(a);
            }
        }
        bail!(tr(
            "Keine freie Primäradresse gefunden",
            "No free primary address found"
        ))
    }

    /// Application Reset (CI 50), optional mit Subcode.
    pub fn application_reset(&mut self, target: &Target, subcode: Option<u8>) -> Result<()> {
        if let Target::Primary(a) = target {
            self.ensure_single(*a)?;
        }
        let address = self.address(target)?;
        self.expect_ack(
            &Frame::snd_ud(address, CI_APP_RESET, subcode.into_iter().collect()),
            &tr(
                "Zähler hat den Reset nicht quittiert",
                "Meter did not acknowledge the reset",
            ),
        )
    }

    /// Beliebige Bytes senden und die Antwort zurückgeben (wie `mbus-tcp-raw-send`).
    pub fn raw_send(&mut self, bytes: &[u8]) -> Result<Reply> {
        self.send_bytes(bytes)?;
        self.receive()
    }

    /// Daten empfangen, aber nicht decodierbar: Fehlertelegramm des Zählers oder unbekanntes Format
    fn note_decode(&mut self, frames: &[RawTelegram]) {
        if let Some((ci, data, bytes)) = frames.first() {
            let key = match (ci, data.first()) {
                (0x70, Some(code)) => format!("app_error:{code:02X}"),
                (0x70, None) => "app_error:00".into(),
                _ => "decode".into(),
            };
            self.note(key, Some(bytes));
        }
    }

    fn probe_header(&mut self, address: u8) -> Result<(Option<u8>, Option<Header>, bool)> {
        self.diag = None;
        match self.request_data(address, false) {
            Ok((primary, frames)) => {
                // brauchbar nur, wenn sich die Antwort decodieren lässt (nicht z.B. "beschäftigt")
                let decoded = frames.first().map(|(ci, data, _)| decode_rsp_ud(*ci, data));
                let usable = matches!(decoded, Some(Ok(_)));
                if !usable {
                    self.note_decode(&frames);
                }
                let header = decoded.and_then(|d| d.ok()?.header);
                Ok((Some(primary), header, usable))
            }
            Err(_) => Ok((None, None, false)),
        }
    }

    pub fn scan_primary(
        &mut self,
        from: u8,
        to: u8,
        retries: u32,
        progress: ProgressFn,
    ) -> Result<()> {
        let total = (to as u32).saturating_sub(from as u32) + 1;
        for (k, a) in (from..=to).enumerate() {
            if !progress(Progress::Step {
                done: k as u32,
                total,
                message: format!("{} {a}", tr("Adresse", "Address")),
            }) {
                return Ok(());
            }
            let mut reply = Reply::None;
            for _ in 0..=retries {
                reply = self.ping(a)?;
                if !matches!(reply, Reply::None) {
                    break;
                }
            }
            let hit = match reply {
                Reply::None => continue,
                Reply::Collision(_) => {
                    let h = ScanHit::from_header(Some(a), None, "collision");
                    self.explain(h)
                }
                Reply::Frame(_) => match self.probe_header(a)? {
                    (_, Some(h), _) => ScanHit::from_header(Some(a), Some(&h), "ok"),
                    (_, None, true) => ScanHit::from_header(Some(a), None, "ok"),
                    (_, None, false) => {
                        let h = ScanHit::from_header(Some(a), None, "no_data");
                        self.explain(h)
                    }
                },
            };
            progress(Progress::Hit(hit));
        }
        progress(Progress::Step {
            done: total,
            total,
            message: tr("Fertig", "Done"),
        });
        Ok(())
    }

    /// Wildcard-Suche über die ID-Stellen (wie libmbus).
    pub fn scan_secondary(&mut self, progress: ProgressFn) -> Result<()> {
        // Eventuell noch selektierte Zähler freigeben
        self.send(&Frame::snd_nke(ADDR_BROADCAST))?;
        std::thread::sleep(self.inter_byte);
        let mut mask: Vec<char> = "FFFFFFFFFFFFFFFF".chars().collect();
        let mut cancelled = false;
        self.scan_secondary_pos(0, &mut mask, progress, &mut cancelled)?;
        if !cancelled {
            progress(Progress::Step {
                done: 10,
                total: 10,
                message: tr("Fertig", "Done"),
            });
        }
        Ok(())
    }

    fn scan_secondary_pos(
        &mut self,
        pos: usize,
        mask: &mut Vec<char>,
        progress: ProgressFn,
        cancelled: &mut bool,
    ) -> Result<()> {
        for digit in 0..=9u8 {
            mask[pos] = char::from(b'0' + digit);
            let m: String = mask.iter().collect();
            let done = if pos == 0 {
                digit as u32
            } else {
                u32::from(mask[0] as u8 - b'0')
            };
            if !progress(Progress::Step {
                done,
                total: 10,
                message: format!("{} {m}", tr("Maske", "Mask")),
            }) {
                *cancelled = true;
                return Ok(());
            }
            let collision = match self.select(&m)? {
                Reply::None => false,
                Reply::Collision(_) => true,
                Reply::Frame(_) => {
                    // ACKs mehrerer Slaves überlagern sich oft fehlerfrei: Daten anfordern prüft das
                    self.diag = None;
                    match self.request_data(ADDR_SELECTED, false) {
                        // ohne lesbaren Datenkopf weitersuchen, bis die ID vollständig ist
                        Ok((_, frames))
                            if pos < 7
                                && frames
                                    .first()
                                    .and_then(|(ci, d, _)| decode_rsp_ud(*ci, d).ok()?.header)
                                    .is_none() =>
                        {
                            true
                        }
                        Ok((primary, frames)) => {
                            let decoded = frames.first().map(|(ci, d, _)| decode_rsp_ud(*ci, d));
                            let status = if matches!(decoded, Some(Ok(_))) {
                                "ok"
                            } else {
                                self.note_decode(&frames);
                                "no_data"
                            };
                            let header = decoded.and_then(|d| d.ok()?.header);
                            let mut hit =
                                ScanHit::from_header(Some(primary), header.as_ref(), status);
                            if hit.secondary_address.is_none() {
                                hit.secondary_address = Some(m.clone());
                            }
                            // ohne lesbaren Datenkopf: ID ist aus der Suche bekannt, sofern vollständig
                            if hit.id.is_none() && !m[..8].contains('F') {
                                hit.id = Some(m[..8].to_string());
                            }
                            let hit = if status == "ok" { hit } else { self.explain(hit) };
                            progress(Progress::Hit(hit));
                            false
                        }
                        // ID vollständig, aber Daten unbrauchbar: Zähler defekt statt Kollision
                        Err(_) if pos == 7 => {
                            let hit = self.explain(ScanHit {
                                secondary_address: Some(m.clone()),
                                id: Some(m[..8].to_string()),
                                ..ScanHit::from_header(None, None, "no_data")
                            });
                            progress(Progress::Hit(hit));
                            false
                        }
                        Err(_) => true,
                    }
                }
            };
            if collision {
                if pos < 7 {
                    self.scan_secondary_pos(pos + 1, mask, progress, cancelled)?;
                    if *cancelled {
                        return Ok(());
                    }
                } else {
                    // gleiche ID bei mehreren Zählern: nur über Hersteller/Version/Medium trennbar
                    progress(Progress::Hit(ScanHit {
                        secondary_address: Some(m.clone()),
                        id: Some(m[..8].to_string()),
                        reason: Some("same_id".into()),
                        ..ScanHit::from_header(None, None, "collision")
                    }));
                }
            }
        }
        mask[pos] = 'F';
        Ok(())
    }
}

fn data_has_more(ci: u8, data: &[u8]) -> bool {
    decode_rsp_ud(ci, data)
        .map(|t| t.more_follow)
        .unwrap_or(false)
}

/// CI-Feld für den Baudratenwechsel (EN 13757-3): B8 = 300 … BF = 38400
fn baud_ci(baud: u32) -> Result<u8> {
    match BAUD_RATES.iter().position(|&b| b == baud) {
        Some(k) => Ok(0xB8 + k as u8),
        None => bail!(
            "{}: {baud}",
            tr("Keine Standard-Baudrate", "Not a standard baud rate")
        ),
    }
}

/// Passt die Sekundäradresse auf die Maske (`F` = Platzhalter)?
pub fn mask_matches(mask: &str, secondary: &str) -> bool {
    mask.len() == secondary.len()
        && mask
            .chars()
            .zip(secondary.chars())
            .all(|(m, s)| m.eq_ignore_ascii_case(&'F') || m.eq_ignore_ascii_case(&s))
}

/// Selektionsdaten: ID (BCD, LSB zuerst), Hersteller, Version, Medium; `F` = Platzhalter.
pub fn select_data(mask: &str) -> Result<Vec<u8>> {
    let m = mask.trim().to_ascii_uppercase();
    if m.len() != 16 || !m.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "{}: {mask}",
            tr(
                "Sekundäradresse muss 16 Hex-Zeichen haben",
                "Secondary address must have 16 hex characters"
            )
        );
    }
    let b: Vec<u8> = (0..8)
        .map(|k| u8::from_str_radix(&m[2 * k..2 * k + 2], 16).unwrap())
        .collect();
    Ok(vec![b[3], b[2], b[1], b[0], b[5], b[4], b[6], b[7]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_parsing() {
        assert_eq!(Target::parse("5").unwrap(), Target::Primary(5));
        assert_eq!(
            Target::parse("12345678").unwrap(),
            Target::Secondary("12345678FFFFFFFF".into())
        );
        assert!(Target::parse("251").is_err());
    }

    #[test]
    fn select_encoding() {
        assert_eq!(
            select_data("123456782C2D1B04").unwrap(),
            vec![0x78, 0x56, 0x34, 0x12, 0x2D, 0x2C, 0x1B, 0x04]
        );
    }
}
