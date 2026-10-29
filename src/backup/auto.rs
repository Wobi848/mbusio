//! Automatische Sicherung (Konzept Sicherung §5): täglich zu einer Uhrzeit auf einen anderen
//! Rechner (Ordner, auch SMB/Netzlaufwerk über das Betriebssystem eingebunden; oder FTP/FTPS),
//! wahlweise mit einem Sicherungs-Passwort verschlüsselt (`crypt`). Am Ziel bleiben nur die
//! letzten `keep` eigenen Sicherungen dieses Geräts.

use std::path::Path;

use anyhow::{bail, Result};
use chrono::{Duration, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use super::crypt;
use super::ftp::{Ftp, FtpConfig};
use crate::db::Db;
use crate::i18n::tr;

/// bei einem Fehlschlag erst nach so vielen Minuten erneut versuchen
const RETRY_MINUTES: i64 = 15;
const TIME: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub enabled: bool,
    /// "folder" (Ordner, auch SMB/Netzlaufwerk) oder "ftp"
    pub target: String,
    /// Ordner auf diesem Gerät (USB-Stick, vom Betriebssystem eingebundenes Netzlaufwerk)
    pub folder: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    /// wird nie an die Oberfläche, ins Protokoll oder in einen Export gegeben
    pub password: String,
    /// Ordner auf dem FTP-Server
    pub dir: String,
    /// FTPS (explizit, „AUTH TLS“)
    pub tls: bool,
    pub insecure: bool,
    /// Uhrzeit wie „02:30“, 24 h
    pub time: String,
    /// so viele eigene Sicherungen bleiben am Ziel, ältere löscht mbusio dort selbst
    pub keep: u32,
    /// Sicherungs-Passwort; leer = nicht verschlüsselt. Wird nie an die Oberfläche, ins Protokoll
    /// oder in einen Export gegeben
    pub secret: String,
}

impl Default for BackupConfig {
    fn default() -> Self {
        BackupConfig {
            enabled: false,
            target: "folder".into(),
            folder: String::new(),
            host: String::new(),
            port: 21,
            user: String::new(),
            password: String::new(),
            dir: String::new(),
            tls: true,
            insecure: false,
            time: "02:30".into(),
            keep: 7,
            secret: String::new(),
        }
    }
}

impl BackupConfig {
    pub fn check(&self) -> Result<()> {
        if !["folder", "ftp"].contains(&self.target.as_str()) {
            bail!(tr("Ziel unbekannt", "Unknown target"));
        }
        if NaiveTime::parse_from_str(self.time.trim(), "%H:%M").is_err() {
            bail!(tr("Uhrzeit wie 02:30", "Time like 02:30"));
        }
        if !(1..=365).contains(&self.keep) {
            bail!(tr("Anzahl: 1 bis 365", "Count: 1 to 365"));
        }
        if self.target == "folder" && self.folder.trim().is_empty() {
            bail!(tr("Ordner fehlt", "Folder missing"));
        }
        if self.target == "ftp" && self.host.trim().is_empty() {
            bail!(tr("FTP-Server fehlt", "FTP server missing"));
        }
        Ok(())
    }

    /// Eingeschaltet und vollständig eingerichtet; sonst warum nicht
    pub fn ready(&self) -> Result<()> {
        if !self.enabled {
            bail!(tr(
                "Automatische Sicherung ist ausgeschaltet",
                "Automatic backup is switched off"
            ));
        }
        self.check()
    }

    /// Ziel als Text, für die Karte und das Protokoll der Weitergabe
    pub fn target_label(&self) -> String {
        if self.target == "ftp" {
            format!("FTP {}", self.host.trim())
        } else {
            format!("{} {}", tr("Ordner", "folder"), self.folder.trim())
        }
    }
}

/// Letzter Versuch, für die Karte und für „einmal am Tag“
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupState {
    pub time: String,
    pub bytes: u64,
    /// None = geglückt
    pub error: Option<String>,
    /// letzte geglückte Sicherung
    last_ok: Option<String>,
    /// letzter Fehlschlag, für die Wartezeit bis zum nächsten Versuch
    last_failed: Option<String>,
}

impl BackupState {
    /// Fällig: die eingestellte Uhrzeit ist seit der letzten geglückten Sicherung vorbei, und (nach
    /// einem Fehlschlag) die Wartezeit für einen neuen Versuch ist vorbei
    pub fn due(&self, time: &str, now: NaiveDateTime) -> bool {
        let Some(slot) = slot(time, now) else {
            return false;
        };
        let retrying = self
            .last_failed
            .as_deref()
            .and_then(parse)
            .is_some_and(|t| now - t < Duration::minutes(RETRY_MINUTES));
        !retrying && self.last_ok.as_deref().and_then(parse).is_none_or(|t| t < slot)
    }

