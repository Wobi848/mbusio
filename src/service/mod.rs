//! mbusio im Hintergrund (Konzept Hintergrund): erkennen, ob es als Dienst läuft, Daten für den
//! Dienst übernehmen, Log-Datei des Dienstes, Energiesparmodus verhindern. Der Windows-Dienst
//! selbst (einrichten, entfernen, laufen) steht in `win` und gibt es nur unter Windows.
// die Bausteine des Dienstes werden nur unter Windows benutzt, getestet werden sie überall
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use anyhow::Result;

/// Name des Windows-Dienstes
pub const NAME: &str = "mbusio";
const DB_FILE: &str = "mbusio.db";
const LOG_FILE: &str = "mbusio.log";
const LOG_MAX: u64 = 1024 * 1024;

static AS_SERVICE: AtomicBool = AtomicBool::new(false);
/// Tests: fester Wert statt Erkennung (0 = erkennen, 1 = nein, 2 = ja). Sonst hängt das Ergebnis
/// davon ab, wo die Tests laufen: ein CI-Runner ist selbst ein systemd-Dienst.
static TEST_BACKGROUND: AtomicU8 = AtomicU8::new(0);

/// Läuft mbusio im Hintergrund, also weiter, wenn das Fenster zu ist? Windows: als Dienst
/// gestartet. Linux: siehe `unix_background`.
pub fn background() -> bool {
    match TEST_BACKGROUND.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }
    if AS_SERVICE.load(Ordering::Relaxed) {
        return true;
    }
    #[cfg(unix)]
    {
        let cgroup = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
        let detached = ["TMUX", "STY"]
            .iter()
            .any(|k| std::env::var_os(k).is_some());
        unix_background(std::os::unix::process::parent_id(), &cgroup, detached)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Linux: von init gestartet (Elternprozess 1), als systemd-Dienst (System oder Benutzer: die
/// cgroup endet auf `.service`; ein Terminal läuft in einer `.scope`) oder in tmux/screen.
/// `INVOCATION_ID` taugt nicht: das erbt auch jede Shell im Terminal.
#[cfg_attr(not(unix), allow(dead_code))]
fn unix_background(ppid: u32, cgroup: &str, tmux_or_screen: bool) -> bool {
    let unit = cgroup
        .lines()
        .last()
        .and_then(|l| l.rsplit('/').next())
        .unwrap_or("");
    ppid == 1 || unit.ends_with(".service") || tmux_or_screen
}

/// Tests: so tun, als liefe mbusio im Hintergrund (true) oder im Fenster (false)
#[cfg(test)]
pub fn set_background_for_tests(on: bool) {
    TEST_BACKGROUND.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

/// Befehlszeile, mit der der Dienst mbusio startet
pub fn service_args(
    data_dir: &Path,
    http_port: u16,
    lan: bool,
    token: Option<&str>,
) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["--service".into(), "--no-browser".into()];
    a.push("--data-dir".into());
    a.push(data_dir.into());
    if http_port != 8080 {
        a.push("--http-port".into());
        a.push(http_port.to_string().into());
    }
    if lan {
        a.push("--lan".into());
    }
    if let Some(t) = token.filter(|t| !t.is_empty()) {
        a.push("--token".into());
        a.push(t.into());
    }
    a
}

/// Daten für den Dienst übernehmen: Hat `to` noch keine Datenbank, wird der erste Ordner aus
/// `from` mit einer Datenbank hineinkopiert (das Original bleibt als Sicherung). Liefert den
/// Ordner, aus dem kopiert wurde; `None`, wenn `to` schon Daten hat oder es nichts gibt.
pub fn adopt_data(to: &Path, from: &[PathBuf]) -> Result<Option<PathBuf>> {
    std::fs::create_dir_all(to)?;
    if to.join(DB_FILE).exists() {
        return Ok(None);
    }
    let same = |d: &Path| match (d.canonicalize(), to.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    let Some(src) = from.iter().find(|d| d.join(DB_FILE).is_file() && !same(d)) else {
        return Ok(None);
    };
    copy_dir(src, to)?;
    Ok(Some(src.clone()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Start, Stopp und Fehler des Dienstes in `<Daten>/mbusio.log` (der Dienst hat kein Fenster).
/// Über 1 MB wird die Datei zu `mbusio.log.1`.
pub fn log(dir: &Path, msg: &str) {
    let path = dir.join(LOG_FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::rename(&path, dir.join(format!("{LOG_FILE}.1")));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{} {msg}", crate::clock::stamp());
    }
}

/// Solange ein Plan läuft, darf der Rechner nicht einschlafen (Windows). Der Bildschirm darf aus.
/// Muss aus einem Thread kommen, der weiterläuft (Zeitplan).
pub fn keep_awake(on: bool) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{
            SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
        };
        let flags = if on {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED
        } else {
            ES_CONTINUOUS
        };
        unsafe { SetThreadExecutionState(flags) };
    }
    #[cfg(not(windows))]
    let _ = on;
}

#[cfg(windows)]
pub use win::{dispatch, install, uninstall};

#[cfg(not(windows))]
fn windows_only() -> anyhow::Error {
    anyhow::anyhow!(crate::i18n::tr(
        "Nur unter Windows. Unter Linux mbusio als systemd-Dienst einrichten (README, „Raspberry Pi als Dienst“).",
        "Windows only. On Linux set mbusio up as a systemd service (README, “Raspberry Pi as a service”)."
    ))
}

#[cfg(not(windows))]
pub fn dispatch() -> Result<()> {
    Err(windows_only())
}

#[cfg(not(windows))]
pub fn install(_: &crate::Args) -> Result<()> {
    Err(windows_only())
}

#[cfg(not(windows))]
pub fn uninstall(_: &crate::Args) -> Result<()> {
    Err(windows_only())
}

#[cfg(windows)]
mod win;

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mbusio-svc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn adopts_data_once_and_keeps_the_original() {
        let (old, empty, to) = (tmp("old"), tmp("empty"), tmp("to"));
        std::fs::create_dir_all(old.join("sim")).unwrap();
        std::fs::write(old.join(DB_FILE), b"db").unwrap();
        std::fs::write(old.join("sim/a.json"), b"{}").unwrap();
        std::fs::create_dir_all(&empty).unwrap();
        // erster Ordner ohne Datenbank wird übersprungen
        let from = [empty.clone(), old.clone()];
        assert_eq!(adopt_data(&to, &from).unwrap(), Some(old.clone()));
        assert_eq!(std::fs::read(to.join(DB_FILE)).unwrap(), b"db");
        assert!(to.join("sim/a.json").is_file());
        assert!(old.join(DB_FILE).is_file(), "Original bleibt");
        // Update: Ziel hat schon Daten, nichts wird überschrieben
        std::fs::write(old.join(DB_FILE), b"neuer").unwrap();
        assert_eq!(adopt_data(&to, &from).unwrap(), None);
        assert_eq!(std::fs::read(to.join(DB_FILE)).unwrap(), b"db");
        for d in [old, empty, to] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn background_on_linux() {
        let term = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.gnome.Terminal.slice/vte-spawn-1.scope";
        let ssh = "0::/user.slice/user-1000.slice/session-3.scope";
        let system = "0::/system.slice/mbusio.service";
        let user = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/mbusio.service";
        assert!(!unix_background(4242, term, false), "Terminal");
        assert!(!unix_background(4242, ssh, false), "SSH");
        assert!(unix_background(4242, system, false), "systemd-Dienst");
        assert!(unix_background(4242, user, false), "systemd-Benutzerdienst");
        assert!(unix_background(4242, ssh, true), "tmux/screen");
        assert!(unix_background(1, "", false), "von init");
    }

    #[test]
    fn service_command_line() {
        let a = |port, lan, token| -> Vec<String> {
            service_args(Path::new(r"C:\ProgramData\mbusio"), port, lan, token)
                .into_iter()
                .map(|s| s.into_string().unwrap())
                .collect()
        };
        assert_eq!(
            a(8080, false, None),
            [
                "--service",
                "--no-browser",
                "--data-dir",
                r"C:\ProgramData\mbusio"
            ]
        );
        let full = a(8090, true, Some("geheim"));
        assert!(full
            .ends_with(&["--http-port", "8090", "--lan", "--token", "geheim"].map(String::from)));
        assert!(!a(8080, true, Some("")).contains(&"--token".to_string()));
    }

    #[test]
    fn log_rolls_over() {
        let d = tmp("log");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(LOG_FILE), vec![b'x'; LOG_MAX as usize + 1]).unwrap();
        log(&d, "Start");
        assert!(d.join(format!("{LOG_FILE}.1")).is_file());
        assert!(std::fs::read_to_string(d.join(LOG_FILE))
            .unwrap()
            .contains("Start"));
        std::fs::remove_dir_all(d).unwrap();
    }
}
