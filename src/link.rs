//! Physische Verbindung zum Bus: serielle Schnittstelle, TCP-Gateway oder Simulator.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::i18n::tr;
use anyhow::{Context, Result};
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};

pub trait Link: Send {
    fn send(&mut self, data: &[u8]) -> io::Result<()>;
    /// Liest verfügbare Bytes, wartet höchstens `timeout`. `Ok(0)` bei Timeout.
    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;
    /// Empfangspuffer verwerfen.
    fn clear(&mut self) -> io::Result<()>;
    /// Baudrate wechseln (für Baudratensuche und Busse mit gemischten Baudraten).
    fn set_baud(&mut self, baud: u32) -> io::Result<()>;
}

pub struct SerialLink {
    port: Box<dyn SerialPort>,
}

impl SerialLink {
    /// M-Bus: 8 Datenbits, gerade Parität, 1 Stopbit.
    pub fn open(name: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(name, baud)
            .data_bits(DataBits::Eight)
            .parity(Parity::Even)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .timeout(Duration::from_millis(100))
            .open()
            .with_context(|| {
                format!(
                    "{} {name}",
                    tr("Port konnte nicht geöffnet werden:", "Could not open port")
                )
            })?;
        Ok(Self { port })
    }
}

impl Link for SerialLink {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.port.write_all(data)?;
        self.port.flush()
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.port
            .set_timeout(timeout.max(Duration::from_millis(1)))
            .map_err(io::Error::other)?;
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }

    fn clear(&mut self) -> io::Result<()> {
        self.port
            .clear(ClearBuffer::Input)
            .map_err(io::Error::other)
    }

    fn set_baud(&mut self, baud: u32) -> io::Result<()> {
        self.port.set_baud_rate(baud).map_err(io::Error::other)
    }
}

/// M-Bus-Master mit Netzwerkanschluss (transparentes TCP, wie `mbus-tcp-*` von libmbus).
pub struct TcpLink {
    stream: TcpStream,
}

pub const TCP_PREFIX: &str = "tcp://";

impl TcpLink {
    /// `host:port`, z.B. `192.168.1.50:10001`
    pub fn open(addr: &str) -> Result<Self> {
        let context = || {
            format!(
                "{} {addr}",
                tr("Keine Verbindung zu", "Could not connect to")
            )
        };
        let sock = addr
            .to_socket_addrs()
            .with_context(context)?
            .next()
            .with_context(context)?;
        let stream =
            TcpStream::connect_timeout(&sock, Duration::from_secs(3)).with_context(context)?;
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }
}

impl Link for TcpLink {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.stream.write_all(data)?;
        self.stream.flush()
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))?;
        match self.stream.read(buf) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                tr(
                    "Gateway hat die Verbindung geschlossen",
                    "Gateway closed the connection",
                ),
            )),
            Ok(n) => Ok(n),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Ok(0)
            }
            Err(e) => Err(e),
        }
    }

    fn clear(&mut self) -> io::Result<()> {
        let mut tmp = [0u8; 256];
        while self.recv(&mut tmp, Duration::from_millis(1))? > 0 {}
        Ok(())
    }

    fn set_baud(&mut self, _baud: u32) -> io::Result<()> {
        // Die Baudrate auf dem Bus stellt man am Gateway ein, nicht über TCP
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            tr(
                "Bei TCP-Gateways wird die Baudrate am Gateway eingestellt",
                "With TCP gateways the baud rate is set on the gateway",
            ),
        ))
    }
}

pub const SIMULATOR_PORT: &str = "SIMULATOR";

