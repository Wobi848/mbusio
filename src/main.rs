mod backup;
mod billing;
mod changeset;
mod cli;
mod clock;
mod datapoint;
mod db;
mod export;
mod i18n;
mod line;
mod link;
mod mail;
mod mbus;
mod meterlist;
mod modbus;
mod mqtt;
mod plan;
mod report;
mod scheduler;
mod service;
mod sim;
mod storage;
#[cfg(test)]
mod testutil;
mod textproto;
mod tls;
mod warn;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;

use cli::{run_cli, Cmd};
use mbus::master::{Bauds, FrameLog, Master, Progress, Reply, ScanHit, Target};

/// M-Bus Master mit Weboberfläche
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Port des Webservers
    #[arg(long, default_value_t = 8080)]
    http_port: u16,
    /// Auch aus dem LAN erreichbar machen (bindet an 0.0.0.0 statt 127.0.0.1)
    #[arg(long)]
    lan: bool,
    /// Zugriffsschlüssel für die API; empfohlen zusammen mit --lan
    #[arg(long, env = "MBUSIO_TOKEN")]
    token: Option<String>,
    /// Verbinden mit /dev/ttyUSB0, COM3, tcp://192.168.1.50:10001 oder SIMULATOR
    #[arg(long, global = true)]
    port: Option<String>,
    /// Baudrate
    #[arg(long, default_value_t = 2400, global = true)]
    baud: u32,
    /// Antwort-Timeout in ms (0 = automatisch aus Baudrate)
    #[arg(long, default_value_t = 0, global = true)]
    timeout_ms: u64,
    /// Ordner für die Zählerliste (meters.json)
    #[arg(long, env = "MBUSIO_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Ports auflisten und beenden
    #[arg(long)]
    list_ports: bool,
    /// Browser beim Start nicht öffnen
    #[arg(long)]
    no_browser: bool,
    /// Systemuhr nie als bestätigt ansehen (Auslesungen gelten als „Zeit unsicher“, bis die
    /// Uhrzeit im Browser übernommen wird)
    #[arg(long)]
    untrusted_clock: bool,
    /// Als Windows-Dienst einrichten oder aktualisieren (als Administrator; mit --lan, --token)
    #[arg(long)]
    install_service: bool,
    /// Windows-Dienst entfernen (Programm und Daten bleiben)
    #[arg(long)]
    uninstall_service: bool,
    /// vom Windows-Dienst-Manager gestartet
    #[arg(long, hide = true)]
    service: bool,
    /// Zeitraffer für Dauertests: Uhr läuft so viel schneller (z.B. 96 = ein Tag in 15 min)
    #[arg(long, hide = true)]
    time_scale: Option<f64>,
    /// Befehle ohne Weboberfläche: Ergebnis als JSON auf stdout
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

/// Öffnet die Weboberfläche im Standardbrowser. Auf Linux nur mit grafischer Oberfläche,
/// damit ein Raspberry Pi als Dienst nichts zu starten versucht.
fn open_browser(url: &str) {
    let result = if cfg!(windows) {
        // "start" ist ein Befehl von cmd; der leere Titel verhindert, dass die URL als Titel gilt
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        std::process::Command::new("xdg-open").arg(url).spawn()
    } else {
        return;
    };
    if result.is_err() {
        println!("Browser: {url}");
    }
}

/// Adresse der Oberfläche auf diesem Gerät. IPv4 statt `localhost`: mbusio hört nur auf IPv4, und
/// Windows versucht bei `localhost` zuerst IPv6 und braucht für die Abweisung spürbar lange.
pub fn local_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// Hat mbusio sich selbst neu gestartet (z.B. nach dem Einspielen einer Sicherung)?
fn restarted() -> bool {
    std::env::var_os(service::RESTARTED).is_some()
}

/// Wie viele Ports nach dem gewünschten probiert werden, wenn Windows ihn sperrt
const PORT_TRIES: u16 = 10;

