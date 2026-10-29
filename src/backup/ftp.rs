//! Eigener kleiner FTP-Client für die automatische Sicherung: anmelden, Ordner wechseln/anlegen,
//! Datei hochladen, Liste, löschen. FTPS (`tls`) wechselt mit „AUTH TLS“ wie STARTTLS bei SMTP,
//! dieselbe `Arc<rustls::ClientConfig>` für Steuer- und Datenverbindung erlaubt vielen Servern die
//! Wiederaufnahme der TLS-Sitzung auf der Datenverbindung (ohne die lehnen sie PASV-Daten ab).

use std::io::{Read, Write};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use crate::i18n::tr;
use crate::textproto::{connect, tls, Conn, Session};

pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub tls: bool,
    pub insecure: bool,
}

pub struct Ftp {
    session: Session,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    host: String,
}

impl Ftp {
    /// Verbinden, bei Bedarf auf FTPS wechseln, anmelden, binären Modus setzen
    pub fn connect(c: &FtpConfig) -> Result<Self> {
        let host = c.host.trim().to_string();
        let tcp = connect(&host, c.port)?;
        let mut session = Session::new(Conn::Plain(tcp));
        session.expect(tr("Begrüssung", "greeting").as_str(), &[220])?;
        let tls_config = if c.tls {
            Some(Arc::new(crate::tls::config(c.insecure, "")?))
        } else {
            None
        };
        if let Some(config) = &tls_config {
            session.cmd("AUTH TLS", "AUTH TLS", &[234])?;
            session = session.upgrade(config.clone(), &host)?;
            session.cmd("PBSZ 0", "PBSZ", &[200])?;
            session.cmd("PROT P", "PROT", &[200])?;
        }
        let mut ftp = Ftp {
            session,
            tls_config,
            host,
        };
        ftp.login(&c.user, &c.password)?;
        ftp.session.cmd("TYPE I", "TYPE I", &[200])?;
        Ok(ftp)
    }

    fn login(&mut self, user: &str, password: &str) -> Result<()> {
        let shown = tr("Anmeldung", "login");
        let (code, _) =
            self.session
                .cmd_code(&format!("USER {user}"), &shown, &[230, 331])?;
        if code == 331 {
            self.session
                .cmd(&format!("PASS {password}"), &shown, &[230])?;
        }
        Ok(())
    }

    /// In den Ordner wechseln, fehlende Teile werden angelegt (jeder Teil einzeln mit CWD/MKD)
    pub fn cd(&mut self, dir: &str) -> Result<()> {
        let dir = dir.trim();
        if dir.is_empty() || dir == "/" {
            return Ok(());
        }
        if dir.starts_with('/') {
            self.session.cmd("CWD /", "CWD", &[250])?;
        }
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            let (code, _) = self
                .session
                .cmd_code(&format!("CWD {part}"), "CWD", &[250, 550])?;
            if code == 550 {
                self.session.cmd(&format!("MKD {part}"), "MKD", &[257])?;
                self.session.cmd(&format!("CWD {part}"), "CWD", &[250])?;
            }
        }
        Ok(())
    }

    /// Passiven Datenkanal aufbauen: Adresse per PASV erfragen und verbinden (gleiche TLS-Sitzung
    /// wie die Steuerverbindung, falls FTPS)
    fn data_conn(&mut self) -> Result<Conn> {
        let lines = self.session.cmd("PASV", "PASV", &[227])?;
        let reply = lines.join(" ");
        let inside = reply
            .split('(')
            .nth(1)
            .and_then(|s| s.split(')').next())
            .ok_or_else(|| anyhow!(tr("Antwort auf PASV unklar", "unclear answer to PASV")))?;
        let nums: Vec<u16> = inside.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        let [a, b, c, d, p1, p2] = nums[..] else {
            bail!(tr("Antwort auf PASV unklar", "unclear answer to PASV"));
        };
        let ip = format!("{a}.{b}.{c}.{d}");
        let port = p1 * 256 + p2;
        let tcp = connect(&ip, port)?;
        match &self.tls_config {
            Some(config) => tls(config.clone(), &self.host, tcp),
            None => Ok(Conn::Plain(tcp)),
        }
    }

    /// Datei hochladen (überschreibt eine gleichnamige)
    pub fn store(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let mut conn = self.data_conn()?;
        self.session
            .cmd(&format!("STOR {name}"), "STOR", &[150, 125])?;
        conn.write_all(data)?;
        conn.finish()?;
        self.session.expect("STOR", &[226, 250])?;
        Ok(())
    }

    /// Dateinamen im aktuellen Ordner (leerer Ordner: leere Liste statt Fehler)
    pub fn list(&mut self) -> Result<Vec<String>> {
        let mut conn = self.data_conn()?;
        let (code, _) = self.session.cmd_code("NLST", "NLST", &[150, 125, 450])?;
        if code == 450 {
            return Ok(Vec::new());
        }
        let mut raw = Vec::new();
        conn.read_to_end(&mut raw)?;
        self.session.expect("NLST", &[226, 250])?;
        Ok(String::from_utf8_lossy(&raw)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Datei im aktuellen Ordner löschen
    pub fn delete(&mut self, name: &str) -> Result<()> {
        self.session
            .cmd(&format!("DELE {name}"), "DELE", &[250])?;
        Ok(())
    }

    /// Verbindung ordentlich beenden; die Antwort auf QUIT ist egal
    pub fn quit(mut self) {
        let _ = self.session.cmd("QUIT", "QUIT", &[221]);
    }
}

#[cfg(test)]
mod tests;