/// Anschluss zum Vergleich: gleicher Schlüssel = dasselbe Gerät. `tcp://Host:Port` = `host:port`,
/// Windows `COM3` = `com3` = `\\.\COM3`; Pfade wie `/dev/ttyUSB0` bleiben, wie sie sind.
/// `None` für Simulationen: jede Linie hat ihre eigene.
pub fn port_key(port: &str) -> Option<String> {
    let p = port.trim();
    if p == SIMULATOR_PORT || p.starts_with("SIMULATOR:") {
        return None;
    }
    if p.starts_with('/') {
        return Some(p.to_string());
    }
    let p = p.to_ascii_lowercase();
    let p = p.strip_prefix(TCP_PREFIX).unwrap_or(&p);
    Some(p.strip_prefix(r"\\.\").unwrap_or(p).to_string())
}

/// `SIMULATOR`, `tcp://host:port` oder Name der seriellen Schnittstelle
pub fn open(port: &str, baud: u32) -> Result<Box<dyn Link>> {
    Ok(if port == SIMULATOR_PORT {
        Box::new(crate::sim::Simulator::new(baud))
    } else if let Some(key) = port.strip_prefix("SIMULATOR:") {
        Box::new(crate::sim::open(key, baud)?)
    } else if let Some(addr) = port.strip_prefix(TCP_PREFIX) {
        Box::new(TcpLink::open(addr)?)
    } else {
        Box::new(SerialLink::open(port, baud)?)
    })
}

/// Ein Anschluss zur Auswahl: Name zum Verbinden, dazu lesbar, was dahintersteckt
#[derive(Debug, Clone, serde::Serialize)]
pub struct PortInfo {
    pub port: String,
    /// z.B. „CP210x UART Bridge (Silicon Labs)“; leer, wenn nichts bekannt ist
    pub label: String,
}

/// Anschlüsse zur Auswahl: USB-Wandler zuerst (mit Namen), dann andere serielle Anschlüsse,
/// zuletzt die Simulationen. Die festen `/dev/ttyS*` ohne erkanntes Gerät (Linux legt sie immer
/// an, auch ohne Hardware) fehlen; eine echte RS-232 lässt sich von Hand eintragen.
pub fn list_ports() -> Vec<PortInfo> {
    let mut usb = Vec::new();
    let mut other = Vec::new();
    for p in serialport::available_ports().unwrap_or_default() {
        match p.port_type {
            serialport::SerialPortType::UsbPort(u) => {
                let name = u
                    .product
                    .unwrap_or_else(|| format!("USB {:04x}:{:04x}", u.vid, u.pid));
                let label = match u.manufacturer {
                    Some(m) if !name.contains(&m) => format!("{name} ({m})"),
                    _ => name,
                };
                usb.push(PortInfo {
                    port: p.port_name,
                    label,
                });
            }
            serialport::SerialPortType::Unknown if p.port_name.starts_with("/dev/ttyS") => {}
            _ => other.push(PortInfo {
                port: p.port_name,
                label: String::new(),
            }),
        }
    }
    let sims = std::iter::once(SIMULATOR_PORT.to_string())
        .chain(
            crate::sim::all()
                .into_iter()
                .filter(|s| s.key != "praxis")
                .map(|s| format!("{SIMULATOR_PORT}:{}", s.key)),
        )
        .map(|port| PortInfo {
            port,
            label: String::new(),
        });
    usb.into_iter().chain(other).chain(sims).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn port_key_treats_spellings_of_one_port_alike() {
        let k = |p: &str| port_key(p);
        assert_eq!(k("tcp://GW.local:10001"), k("gw.local:10001"));
        assert_eq!(k("COM3"), k("com3"));
        assert_eq!(k(r"\\.\COM3"), k("COM3"));
        assert_ne!(
            k("/dev/ttyUSB0"),
            k("/dev/ttyusb0"),
            "Pfade unterscheiden Gross/klein"
        );
        assert_eq!(k("SIMULATOR"), None);
        assert_eq!(k("SIMULATOR:ok10"), None);
    }

    #[test]
    fn tcp_link_round_trip() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut buf = [0u8; 5];
            s.read_exact(&mut buf).unwrap();
            s.write_all(&[0xE5]).unwrap();
            buf
        });
        let mut link = open(&format!("{TCP_PREFIX}{addr}"), 2400).unwrap();
        link.send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
        let mut buf = [0u8; 8];
        let n = link.recv(&mut buf, Duration::from_secs(2)).unwrap();
        assert_eq!(&buf[..n], &[0xE5]);
        assert_eq!(t.join().unwrap(), [0x10, 0x40, 0x01, 0x41, 0x16]);
        assert!(link.set_baud(9600).is_err());
    }
}
