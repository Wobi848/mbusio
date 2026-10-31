//! USB-Wandler am Android-Handy: Android lässt Programme nicht an `/dev/ttyUSB*`, nur die App
//! darf über die USB-Host-API auf den Wandler. Die App (android/) startet mbusio und reicht den
//! Wandler über eine lokale TCP-Verbindung durch; Adresse und Schlüssel stehen in
//! `MBUSIO_USB_BRIDGE` (`127.0.0.1:Port`) und `MBUSIO_USB_BRIDGE_TOKEN`.
//!
//! Rahmen in beide Richtungen: Art (1 Byte), Länge (u32, big endian), Inhalt.
//! mbusio → App: `H` Schlüssel (immer zuerst), `L` Wandler auflisten, `O` öffnen
//! („Name\nBaudrate“), `W` senden, `B` Baudrate, `C` Empfangspuffer verwerfen.
//! App → mbusio: `D` empfangene Bytes, `R` erledigt (bei `L`: JSON-Liste), `E` Fehlertext.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::{Link, PortInfo};
use crate::i18n::tr;
use anyhow::{anyhow, Context, Result};

pub const USB_PREFIX: &str = "usb://";
const ADDR_VAR: &str = "MBUSIO_USB_BRIDGE";
const TOKEN_VAR: &str = "MBUSIO_USB_BRIDGE_TOKEN";
/// So lange darf die App für eine Antwort brauchen (Öffnen fragt evtl. nach der USB-Erlaubnis)
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

pub struct BridgeLink {
    stream: TcpStream,
    /// angekommene, noch nicht vollständige Rahmen
    raw: Vec<u8>,
    /// empfangene Bytes vom Bus, noch nicht abgeholt
    data: VecDeque<u8>,
}

/// Wandler, die die App gerade sieht; leer ohne App
pub fn list() -> Vec<PortInfo> {
    let Some(addr) = std::env::var_os(ADDR_VAR) else {
        return Vec::new();
    };
    let listed = BridgeLink::connect(&addr.to_string_lossy())
        .and_then(|mut b| Ok(b.request(b'L', &[])?))
        .and_then(|json| Ok(serde_json::from_slice::<Vec<Listed>>(&json)?));
    match listed {
        Ok(l) => l
            .into_iter()
            .map(|l| PortInfo {
                port: format!("{USB_PREFIX}{}", l.name),
                label: l.label,
                dev: None,
            })
            .collect(),
        Err(e) => {
            eprintln!("USB: {e:#}");
            Vec::new()
        }
    }
}

#[derive(serde::Deserialize)]
struct Listed {
    name: String,
    label: String,
}

/// `name` ohne `usb://`
pub fn open(name: &str, baud: u32) -> Result<BridgeLink> {
    let addr = std::env::var(ADDR_VAR).map_err(|_| {
        anyhow!(tr(
            "USB-Wandler mit usb:// gibt es nur in der Android-App",
            "USB converters with usb:// only exist in the Android app",
        ))
    })?;
    let mut link = BridgeLink::connect(&addr)?;
    link.request(b'O', format!("{name}\n{baud}").as_bytes())
        .with_context(|| {
            format!(
                "{} {USB_PREFIX}{name}",
                tr("Port konnte nicht geöffnet werden:", "Could not open port")
            )
        })?;
    Ok(link)
}

