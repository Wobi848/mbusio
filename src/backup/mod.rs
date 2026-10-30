//! Sicherung und Wiederherstellung (Konzept Sicherung): der ganze Stand eines Geräts ist die
//! Datenbank. Gesichert wird eine stimmige Kopie (`snapshot`, eigene Verbindung, ohne Sperre);
//! eingespielt wird beim Start, bevor etwas die Datenbank öffnet (`restore_pending`), der bisherige
//! Stand bleibt als Rückfall. Alles läuft über Dateien, nie ganz im Arbeitsspeicher (Pi).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::i18n::tr;

pub mod auto;
pub mod crypt;
pub mod ftp;

/// Gerätename für einen Dateinamen: nur Buchstaben, Ziffern und „-“
pub(crate) fn safe_device_name(device: &str) -> String {
    device
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

const DB: &str = "mbusio.db";
/// hochgeladen und geprüft, wartet auf „Einspielen“
const UPLOAD: &str = "mbusio.db.upload";
/// beim nächsten Start einsetzen
const RESTORE: &str = "mbusio.db.restore";
/// Stand vor dem Einspielen (eine Rückfalldatei, die nächste ersetzt sie)
const BEFORE: &str = "mbusio.db.vorher";
/// Zwischendateien beim Sichern (`mbusio.db.sicherung-<n>.db|.mbusio`), danach gelöscht
const TEMP: &str = "mbusio.db.sicherung-";
/// Tabellen, die jede Datenbank von mbusio hat
const TABLES: [&str; 5] = ["meta", "projects", "lines", "meters", "readings"];

/// Was in einer Sicherung steckt (für die Rückfrage vor dem Einspielen)
#[derive(Debug, Serialize)]
pub struct Summary {
    /// mbusio-Version, die sie geschrieben hat (ältere Sicherungen kennen sie nicht)
    pub version: Option<String>,
    pub projects: Vec<String>,
    pub meters: i64,
    pub readings: i64,
    /// erste und letzte Auslesung (Ortszeit)
    pub first: Option<String>,
    pub last: Option<String>,
}

/// Stimmige Kopie der Datenbank nach `to`, über eine eigene, nur lesende Verbindung: braucht die
/// Sperre der Datenbank nicht, Oberfläche und Plan laufen währenddessen weiter
pub fn snapshot(db_file: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        std::fs::remove_file(to)?;
    }
    let c = Connection::open_with_flags(db_file, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| db_file.display().to_string())?;
    c.execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
    Ok(())
}

/// Sicherungsdatei neben der Datenbank: Kopie, mit Sicherungs-Passwort verschlüsselt. Liefert die
/// Zwischendatei (der Aufrufer löscht sie nach dem Ablegen) und ob sie verschlüsselt ist.
pub fn make(db_file: &Path, secret: &str) -> Result<(PathBuf, bool)> {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = db_file
        .parent()
        .ok_or_else(|| anyhow::anyhow!(tr("Keine Datenbankdatei", "No database file")))?;
    let n = N.fetch_add(1, Ordering::Relaxed);
    let plain = dir.join(format!("{TEMP}{n}.db"));
    snapshot(db_file, &plain)?;
    if secret.is_empty() {
        return Ok((plain, false));
    }
    let sealed = dir.join(format!("{TEMP}{n}.mbusio"));
    let result = crypt::encrypt_file(secret, &plain, &sealed);
    std::fs::remove_file(&plain)?;
    result?;
    Ok((sealed, true))
}

/// Beim Start: Zwischendateien abgebrochener Sicherungen (z.B. Herunterladen abgebrochen) löschen
pub fn clean_temp(dir: &Path) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().starts_with(TEMP) {
            std::fs::remove_file(e.path())?;
        }
    }
    Ok(())
}

/// Hierhin schreibt die Oberfläche eine hochgeladene Sicherung, Stück für Stück
pub fn upload_file(dir: &Path) -> PathBuf {
    dir.join(UPLOAD)
}