/// Port für die Oberfläche belegen. Sperrt Windows ihn (reservierter Bereich von Hyper-V, WSL oder
/// Docker: os error 10013), den nächsten nehmen. Belegt heisst meist, dass mbusio schon läuft
/// (zweiter Doppelklick): dann nur den Browser öffnen und `None` liefern.
async fn listen(host: &str, port: u16, no_browser: bool) -> anyhow::Result<Option<TcpListener>> {
    let mut p = port;
    // selbst neu gestartet: der alte Prozess gibt den Port gleich frei
    let mut wait = if restarted() { 50 } else { 0 };
    loop {
        match TcpListener::bind((host, p)).await {
            Ok(l) => return Ok(Some(l)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && wait > 0 => {
                wait -= 1;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && !no_browser => {
                let url = local_url(p);
                println!("Port {p} belegt, mbusio läuft vermutlich schon / already running: {url}");
                open_browser(&url);
                return Ok(None);
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && p < port + PORT_TRIES => {
                println!(
                    "Port {p} gesperrt, versuche {} / port {p} not allowed, trying {}",
                    p + 1,
                    p + 1
                );
                p += 1;
            }
            Err(e) => return Err(e).with_context(|| format!("Cannot listen on {host}:{p}")),
        }
    }
}

/// Beim Doppelklick unter Windows schliesst sich das Fenster sofort; Fehler sonst unsichtbar.
fn wait_for_enter() {
    if cfg!(windows) {
        println!("Enter = Ende / quit");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.service {
        return service::dispatch();
    }
    if args.install_service || args.uninstall_service {
        let r = if args.install_service {
            service::install(&args)
        } else {
            service::uninstall(&args)
        };
        if let Err(e) = &r {
            eprintln!("Fehler / error: {e:#}");
        }
        wait_for_enter();
        std::process::exit(r.is_err() as i32);
    }
    if args.list_ports || args.cmd.is_some() {
        return run_cli(&args);
    }
    let rt = tokio::runtime::Runtime::new()?;
    if let Err(e) = rt.block_on(serve(&args, stop_signal())) {
        eprintln!("Fehler / error: {e:#}");
        wait_for_enter();
        std::process::exit(1);
    }
    Ok(())
}

/// Ende im Fenster (Ctrl+C) oder unter systemd (SIGTERM)
async fn stop_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// Weboberfläche, Zeitplan und MQTT bis `stop` (Fenster, systemd oder Windows-Dienst)
async fn serve(
    args: &Args,
    stop: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let data_dir = args.data_dir.clone().unwrap_or_else(db::default_dir);
    if backup::restore_pending(&data_dir)? {
        println!("Sicherung eingespielt / backup restored (vorher: mbusio.db.vorher)");
    }
    let database = db::Db::open(&data_dir)?;
    let active = database.active()?;
    let state: web::AppState = Arc::new(web::Shared::new(
        args.token.clone().filter(|t| !t.is_empty()),
        database,
    ));
    if let Some(port) = &args.port {
        // --port gilt nur für diesen Start und wird nicht bei der Linie gespeichert
        let conf = line::LineConf {
            connection: port.clone(),
            baud: args.baud,
            timeout_ms: args.timeout_ms,
        };
        state.connect_line(active.line.id, conf)?;
        println!("Verbunden mit {port} ({} Bd)", args.baud);
    } else if !active.line.connection.is_empty() {
        // zuletzt benutzte Linie gleich wieder verbinden; klappt das nicht, geht es trotzdem weiter
        let l = &active.line;
        let conf = line::LineConf::of(l);
        match state.connect_line(l.id, conf) {
            Ok(()) => println!("Verbunden mit {} ({} Bd)", l.connection, l.baud),
            Err(e) => println!("{}: {e:#}", l.connection),
        }
    }
    println!(
        "Projekt / project: {} · {} · {}",
        active.project.name, active.gateway.name, active.line.name
    );

    let host = if args.lan { "0.0.0.0" } else { "127.0.0.1" };
    let Some(listener) = listen(host, args.http_port, args.no_browser).await? else {
        return Ok(());
    };
    // Zeitplan erst, wenn sicher nur dieses mbusio läuft
    if args.untrusted_clock {
        clock::distrust_system();
    }
    if let Some(f) = args.time_scale.filter(|f| *f > 1.0) {
        clock::set_scale(f);
        println!("Zeitraffer / time scale: {f}×");
    }
    let st = state.clone();
    std::thread::Builder::new()
        .name("plan".into())
        .spawn(move || scheduler::run(st))?;
    let st = state.clone();
    std::thread::Builder::new()
        .name("mqtt".into())
        .spawn(move || mqtt::run(st))?;
    tokio::spawn(modbus::run(state.clone()));
    let port = listener.local_addr()?.port();
    let url = local_url(port);
    println!("mbusio {}: {url}", env!("CARGO_PKG_VERSION"));
    println!("Daten / data: {}", data_dir.join("mbusio.db").display());
    if args.lan {
        println!("LAN: http://<IP>:{port}");
        if args.token.is_none() {
            println!("Hinweis/Note: --token empfohlen / recommended");
        }
    }
    println!("Ctrl+C = Ende / quit");
    // nach einem Neustart ist die Oberfläche schon offen und lädt von selbst
    if !args.no_browser && !restarted() {
        open_browser(&url);
    }

    let st = state.clone();
    let stop = async move {
        stop.await;
        st.stop.send_replace(true);
    };
    axum::serve(listener, web::router(state))
        .with_graceful_shutdown(stop)
        .await?;
    Ok(())
}