impl BridgeLink {
    fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).with_context(|| {
            format!(
                "{} {addr}",
                tr("Keine Verbindung zu", "Could not connect to")
            )
        })?;
        stream.set_nodelay(true)?;
        let mut link = Self {
            stream,
            raw: Vec::new(),
            data: VecDeque::new(),
        };
        let token = std::env::var(TOKEN_VAR).unwrap_or_default();
        link.write_frame(b'H', token.as_bytes())?;
        Ok(link)
    }

    fn write_frame(&mut self, kind: u8, payload: &[u8]) -> io::Result<()> {
        let mut frame = Vec::with_capacity(5 + payload.len());
        frame.push(kind);
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload);
        self.stream.write_all(&frame)?;
        self.stream.flush()
    }

    /// Nächster ganzer Rahmen, `None` wenn bis `deadline` keiner kommt
    fn next_frame(&mut self, deadline: Instant) -> io::Result<Option<(u8, Vec<u8>)>> {
        loop {
            if self.raw.len() >= 5 {
                let len = u32::from_be_bytes(self.raw[1..5].try_into().unwrap()) as usize;
                if self.raw.len() >= 5 + len {
                    let kind = self.raw[0];
                    let payload = self.raw[5..5 + len].to_vec();
                    self.raw.drain(..5 + len);
                    return Ok(Some((kind, payload)));
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            self.stream.set_read_timeout(Some(left))?;
            let mut buf = [0u8; 512];
            match self.stream.read(&mut buf) {
                Ok(0) => return Err(gone()),
                Ok(n) => self.raw.extend_from_slice(&buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None)
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Auftrag an die App, wartet auf `R` (Inhalt) oder `E` (Fehler). Bytes vom Bus, die
    /// unterdessen ankommen, bleiben für `recv`.
    fn request(&mut self, kind: u8, payload: &[u8]) -> io::Result<Vec<u8>> {
        self.write_frame(kind, payload)?;
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            match self.next_frame(deadline)? {
                Some((b'D', d)) => self.data.extend(d),
                Some((b'R', r)) => return Ok(r),
                Some((b'E', e)) => {
                    return Err(io::Error::other(String::from_utf8_lossy(&e).into_owned()))
                }
                Some(_) => {}
                None => return Err(io::Error::new(io::ErrorKind::TimedOut, gone())),
            }
        }
    }
}

/// App beendet oder Wandler abgezogen
fn gone() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        tr(
            "USB-Wandler nicht erreichbar, abgezogen?",
            "USB converter not reachable, unplugged?",
        ),
    )
}

impl Link for BridgeLink {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.request(b'W', data).map(|_| ())
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        let deadline = Instant::now() + timeout;
        while self.data.is_empty() {
            match self.next_frame(deadline)? {
                Some((b'D', d)) => self.data.extend(d),
                Some((b'E', e)) => {
                    return Err(io::Error::other(String::from_utf8_lossy(&e).into_owned()))
                }
                Some(_) => {}
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.data.len());
        for (b, d) in buf.iter_mut().zip(self.data.drain(..n)) {
            *b = d;
        }
        Ok(n)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.request(b'C', &[])?;
        // was vor der Antwort kam, hat die App schon gelesen: ebenfalls verwerfen
        self.data.clear();
        Ok(())
    }

    fn set_baud(&mut self, baud: u32) -> io::Result<()> {
        self.request(b'B', baud.to_string().as_bytes()).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    type Frame = (u8, Vec<u8>);

    fn read_frame(s: &mut TcpStream) -> Frame {
        let mut head = [0u8; 5];
        s.read_exact(&mut head).unwrap();
        let mut payload = vec![0u8; u32::from_be_bytes(head[1..5].try_into().unwrap()) as usize];
        s.read_exact(&mut payload).unwrap();
        (head[0], payload)
    }

    fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut f = vec![kind];
        f.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        f.extend_from_slice(payload);
        f
    }

    /// Unechte App: beantwortet jeden Auftrag mit `R`; auf `W` schickt sie die Antwort des
    /// Zählers in zwei Teilen, auf `B` 1200 einen Fehler
    fn fake_app() -> (String, std::thread::JoinHandle<Vec<Frame>>) {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap().to_string();
        let t = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut seen = Vec::new();
            loop {
                let mut head = [0u8; 1];
                if s.peek(&mut head).unwrap_or(0) == 0 {
                    return seen;
                }
                let (kind, payload) = read_frame(&mut s);
                let reply = match (kind, payload.as_slice()) {
                    (b'H', _) => Vec::new(),
                    (b'W', _) => [
                        frame(b'D', &[0x68, 0x03]),
                        frame(b'R', &[]),
                        frame(b'D', &[0x03]),
                    ]
                    .concat(),
                    (b'C', _) => [frame(b'D', &[0xFF]), frame(b'R', &[])].concat(),
                    (b'B', b"1200") => frame(b'E', b"nicht erlaubt"),
                    _ => frame(b'R', &[]),
                };
                s.write_all(&reply).unwrap();
                seen.push((kind, payload));
            }
        });
        (addr, t)
    }

    #[test]
    fn bridge_round_trip() {
        let (addr, app) = fake_app();
        let mut link = BridgeLink::connect(&addr).unwrap();
        link.request(b'O', b"1a86:7523\n2400").unwrap();
        link.clear().unwrap();
        link.send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
        let mut buf = [0u8; 8];
        let mut got = Vec::new();
        while got.len() < 3 {
            let n = link.recv(&mut buf, Duration::from_secs(2)).unwrap();
            assert!(n > 0);
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, [0x68, 0x03, 0x03]);
        assert_eq!(link.recv(&mut buf, Duration::from_millis(50)).unwrap(), 0);
        link.set_baud(9600).unwrap();
        assert!(link
            .set_baud(1200)
            .unwrap_err()
            .to_string()
            .contains("nicht erlaubt"));
        drop(link);
        let seen = app.join().unwrap();
        let kinds: Vec<u8> = seen.iter().map(|(k, _)| *k).collect();
        assert_eq!(kinds, b"HOCWBB");
        assert_eq!(seen[3].1, [0x10, 0x40, 0x01, 0x41, 0x16]);
    }

    #[test]
    fn open_without_app_explains() {
        std::env::remove_var(ADDR_VAR);
        let Err(e) = open("1a86:7523", 2400) else {
            panic!("ohne App darf usb:// nicht aufgehen")
        };
        assert!(e.to_string().contains("Android"));
        assert!(list().is_empty());
    }
}
