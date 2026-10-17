//! Physische Verbindung zum Bus: serielle Schnittstelle oder Simulator.

use std::io::{self, Read, Write};
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
}

pub const SIMULATOR_PORT: &str = "SIMULATOR";

pub fn list_ports() -> Vec<String> {
    let mut ports = vec![SIMULATOR_PORT.to_string()];
    if let Ok(found) = serialport::available_ports() {
        ports.extend(found.into_iter().map(|p| p.port_name));
    }
    ports
}
