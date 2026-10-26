//! M-Bus Master: Scan, Auslesen, Adresse setzen.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use super::decode::{decode_rsp_ud, Header, Record};
use super::frame::{
    self, hex, Frame, Parse, ADDR_BROADCAST, ADDR_BROADCAST_REPLY, ADDR_SELECTED, CI_APP_RESET,
    CI_DATA_SEND, CI_SELECT,
};
use crate::i18n::{both, tr, Text};
use crate::link::Link;

mod config;
mod log;
mod scan;
mod types;

pub use log::FrameLog;
pub use types::*;

const MAX_TELEGRAMS: usize = 16;

/// Empfangenes Datentelegramm: CI-Feld, Nutzdaten, komplettes Telegramm
type RawTelegram = (u8, Vec<u8>, Vec<u8>);

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

/// Fehler, wenn eine der Baudraten keine Standard-Baudrate ist
pub fn check_baud(bauds: impl IntoIterator<Item = u32>) -> Result<()> {
    for b in bauds {
        if !BAUD_RATES.contains(&b) {
            bail!(
                "{}: {b}",
                tr("Keine Standard-Baudrate", "Not a standard baud rate")
            );
        }
    }
    Ok(())
}

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

    /// `f` mit dieser Baudrate ausführen (None = wie jetzt), danach zurückschalten
    pub fn at_baud<T>(
        &mut self,
        baud: Option<u32>,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let home = self.baud;
        self.set_baud(baud.unwrap_or(home))?;
        let r = f(self);
        let restore = self.set_baud(home);
        let v = r?;
        restore?;
        Ok(v)
    }

    /// Baudrate des Masters wechseln; Timeouts werden mit angepasst.
    pub fn set_baud(&mut self, baud: u32) -> Result<()> {
        if baud == self.baud {
            return Ok(());
        }
        self.link.set_baud(baud)?;
        self.baud = baud;
        (self.timeout, self.inter_byte) = timings(baud, self.timeout_ms);
        self.log
            .push("INFO", format!("{} {baud} Bd", tr("Baudrate", "Baud rate")));
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
                            both(|| {
                                tr(
                                    "Zusätzliche Bytes empfangen: Kollision",
                                    "Extra bytes received: collision",
                                )
                            }),
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
                        "checksum" | "stop" => why,
                        _ => "garbled",
                    };
                    self.note(key, Some(&buf));
                    self.log.push("RX", hex(&buf));
                    self.log.push(
                        "INFO",
                        both(|| {
                            format!(
                                "{} ({})",
                                tr(
                                    "Ungültiges Telegramm: Kollision oder Störung",
                                    "Invalid telegram: collision or noise"
                                ),
                                frame::invalid_text(why)
                            )
                        }),
                    );
                    return Ok(Reply::Collision(buf));
                }
            }
        }
        if buf.is_empty() {
            self.log.push(
                "INFO",
                both(|| {
                    format!(
                        "{} ({} ms)",
                        tr("Keine Antwort", "No reply"),
                        self.timeout.as_millis()
                    )
                }),
            );
            Ok(Reply::None)
        } else {
            self.log.push("RX", hex(&buf));
            self.note("truncated", Some(&buf));
            self.log.push(
                "INFO",
                both(|| tr("Unvollständiges Telegramm", "Incomplete telegram")),
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
}

fn data_has_more(ci: u8, data: &[u8]) -> bool {
    decode_rsp_ud(ci, data)
        .map(|t| t.more_follow)
        .unwrap_or(false)
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