    /// Ergebnis eines Versuchs festhalten
    pub fn done(&mut self, result: &Result<u64>, now: NaiveDateTime) {
        self.time = stamp(now);
        match result {
            Ok(bytes) => {
                self.bytes = *bytes;
                self.error = None;
                self.last_ok = Some(stamp(now));
                self.last_failed = None;
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                self.last_failed = Some(stamp(now));
            }
        }
    }
}

/// Letzter Termin bis jetzt (heute, sonst gestern)
fn slot(time: &str, now: NaiveDateTime) -> Option<NaiveDateTime> {
    let at = NaiveTime::parse_from_str(time.trim(), "%H:%M").ok()?;
    let slot = now.date().and_time(at);
    Some(if slot > now { slot - Duration::days(1) } else { slot })
}

fn parse(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIME).ok()
}

fn stamp(t: NaiveDateTime) -> String {
    t.format(TIME).to_string()
}

impl Db {
    pub fn backup_config(&self) -> BackupConfig {
        self.meta_json("backup")
    }

    pub fn set_backup_config(&mut self, c: &BackupConfig) -> Result<()> {
        c.check()?;
        self.set_meta("backup", &serde_json::to_string(c)?)
    }

    pub fn backup_state(&self) -> BackupState {
        self.meta_json("backup_state")
    }

    pub fn set_backup_state(&mut self, s: &BackupState) -> Result<()> {
        self.set_meta("backup_state", &serde_json::to_string(s)?)
    }

    /// Ergebnis eines Versuchs festhalten: Zustand für die Karte und ins Protokoll der Weitergabe
    /// (Ausgang `backup`). Jede Sicherung geht hier durch, ob durch den Zeitplan oder von Hand.
    pub fn backup_record(&mut self, target: &str, result: &Result<u64>, now: NaiveDateTime) -> Result<()> {
        let mut state = self.backup_state();
        state.done(result, now);
        let reason = result.as_ref().err().map(|e| format!("{e:#}")).unwrap_or_default();
        self.out_log(&crate::datapoint::OutLogEntry {
            output: "backup",
            dp: None,
            name: None,
            address: target,
            value: &result.as_ref().map(u64::to_string).unwrap_or_default(),
            quality: "",
            result: if result.is_ok() { "sent" } else { "failed" },
            reason: &reason,
        })?;
        self.set_backup_state(&state)
    }
}

/// Stimmige Kopie der Datenbank als Bytes (über eine Zwischendatei neben `mbusio.db`, wie beim
/// Herunterladen von Hand)
pub fn build(db: &Db) -> Result<Vec<u8>> {
    let dir = db
        .path()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow::anyhow!(tr("Keine Datenbankdatei", "No database file")))?;
    let tmp = dir.join("mbusio.db.autosicherung");
    if tmp.exists() {
        std::fs::remove_file(&tmp)?;
    }
    db.backup_to(&tmp)?;
    let bytes = std::fs::read(&tmp)?;
    std::fs::remove_file(&tmp)?;
    Ok(bytes)
}

/// Dateiname `mbusio-sicherung-<gerät>-<datum>_<zeit>.<db|mbusio>`
fn file_name(device: &str, encrypted: bool, now: chrono::DateTime<chrono::Local>) -> String {
    format!(
        "mbusio-sicherung-{device}-{}.{}",
        now.format("%Y-%m-%d_%H%M"),
        if encrypted { "mbusio" } else { "db" }
    )
}

/// Eigene Sicherungen dieses Geräts (Muster `mbusio-sicherung-<gerät>-*`) über `keep` hinaus,
/// älteste zuerst
fn surplus(names: &[String], device: &str, keep: u32) -> Vec<String> {
    let prefix = format!("mbusio-sicherung-{device}-");
    let mut mine: Vec<&String> = names.iter().filter(|n| n.starts_with(&prefix)).collect();
    mine.sort();
    let extra = mine.len().saturating_sub(keep as usize);
    mine[..extra].iter().map(|s| s.to_string()).collect()
}

/// Bei Bedarf verschlüsseln, ablegen (Ordner oder FTP) und am Ziel nur die letzten `keep` eigenen
/// Sicherungen behalten. Liefert die abgelegte Grösse. Braucht keine Datenbank-Sperre (Netz-I/O).
pub fn store(c: &BackupConfig, device: &str, data: &[u8]) -> Result<u64> {
    c.ready()?;
    let encrypted = !c.secret.is_empty();
    let bytes = if encrypted {
        crypt::encrypt(&c.secret, data)?
    } else {
        data.to_vec()
    };
    let name = file_name(device, encrypted, chrono::Local::now());
    match c.target.as_str() {
        "ftp" => store_ftp(c, &name, &bytes, device)?,
        _ => store_folder(c, &name, &bytes, device)?,
    }
    Ok(bytes.len() as u64)
}