/// Warum eine hochgeladene Sicherung nicht angenommen wurde
pub enum Refused {
    /// verschlüsselt: Passwort fehlt oder ist falsch (die Oberfläche fragt danach)
    Password(String),
    Other(anyhow::Error),
}

/// Hochgeladene Sicherung annehmen: falls verschlüsselt mit `password` entschlüsseln, dann prüfen;
/// liefert, was drin ist. Eine unbrauchbare Datei bleibt nicht liegen.
pub fn accept(dir: &Path, password: &str) -> std::result::Result<Summary, Refused> {
    let result =
        unpack(dir, password).and_then(|()| check(&upload_file(dir)).map_err(Refused::Other));
    if result.is_err() {
        discard(dir).map_err(Refused::Other)?;
    }
    result
}

fn unpack(dir: &Path, password: &str) -> std::result::Result<(), Refused> {
    let up = upload_file(dir);
    if !crypt::is_encrypted_file(&up).map_err(Refused::Other)? {
        return Ok(());
    }
    if password.is_empty() {
        return Err(Refused::Password(tr(
            "Sicherung ist verschlüsselt: Passwort nötig",
            "Backup is encrypted: password required",
        )));
    }
    let plain = dir.join(format!("{UPLOAD}.plain"));
    crypt::decrypt_file(password, &up, &plain).map_err(|e| Refused::Password(format!("{e:#}")))?;
    std::fs::rename(&plain, &up).map_err(|e| Refused::Other(e.into()))
}

/// Geprüfte Sicherung für den nächsten Start bereitlegen
pub fn apply(dir: &Path) -> Result<()> {
    let upload = dir.join(UPLOAD);
    if !upload.exists() {
        bail!(tr(
            "Keine geprüfte Sicherung vorhanden",
            "No checked backup present"
        ));
    }
    std::fs::rename(&upload, dir.join(RESTORE))?;
    Ok(())
}

