//! Eigener kleiner SMTP-Client: EHLO, STARTTLS bzw. TLS, AUTH PLAIN oder LOGIN, eine Nachricht

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

use super::{base64, MailConfig, Message};
use crate::i18n::tr;

const TIMEOUT: Duration = Duration::from_secs(20);

type Tls = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

enum Conn {
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

fn tls(c: &MailConfig, tcp: TcpStream) -> Result<Conn> {
    let name = rustls::pki_types::ServerName::try_from(c.host.trim().to_string())
        .map_err(|e| anyhow!("{}: {e}", c.host))?;
    let conn = rustls::ClientConnection::new(Arc::new(crate::tls::config(c.insecure, "")?), name)?;
    Ok(Conn::Tls(Box::new(rustls::StreamOwned::new(conn, tcp))))
}

struct Session {
    conn: Conn,
    buf: Vec<u8>,
}

impl Session {
    /// Eine Antwort des Servers (auch mehrzeilig): Code und Text der Zeilen
    fn reply(&mut self) -> Result<(u16, Vec<String>)> {
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
    fn cmd(&mut self, line: &str, shown: &str, want: &[u16]) -> Result<Vec<String>> {
        self.conn.write_all(format!("{line}\r\n").as_bytes())?;
        self.conn.flush()?;
        self.expect(shown, want)
    }

    fn expect(&mut self, shown: &str, want: &[u16]) -> Result<Vec<String>> {
        let (code, lines) = self.reply()?;
        if !want.contains(&code) {
            let why = if code == 535 {
                tr("Benutzer oder Passwort falsch", "bad user name or password")
            } else {
                tr("Server lehnt ab", "server refuses")
            };
            bail!("{shown}: {why} ({code} {})", lines.join(" "));
        }
        Ok(lines)
    }
}

/// Nachricht senden; Fehler nennen den Schritt und die Antwort des Servers, nie das Passwort
pub fn send(c: &MailConfig, m: &Message) -> Result<()> {
    let host = c.host.trim();
    let addr = (host, c.port)
        .to_socket_addrs()
        .with_context(|| format!("{host}:{}", c.port))?
        .next()
        .ok_or_else(|| anyhow!("{host}: {}", tr("Adresse unbekannt", "unknown address")))?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .with_context(|| format!("{host}:{}", c.port))?;
    tcp.set_read_timeout(Some(TIMEOUT))?;
    tcp.set_write_timeout(Some(TIMEOUT))?;
    let conn = if c.security == "tls" {
        tls(c, tcp)?
    } else {
        Conn::Plain(tcp)
    };
    let mut s = Session {
        conn,
        buf: Vec::new(),
    };
    s.expect(tr("Begrüssung", "greeting").as_str(), &[220])?;
    let mut caps = s.cmd("EHLO mbusio", "EHLO", &[250])?;
    if c.security == "starttls" {
        if !caps.iter().any(|l| l.eq_ignore_ascii_case("STARTTLS")) {
            bail!(tr(
                "Server kann kein STARTTLS (Verschlüsselung „TLS“ oder „keine“ wählen)",
                "Server does not offer STARTTLS (choose encryption “TLS” or “none”)"
            ));
        }
        s.cmd("STARTTLS", "STARTTLS", &[220])?;
        let Conn::Plain(tcp) = s.conn else {
            unreachable!()
        };
        s.conn = tls(c, tcp)?;
        s.buf.clear();
        caps = s.cmd("EHLO mbusio", "EHLO", &[250])?;
    }
    if !c.username.is_empty() {
        login(&mut s, c, &caps)?;
    }
    let from = c.sender()?;
    s.cmd(&format!("MAIL FROM:<{from}>"), &from, &[250])?;
    for to in &m.to {
        s.cmd(&format!("RCPT TO:<{to}>"), to, &[250, 251])?;
    }
    s.cmd("DATA", "DATA", &[354])?;
    let id = format!(
        "{}.{}",
        chrono::Local::now().format("%Y%m%d%H%M%S%f"),
        std::process::id()
    );
    let body = m.render(&chrono::Local::now().to_rfc2822(), &id);
    s.conn.write_all(body.as_bytes())?;
    s.cmd(".", tr("Nachricht", "message").as_str(), &[250])?;
    // Antwort auf QUIT ist egal: die Nachricht ist angenommen
    let _ = s.cmd("QUIT", "QUIT", &[221]);
    Ok(())
}

fn login(s: &mut Session, c: &MailConfig, caps: &[String]) -> Result<()> {
    let auth = caps
        .iter()
        .find(|l| l.to_ascii_uppercase().starts_with("AUTH"))
        .map(|l| l.to_ascii_uppercase())
        .unwrap_or_default();
    let shown = tr("Anmeldung", "login");
    if auth.contains("PLAIN") || !auth.contains("LOGIN") {
        let token = base64(format!("\0{}\0{}", c.username, c.password).as_bytes());
        s.cmd(&format!("AUTH PLAIN {token}"), &shown, &[235])?;
    } else {
        s.cmd("AUTH LOGIN", &shown, &[334])?;
        s.cmd(&base64(c.username.as_bytes()), &shown, &[334])?;
        s.cmd(&base64(c.password.as_bytes()), &shown, &[235])?;
    }
    Ok(())
}