fn store_folder(c: &BackupConfig, name: &str, bytes: &[u8], device: &str) -> Result<()> {
    let dir = Path::new(c.folder.trim());
    if !dir.is_dir() {
        bail!(tr("Ordner nicht gefunden", "folder not found"));
    }
    std::fs::write(dir.join(name), bytes)?;
    let names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    for old in surplus(&names, device, c.keep) {
        std::fs::remove_file(dir.join(old))?;
    }
    Ok(())
}

fn store_ftp(c: &BackupConfig, name: &str, bytes: &[u8], device: &str) -> Result<()> {
    let mut ftp = Ftp::connect(&FtpConfig {
        host: c.host.clone(),
        port: c.port,
        user: c.user.clone(),
        password: c.password.clone(),
        tls: c.tls,
        insecure: c.insecure,
    })?;
    ftp.cd(&c.dir)?;
    ftp.store(name, bytes)?;
    let names = ftp.list()?;
    for old in surplus(&names, device, c.keep) {
        ftp.delete(&old)?;
    }
    ftp.quit();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, TIME).unwrap()
    }

    #[test]
    fn config_checks_target_time_and_keep() {
        let mut c = BackupConfig::default();
        assert!(c.check().is_err(), "Ordner fehlt noch");
        c.folder = "/tmp".into();
        c.check().unwrap();
        c.time = "25:99".into();
        assert!(c.check().is_err());
        c.time = "02:30".into();
        c.keep = 0;
        assert!(c.check().is_err());
        c.keep = 7;
        c.target = "ftp".into();
        assert!(c.check().is_err(), "FTP-Server fehlt noch");
        c.host = "ftp.example.org".into();
        c.check().unwrap();
    }

    #[test]
    fn due_once_a_day_and_retries_after_failure() {
        let mut s = BackupState::default();
        assert!(s.due("02:30", at("2026-10-29 02:30:00")));
        s.done(&Ok(1234), at("2026-10-29 02:30:05"));
        assert!(!s.due("02:30", at("2026-10-29 10:00:00")), "heute schon geglückt");
        assert!(s.due("02:30", at("2026-10-30 02:31:00")), "neuer Tag");
        s.done(&Err(anyhow::anyhow!("kein Netz")), at("2026-10-30 02:31:00"));
        assert_eq!(s.error.as_deref(), Some("kein Netz"));
        assert!(!s.due("02:30", at("2026-10-30 02:40:00")), "Wartezeit nach Fehlschlag");
        assert!(s.due("02:30", at("2026-10-30 02:50:00")), "Wartezeit vorbei");
    }

    #[test]
    fn surplus_keeps_only_newest_and_ignores_other_devices() {
        let names = vec![
            "mbusio-sicherung-pi-2026-10-27_0230.db".to_string(),
            "mbusio-sicherung-pi-2026-10-28_0230.db".to_string(),
            "mbusio-sicherung-pi-2026-10-29_0230.db".to_string(),
            "mbusio-sicherung-anderes-2026-10-29_0230.db".to_string(),
            "etwas-anderes.txt".to_string(),
        ];
        let old = surplus(&names, "pi", 2);
        assert_eq!(old, vec!["mbusio-sicherung-pi-2026-10-27_0230.db"]);
    }

    #[test]
    fn build_and_store_folder_round_trip_with_cleanup() {
        let dir = std::env::temp_dir().join(format!("mbusio-autobackup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let data_dir = dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db = Db::open(&data_dir).unwrap();
        let data = build(&db).unwrap();
        assert!(!data.is_empty());

        let target = dir.join("ziel");
        std::fs::create_dir_all(&target).unwrap();
        // ältere eigene Sicherung, die beim Ablegen (keep=1) verschwinden muss
        std::fs::write(target.join("mbusio-sicherung-testgeraet-2000-01-01_0000.db"), b"alt").unwrap();
        let c = BackupConfig {
            enabled: true,
            folder: target.to_string_lossy().into_owned(),
            keep: 1,
            secret: "geheim".into(),
            ..Default::default()
        };
        store(&c, "testgeraet", &data).unwrap();
        let left: Vec<String> = std::fs::read_dir(&target)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left.len(), 1, "nur die neue (keep=1) bleibt");
        assert!(left[0].ends_with(".mbusio"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
