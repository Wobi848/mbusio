//! M-Bus Master: Scan, Auslesen, Adresse setzen.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use super::decode::{decode_rsp_ud, Header, Record};
use super::frame::{
    self, hex, Frame, Parse, ADDR_BROADCAST, ADDR_SELECTED, CI_DATA_SEND, CI_SELECT,
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
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
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

#[derive(Debug, Clone, Serialize)]
pub struct MeterReading {
    pub timestamp: String,
    pub target: String,
    pub primary_address: u8,
    pub header: Option<Header>,
    pub records: Vec<Record>,
    pub raw_frames: Vec<String>,
}

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
    /// Wartezeit auf das erste Antwortbyte
    pub timeout: Duration,
    /// Pause zwischen Bytes, nach der ein Telegramm als beendet gilt
    inter_byte: Duration,
}

impl Master {
    /// `timeout_ms = 0` → automatisch aus der Baudrate (330 Bitzeiten + 50 ms + Reserve für USB-Wandler).
    pub fn new(link: Box<dyn Link>, baud: u32, timeout_ms: u64, log: Arc<FrameLog>) -> Self {
        let bit = 1.0 / baud.max(1) as f64;
        let timeout = if timeout_ms == 0 {
            Duration::from_secs_f64(330.0 * bit + 0.25)
        } else {
            Duration::from_millis(timeout_ms)
        };
        let inter_byte = Duration::from_secs_f64((40.0 * 11.0 * bit).max(0.06));
        Self {
            link,
            log,
            timeout,
            inter_byte,
        }
    }

    fn send(&mut self, f: &Frame) -> Result<()> {
        self.link.clear()?;
        let bytes = f.encode();
        self.log.push("TX", hex(&bytes));
        self.link.send(&bytes)?;
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
                    while self.link.recv(&mut tmp, self.inter_byte)? > 0 {}
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
                    bail!("{}: {f:?}", tr("Unerwartete Antwort", "Unexpected reply"))
                }
                Reply::None => bail!(
                    "{} {address}",
                    tr("Keine Antwort von Adresse", "No reply from address")
                ),
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
            // Kommunikation zurücksetzen; manche Zähler quittieren nicht, das ist ok
            self.ping(*a)?;
        }
        let address = self.address(target)?;
        let (primary, frames) = self.request_data(address, true)?;
        let mut header = None;
        let mut records = Vec::new();
        let mut raw_frames = Vec::new();
        for (ci, data, bytes) in frames {
            raw_frames.push(hex(&bytes));
            let t = decode_rsp_ud(ci, &data)
                .map_err(|e| anyhow!("{}: {e}", tr("Decodierfehler", "Decoding error")))?;
            if header.is_none() {
                header = t.header;
            }
            records.extend(t.records);
        }
        Ok(MeterReading {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            target: target.label(),
            primary_address: primary,
            header,
            records,
            raw_frames,
        })
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
            match self.ping(*a)? {
                Reply::Frame(_) => {}
                Reply::None => bail!(
                    "{} {a}",
                    tr("Keine Antwort von Adresse", "No reply from address")
                ),
                Reply::Collision(_) => bail!(
                    "{} {a}: {}",
                    tr(
                        "Mehrere Zähler auf Adresse",
                        "Several meters on address"
                    ),
                    tr(
                        "bitte über Sekundäradresse oder ID ändern",
                        "please use the secondary address or ID"
                    )
                ),
            }
        }
        let address = self.address(target)?;
        match self.transact(&Frame::snd_ud(address, CI_DATA_SEND, vec![0x01, 0x7A, new]))? {
            Reply::Frame(Frame::Ack) => {}
            Reply::None => bail!(tr(
                "Zähler hat die Adressänderung nicht quittiert",
                "Meter did not acknowledge the address change"
            )),
            other => bail!(
                "{}: {other:?}",
                tr("Unerwartete Antwort", "Unexpected reply")
            ),
        }
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

    fn probe_header(&mut self, address: u8) -> Result<(Option<u8>, Option<Header>, bool)> {
        match self.request_data(address, false) {
            Ok((primary, frames)) => {
                let header = frames
                    .first()
                    .and_then(|(ci, data, _)| decode_rsp_ud(*ci, data).ok()?.header);
                Ok((Some(primary), header, true))
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
                Reply::Collision(_) => ScanHit::from_header(Some(a), None, "collision"),
                Reply::Frame(_) => match self.probe_header(a)? {
                    (_, Some(h), _) => ScanHit::from_header(Some(a), Some(&h), "ok"),
                    (_, None, true) => ScanHit::from_header(Some(a), None, "ok"),
                    (_, None, false) => ScanHit::from_header(Some(a), None, "no_data"),
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
                    match self.request_data(ADDR_SELECTED, false) {
                        Ok((primary, frames)) => {
                            let header = frames
                                .first()
                                .and_then(|(ci, d, _)| decode_rsp_ud(*ci, d).ok()?.header);
                            let mut hit =
                                ScanHit::from_header(Some(primary), header.as_ref(), "ok");
                            if hit.secondary_address.is_none() {
                                hit.secondary_address = Some(m.clone());
                            }
                            progress(Progress::Hit(hit));
                            false
                        }
                        // ID vollständig, aber Daten unbrauchbar: Zähler defekt statt Kollision
                        Err(_) if pos == 7 => {
                            progress(Progress::Hit(ScanHit {
                                secondary_address: Some(m.clone()),
                                id: Some(m[..8].to_string()),
                                ..ScanHit::from_header(None, None, "no_data")
                            }));
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
                    progress(Progress::Hit(ScanHit {
                        secondary_address: Some(m.clone()),
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

/// Passt die Sekundäradresse auf die Maske (`F` = Platzhalter)?
fn mask_matches(mask: &str, secondary: &str) -> bool {
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