/// Hochgeladene Sicherung verwerfen (Rückfrage abgelehnt)
pub fn discard(dir: &Path) -> Result<()> {
    match std::fs::remove_file(dir.join(UPLOAD)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Beim Start, vor dem Öffnen der Datenbank: liegt eine Sicherung bereit, ersetzt sie den Stand.
/// Der bisherige (mit seinen WAL-Dateien) bleibt als `mbusio.db.vorher`. true = eingespielt.
pub fn restore_pending(dir: &Path) -> Result<bool> {
    let restore = dir.join(RESTORE);
    if !restore.exists() {
        return Ok(false);
    }
    for suffix in ["", "-wal", "-shm"] {
        let keep = dir.join(format!("{BEFORE}{suffix}"));
        if keep.exists() {
            std::fs::remove_file(&keep)?;
        }
        let old = dir.join(format!("{DB}{suffix}"));
        if old.exists() {
            std::fs::rename(&old, &keep)?;
        }
    }
    std::fs::rename(&restore, dir.join(DB))?;
    Ok(true)
}

/// Ist das eine brauchbare Sicherung von mbusio, die diese Version lesen kann?
pub fn check(path: &Path) -> Result<Summary> {
    let not_ours = || {
        anyhow::anyhow!(tr(
            "Die Datei ist keine Sicherung von mbusio",
            "The file is not an mbusio backup"
        ))
    };
    let mut head = [0u8; 16];
    std::fs::File::open(path)?
        .read_exact(&mut head)
        .map_err(|_| not_ours())?;
    if &head != b"SQLite format 3\0" {
        return Err(not_ours());
    }
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        bail!(
            "{}: {integrity}",
            tr("Die Sicherung ist beschädigt", "The backup is damaged")
        );
    }
    for table in TABLES {
        let found: i64 = c.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get(0),
        )?;
        if found == 0 {
            return Err(not_ours());
        }
    }
    let version: Option<String> = c
        .query_row(
            "SELECT value FROM meta WHERE key = 'app_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(v) = version
        .as_deref()
        .filter(|v| newer(v, env!("CARGO_PKG_VERSION")))
    {
        bail!(
            "{} {v} ({} {})",
            tr(
                "Die Sicherung stammt von einer neueren mbusio-Version:",
                "The backup comes from a newer mbusio version:"
            ),
            tr("hier läuft", "this is"),
            env!("CARGO_PKG_VERSION")
        );
    }
    let projects = c
        .prepare("SELECT name FROM projects ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let count = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0));
    let (first, last) = c.query_row(
        "SELECT MIN(timestamp), MAX(timestamp) FROM readings",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(Summary {
        version,
        projects,
        meters: count("SELECT COUNT(*) FROM meters")?,
        readings: count("SELECT COUNT(*) FROM readings")?,
        first,
        last,
    })
}

/// Version a neuer als b ("0.10.0" > "0.9.3")
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u32> { v.split('.').map(|x| x.parse().unwrap_or(0)).collect() };
    parts(a) > parts(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Db, Meter};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("mbusio-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Hochladen wie die Oberfläche: Datei an die Stelle schreiben, dann annehmen
    fn upload(dir: &Path, bytes: &[u8], password: &str) -> Result<Summary> {
        std::fs::write(upload_file(dir), bytes).unwrap();
        accept(dir, password).map_err(|e| match e {
            Refused::Password(m) => anyhow::anyhow!(m),
            Refused::Other(e) => e,
        })
    }

    /// Gerät mit einem Zähler; liefert den Ordner und die Sicherung als Bytes
    fn device_with_backup(name: &str, secret: &str) -> (std::path::PathBuf, Vec<u8>) {
        let d = dir(name);
        let mut db = Db::open(&d).unwrap();
        let line = db.active_line().unwrap().id;
        let meter = Meter {
            address: "7".into(),
            name: "Whg 7".into(),
            ..Default::default()
        };
        db.add_meter(line, meter).unwrap();
        let (file, encrypted) = make(db.path().unwrap(), secret).unwrap();
        assert_eq!(encrypted, !secret.is_empty());
        let bytes = std::fs::read(&file).unwrap();
        std::fs::remove_file(file).unwrap();
        (d, bytes)
    }

    #[test]
    fn backup_restores_on_next_start_and_keeps_the_old_state() {
        let (old, bytes) = device_with_backup("quelle", "");
        // neues Gerät: leer, Sicherung hochladen, prüfen, bereitlegen
        let new = dir("ziel");
        drop(Db::open(&new).unwrap());
        let s = upload(&new, &bytes, "").unwrap();
        assert_eq!(s.meters, 1);
        assert_eq!(s.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        apply(&new).unwrap();
        // Start: eingesetzt, bisheriger Stand als Rückfall
        assert!(restore_pending(&new).unwrap());
        assert!(new.join(BEFORE).exists());
        let db = Db::open(&new).unwrap();
        let line = db.active_line().unwrap().id;
        assert_eq!(db.meters(line).unwrap()[0].name, "Whg 7");
        // nur einmal
        assert!(!restore_pending(&new).unwrap());
        for d in [old, new] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn refuses_other_files_and_newer_versions() {
        let d = dir("fremd");
        assert!(upload(&d, b"kein SQLite", "").is_err());
        assert!(
            !d.join(UPLOAD).exists(),
            "unbrauchbare Datei bleibt nicht liegen"
        );
        let other = d.join("andere.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert!(upload(&d, &std::fs::read(&other).unwrap(), "").is_err());
        assert!(newer("0.10.0", "0.9.3"));
        assert!(!newer("0.5.0", "0.5.0"));
        assert!(
            apply(&d).is_err(),
            "ohne geprüfte Sicherung nichts bereitlegen"
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn encrypted_backup_needs_the_password() {
        let (old, bytes) = device_with_backup("geheim", "Grüße");
        assert!(crypt::is_encrypted(&bytes));
        let new = dir("geheim-ziel");
        std::fs::write(upload_file(&new), &bytes).unwrap();
        assert!(matches!(accept(&new, ""), Err(Refused::Password(_))));
        std::fs::write(upload_file(&new), &bytes).unwrap();
        assert!(matches!(accept(&new, "falsch"), Err(Refused::Password(_))));
        assert_eq!(upload(&new, &bytes, "Grüße").unwrap().meters, 1);
        for d in [old, new] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }
}
