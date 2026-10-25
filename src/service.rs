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
mod win {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use anyhow::{bail, Context, Result};
    use clap::Parser;
    use windows_service::service::{
        ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept,
        ServiceErrorControl, ServiceExitCode, ServiceFailureActions, ServiceFailureResetPeriod,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_dispatcher};

    use super::{adopt_data, log, service_args, AS_SERVICE, NAME};
    use crate::i18n::tr;

    /// Konto des Dienstes: wenig Rechte, der Webserver ist im LAN erreichbar
    const ACCOUNT: &str = r"NT AUTHORITY\LocalService";
    /// SID von LocalService (für icacls, unabhängig von der Sprache von Windows)
    const ACCOUNT_SID: &str = "*S-1-5-19";

    fn env_dir(key: &str, fallback: &str) -> PathBuf {
        std::env::var_os(key)
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from(fallback), PathBuf::from)
    }

    /// Daten des Dienstes: für alle Benutzer, bleibt bei Updates
    fn data_dir() -> PathBuf {
        env_dir("ProgramData", r"C:\ProgramData").join("mbusio")
    }

    /// Programm des Dienstes: fester Ort, unabhängig davon, wo die heruntergeladene .exe liegt
    fn program() -> PathBuf {
        env_dir("ProgramFiles", r"C:\Program Files")
            .join("mbusio")
            .join("mbusio.exe")
    }

    /// Vom Dienst-Manager gestartet (`--service`): melden und `serve` bis zum Stopp laufen lassen
    pub fn dispatch() -> Result<()> {
        service_dispatcher::start(NAME, ffi_service_main)
            .context("service dispatcher (nur vom Windows-Dienst-Manager startbar)")
    }

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_: Vec<OsString>) {
        AS_SERVICE.store(true, Ordering::Relaxed);
        let args = crate::Args::parse();
        let dir = args.data_dir.clone().unwrap_or_else(data_dir);
        log(&dir, &format!("Start mbusio {}", env!("CARGO_PKG_VERSION")));
        match run(&args) {
            Ok(()) => log(&dir, "Stopp"),
            Err(e) => log(&dir, &format!("Fehler / error: {e:#}")),
        }
    }

    fn status(state: ServiceState, code: u32) -> ServiceStatus {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: if state == ServiceState::Running {
                ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
            } else {
                ServiceControlAccept::empty()
            },
            exit_code: ServiceExitCode::Win32(code),
            checkpoint: 0,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        }
    }

    fn run(args: &crate::Args) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = Mutex::new(Some(tx));
        let handle = service_control_handler::register(NAME, move |c| match c {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;
        handle.set_service_status(status(ServiceState::Running, 0))?;
        let result = tokio::runtime::Runtime::new()
            .map_err(anyhow::Error::from)
            .and_then(|rt| {
                rt.block_on(crate::serve(args, async {
                    let _ = rx.await;
                }))
            });
        // Fehler: Code ≠ 0, damit Windows den Dienst nach den Wiederherstellungs-Regeln neu startet
        let code = if result.is_ok() { 0 } else { 1 };
        handle.set_service_status(status(ServiceState::Stopped, code))?;
        result
    }

    fn manager() -> Result<ServiceManager> {
        ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )
        .map_err(|_| {
            anyhow::anyhow!(tr(
                "Adminrechte nötig: Eingabeaufforderung mit Rechtsklick „Als Administrator ausführen“ öffnen oder mbusio-dienst-einrichten.cmd doppelklicken.",
                "Administrator rights needed: open the command prompt with right click “Run as administrator” or double-click mbusio-dienst-einrichten.cmd."
            ))
        })
    }

    /// Wartet, bis der Dienst diesen Zustand hat (höchstens `max`)
    fn wait_for(
        service: &windows_service::service::Service,
        want: ServiceState,
        max: Duration,
    ) -> bool {
        let until = Instant::now() + max;
        while Instant::now() < until {
            if service
                .query_status()
                .is_ok_and(|s| s.current_state == want)
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        false
    }

    /// Laufenden Dienst anhalten (falls eingerichtet); true = er war eingerichtet
    fn stop_existing(m: &ServiceManager) -> Result<bool> {
        let Ok(s) = m.open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP) else {
            return Ok(false);
        };
        if s.query_status()?.current_state != ServiceState::Stopped {
            let _ = s.stop();
            if !wait_for(&s, ServiceState::Stopped, Duration::from_secs(30)) {
                bail!(tr(
                    "Der Dienst mbusio lässt sich nicht anhalten",
                    "The mbusio service does not stop"
                ));
            }
        }
        Ok(true)
    }

    fn run_quiet(cmd: &str, args: &[&str]) -> bool {
        std::process::Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn firewall(program: &Path, port: Option<u16>) {
        run_quiet(
            "netsh",
            &["advfirewall", "firewall", "delete", "rule", "name=mbusio"],
        );
        if let Some(port) = port {
            let ok = run_quiet(
                "netsh",
                &[
                    "advfirewall",
                    "firewall",
                    "add",
                    "rule",
                    "name=mbusio",
                    "dir=in",
                    "action=allow",
                    "protocol=TCP",
                    &format!("localport={port}"),
                    &format!("program={}", program.display()),
                ],
            );
            if !ok {
                println!(
                    "{}",
                    tr(
                        "Hinweis: Firewall-Regel nicht angelegt",
                        "Note: firewall rule not created"
                    )
                );
            }
        }
    }

    /// `--install-service`: einrichten oder aktualisieren (Konzept Hintergrund, 3.1)
    pub fn install(args: &crate::Args) -> Result<()> {
        let m = manager()?;
        let existed = stop_existing(&m)?;
        // läuft mbusio noch in einem Fenster, wäre die Datenbank offen und der Port belegt
        if std::net::TcpListener::bind(("127.0.0.1", args.http_port)).is_err() {
            bail!(tr(
                &format!("Port {} ist belegt: läuft mbusio noch in einem Fenster? Bitte schliessen und noch einmal.", args.http_port),
                &format!("Port {} is in use: is mbusio still open in a window? Close it and try again.", args.http_port),
            ));
        }
        // Programm an festen Ort
        let exe = std::env::current_exe()?;
        let target = program();
        let same = exe.canonicalize().ok() == target.canonicalize().ok();
        if !same {
            std::fs::create_dir_all(target.parent().unwrap())?;
            std::fs::copy(&exe, &target)
                .with_context(|| format!("{} → {}", exe.display(), target.display()))?;
        }
        // Daten übernehmen und dem Dienst-Konto Schreibrechte geben
        let dir = data_dir();
        let mut from: Vec<PathBuf> = args.data_dir.iter().cloned().collect();
        from.push(crate::db::default_dir());
        let copied = adopt_data(&dir, &from)?;
        let grant = format!("{ACCOUNT_SID}:(OI)(CI)M");
        if !run_quiet(
            "icacls",
            &[&dir.display().to_string(), "/grant", &grant, "/T", "/Q"],
        ) {
            println!(
                "{}",
                tr(
                    "Hinweis: Rechte auf den Datenordner nicht gesetzt",
                    "Note: could not set rights on the data folder"
                )
            );
        }
        // Dienst anlegen oder ändern
        let info = ServiceInfo {
            name: NAME.into(),
            display_name: "mbusio".into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: target.clone(),
            launch_arguments: service_args(&dir, args.http_port, args.lan, args.token.as_deref()),
            dependencies: vec![],
            account_name: Some(ACCOUNT.into()),
            account_password: None,
        };
        let access = ServiceAccess::QUERY_STATUS
            | ServiceAccess::START
            | ServiceAccess::STOP
            | ServiceAccess::CHANGE_CONFIG;
        let service = if existed {
            let s = m.open_service(NAME, access)?;
            s.change_config(&info)?;
            s
        } else {
            m.create_service(&info, access)?
        };
        service.set_description(tr(
            "M-Bus-Zähler regelmässig auslesen und Werte weitergeben (Weboberfläche)",
            "Read M-Bus meters regularly and pass on values (web interface)",
        ))?;
        // bei Absturz nach 10 s neu starten
        let restart = ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(10),
        };
        service.update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86400)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![restart.clone(), restart.clone(), restart]),
        })?;
        service.set_failure_actions_on_non_crash_failures(true)?;
        firewall(&target, args.lan.then_some(args.http_port));
        service.start::<&str>(&[])?;
        let running = wait_for(&service, ServiceState::Running, Duration::from_secs(20));
        let url = format!("http://localhost:{}", args.http_port);
        println!(
            "{}",
            tr(
                &format!(
                    "mbusio läuft als Dienst: {url}\n  Programm: {}\n  Daten:    {}",
                    target.display(),
                    dir.display()
                ),
                &format!(
                    "mbusio runs as a service: {url}\n  program: {}\n  data:    {}",
                    target.display(),
                    dir.display()
                ),
            )
        );
        if let Some(src) = copied {
            println!(
                "{}",
                tr(
                    &format!("  Daten übernommen aus {} (Original bleibt)", src.display()),
                    &format!("  data copied from {} (original kept)", src.display())
                )
            );
        }
        if !running {
            bail!(tr(
                &format!(
                    "Der Dienst ist eingerichtet, aber nicht gestartet. Siehe {}",
                    dir.join(super::LOG_FILE).display()
                ),
                &format!(
                    "The service is set up but did not start. See {}",
                    dir.join(super::LOG_FILE).display()
                ),
            ));
        }
        crate::open_browser(&url);
        Ok(())
    }

    /// `--uninstall-service`: Dienst anhalten und entfernen; Programm und Daten bleiben
    pub fn uninstall(_: &crate::Args) -> Result<()> {
        let m = manager()?;
        if !stop_existing(&m)? {
            println!(
                "{}",
                tr(
                    "Kein Dienst mbusio eingerichtet",
                    "No mbusio service set up"
                )
            );
            return Ok(());
        }
        m.open_service(NAME, ServiceAccess::DELETE)?.delete()?;
        firewall(&program(), None);
        println!(
            "{}",
            tr(
                &format!(
                    "Dienst mbusio entfernt. Bleiben liegen:\n  Programm: {}\n  Daten:    {}",
                    program().display(),
                    data_dir().display()
                ),
                &format!(
                    "mbusio service removed. Left in place:\n  program: {}\n  data:    {}",
                    program().display(),
                    data_dir().display()
                ),
            )
        );
        Ok(())
    }
}

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
