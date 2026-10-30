//! Textprotokolle mit Zahlencode in der Antwort (SMTP, FTP): Verbindung mit oder ohne TLS,
//! Befehl senden, Antwort lesen (auch mehrzeilig, „250-…“ bis „250 …“)

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

use crate::i18n::tr;

const TIMEOUT: Duration = Duration::from_secs(20);

type Tls = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

pub enum Conn {
    Plain(TcpStream),
    Tls(Box<Tls>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            Conn::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            Conn::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(s) => s.flush(),
        }
    }
}

impl Conn {
    /// TLS sauber beenden (close_notify): FTP-Server werten eine Datei sonst als abgebrochen
    pub fn finish(self) -> Result<()> {
        if let Conn::Tls(mut s) = self {
            s.conn.send_close_notify();
            s.flush()?;
            // erst das Schreiben beenden, dann lesen, bis der Server schliesst: bliebe etwas
            // ungelesen (z.B. ein Sitzungs-Ticket), käme das Schliessen als RST an, und der Server
            // verwürfe, was er noch nicht verarbeitet hat (abgeschnittene Datei)
            s.sock.shutdown(std::net::Shutdown::Write)?;
            let mut rest = [0u8; 4096];
            while matches!(s.read(&mut rest), Ok(n) if n > 0) {}
        }
        Ok(())
    }
}

/// TCP-Verbindung mit Zeitgrenzen; Fehler nennen Server und Port
/// TCP-Verbindung zu `addr` (Name oder IP mit Port): alle Adressen des Namens der Reihe nach,
/// z.B. `localhost` erst ::1, dann 127.0.0.1 (für SMTP, FTP, MQTT und M-Bus-Gateways)
pub fn tcp<A: ToSocketAddrs + std::fmt::Display>(addr: A, timeout: Duration) -> Result<TcpStream> {
    let mut last = None;
    for a in addr.to_socket_addrs().with_context(|| addr.to_string())? {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(t) => return Ok(t),
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) => anyhow!(e).context(addr.to_string()),
        None => anyhow!("{addr}: {}", tr("Adresse unbekannt", "unknown address")),
    })
}

/// Verbindung für SMTP und FTP, mit Zeitgrenzen für Lesen und Schreiben
pub fn connect(host: &str, port: u16) -> Result<TcpStream> {
    let tcp = tcp(format!("{host}:{port}"), Duration::from_secs(10))?;
    tcp.set_read_timeout(Some(TIMEOUT))?;
    tcp.set_write_timeout(Some(TIMEOUT))?;
    Ok(tcp)
}

/// TLS über eine bestehende Verbindung. Dieselbe `config` für mehrere Verbindungen zum selben
/// Server erlaubt die Wiederaufnahme der Sitzung (verlangen viele FTPS-Server für die Daten)
pub fn tls(config: Arc<rustls::ClientConfig>, host: &str, tcp: TcpStream) -> Result<Conn> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| anyhow!("{host}: {e}"))?;
    let conn = rustls::ClientConnection::new(config, name)?;
    Ok(Conn::Tls(Box::new(rustls::StreamOwned::new(conn, tcp))))
}

pub struct Session {
    pub conn: Conn,
    buf: Vec<u8>,
}

impl Write for Session {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.conn.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.conn.flush()
    }
}

impl Session {
    pub fn new(conn: Conn) -> Self {
        Session {
            conn,
            buf: Vec::new(),
        }
    }

    /// Von unverschlüsselt auf TLS wechseln (STARTTLS, FTP „AUTH TLS“). Was noch unverschlüsselt
    /// im Puffer lag, gilt nicht mehr (schützt vor Befehlen, die vor dem Wechsel eingeschleust wurden)
    pub fn upgrade(mut self, config: Arc<rustls::ClientConfig>, host: &str) -> Result<Self> {
        let Conn::Plain(tcp) = self.conn else {
            bail!(tr(
                "Verbindung ist schon verschlüsselt",
                "connection is already encrypted"
            ));
        };
        self.conn = tls(config, host, tcp)?;
        self.buf.clear();
        Ok(self)
    }

    /// Eine Antwort des Servers (auch mehrzeilig): Code und Text der Zeilen
    pub fn reply(&mut self) -> Result<(u16, Vec<String>)> {
        let mut lines = Vec::new();
        loop {
            while let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") {
                let line = String::from_utf8_lossy(&self.buf[..i]).into_owned();
                self.buf.drain(..i + 2);
                let last = line.as_bytes().get(3) != Some(&b'-');
                let code = line.get(..3).and_then(|c| c.parse().ok());
                lines.push(line.get(4..).unwrap_or_default().to_string());
                if last {
                    let code = code.ok_or_else(|| {
                        anyhow!("{}: {line}", tr("Antwort unklar", "unclear answer"))
                    })?;
                    return Ok((code, lines));
                }
            }
            let mut chunk = [0u8; 1024];
            let n = self
                .conn
                .read(&mut chunk)
                .context(tr("keine Antwort vom Server", "no answer from server"))?;
            if n == 0 {
                bail!(tr(
                    "Server hat die Verbindung beendet",
                    "server closed the connection"
                ));
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// Befehl senden und eine Antwort mit Code `want` erwarten; `shown`: was im Fehler steht
    /// (nie das Passwort)
    pub fn cmd(&mut self, line: &str, shown: &str, want: &[u16]) -> Result<Vec<String>> {
        Ok(self.cmd_code(line, shown, want)?.1)
    }

    /// Wie [`Session::cmd`], liefert aber auch den Code (z.B. FTP „USER“: 230 ohne, 331 mit Passwort)
    pub fn cmd_code(
        &mut self,
        line: &str,
        shown: &str,
        want: &[u16],
    ) -> Result<(u16, Vec<String>)> {
        self.conn.write_all(format!("{line}\r\n").as_bytes())?;
        self.conn.flush()?;
        self.expect_code(shown, want)
    }

    pub fn expect(&mut self, shown: &str, want: &[u16]) -> Result<Vec<String>> {
        Ok(self.expect_code(shown, want)?.1)
    }

    /// Wie [`Session::expect`], liefert aber auch den Code
    pub fn expect_code(&mut self, shown: &str, want: &[u16]) -> Result<(u16, Vec<String>)> {
        let (code, lines) = self.reply()?;
        if !want.contains(&code) {
            // 530 FTP, 535 SMTP: Anmeldung abgelehnt
            let why = if code == 530 || code == 535 {
                tr("Benutzer oder Passwort falsch", "bad user name or password")
            } else {
                tr("Server lehnt ab", "server refuses")
            };
            bail!("{shown}: {why} ({code} {})", lines.join(" "));
        }
        Ok((code, lines))
    }
}
