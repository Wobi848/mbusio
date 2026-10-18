mod changeset;
mod clock;
mod datapoint;
mod db;
mod export;
mod i18n;
mod line;
mod link;
mod mbus;
mod meterlist;
mod mqtt;
mod plan;
mod scheduler;
mod sim;
mod storage;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};

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
    /// Zeitraffer für Dauertests: Uhr läuft so viel schneller (z.B. 96 = ein Tag in 15 min)
    #[arg(long, hide = true)]
    time_scale: Option<f64>,
    /// Befehle ohne Weboberfläche: Ergebnis als JSON auf stdout
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Primärscan
    Scan {
        #[arg(long, default_value_t = 0)]
        from: u8,
        #[arg(long, default_value_t = 250)]
        to: u8,
        #[arg(long, default_value_t = 1)]
        retries: u32,
        /// Auf allen Baudraten scannen, auf denen Geräte antworten
        #[arg(long)]
        all_bauds: bool,
        /// Auf diesen Baudraten scannen, z.B. 2400,9600
        #[arg(long, value_delimiter = ',')]
        bauds: Vec<u32>,
    },
    /// Sekundärscan (Wildcard-Suche über die ID)
    ScanSecondary {
        /// Auf allen Baudraten scannen, auf denen Geräte antworten
        #[arg(long)]
        all_bauds: bool,
        /// Auf diesen Baudraten scannen, z.B. 2400,9600
        #[arg(long, value_delimiter = ',')]
        bauds: Vec<u32>,
    },
    /// Prüfen, auf welchen Baudraten Geräte am Bus antworten
    BusBauds,
    /// Zähler auslesen: Primäradresse, ID oder Sekundäradresse
    Read { address: String },
    /// Primäradresse setzen
    SetAddress { address: String, new_address: u8 },
    /// Baudrate am Zähler umstellen
    SwitchBaud { address: String, baud: u32 },
    /// Baudrate eines Zählers suchen
    DetectBaud { address: String },
    /// Application Reset senden
    Reset {
        address: String,
        #[arg(long)]
        subcode: Option<u8>,
    },
    /// Rohe Bytes senden, z.B. "10 40 01 41 16"
    Raw { hex: String },
}

fn choice(all_bauds: bool, bauds: &[u32]) -> Bauds {
    match (all_bauds, bauds.is_empty()) {
        (true, _) => Bauds::Probe,
        (false, true) => Bauds::Current,
        (false, false) => Bauds::List(bauds.to_vec()),
    }
}

/// Treffer sammeln, Fortschritt auf stderr
fn collect(hits: &mut Vec<ScanHit>) -> impl FnMut(Progress) -> bool + '_ {
    |p| {
        match p {
            Progress::Hit(h) => hits.push(h),
            Progress::Step { message, .. } => eprintln!("{message}"),
            Progress::Stopped { .. } => {}
        }
        true
    }
}

fn run_command(args: &Args, cmd: &Cmd) -> anyhow::Result<()> {
    let Some(port) = &args.port else {
        bail!("--port fehlt / missing");
    };
    let log = Arc::new(FrameLog::default());
    let link = link::open(port, args.baud)?;
    let mut m = Master::new(link, args.baud, args.timeout_ms, log.clone());
    let target = |a: &str| Target::parse(a);
    let out = match cmd {
        Cmd::Scan {
            from,
            to,
            retries,
            all_bauds,
            bauds,
        } => {
            let mut hits = Vec::new();
            m.scan_with(
                choice(*all_bauds, bauds),
                &mut collect(&mut hits),
                &mut |m, p| m.scan_primary(*from, *to, *retries, p),
            )?;
            serde_json::to_value(hits)?
        }
        Cmd::ScanSecondary { all_bauds, bauds } => {
            let mut hits = Vec::new();
            m.scan_with(
                choice(*all_bauds, bauds),
                &mut collect(&mut hits),
                &mut |m, p| m.scan_secondary(p),
            )?;
            serde_json::to_value(hits)?
        }
        Cmd::BusBauds => serde_json::json!({ "bauds": m.bus_bauds(&[])? }),
        Cmd::Read { address } => serde_json::to_value(m.read_meter(&target(address)?)?)?,
        Cmd::SetAddress {
            address,
            new_address,
        } => {
            m.set_primary(&target(address)?, *new_address)?;
            serde_json::json!({ "ok": true })
        }
        Cmd::SwitchBaud { address, baud } => {
            m.switch_baud(&target(address)?, *baud)?;
            serde_json::json!({ "ok": true })
        }
        Cmd::DetectBaud { address } => {
            serde_json::json!({ "bauds": m.detect_baud(&target(address)?)? })
        }
        Cmd::Reset { address, subcode } => {
            m.application_reset(&target(address)?, *subcode)?;
            serde_json::json!({ "ok": true })
        }
        Cmd::Raw { hex } => {
            let bytes = web::parse_hex(hex)?;
            match m.raw_send(&bytes)? {
                Reply::None => serde_json::json!({ "kind": "none" }),
                Reply::Collision(b) => {
                    serde_json::json!({ "kind": "collision", "hex": mbus::frame::hex(&b) })
                }
                Reply::Frame(f) => {
                    serde_json::json!({ "kind": "frame", "hex": mbus::frame::hex(&f.encode()) })
                }
            }
        }
    };
    // Telegramm-Log für die Fehlersuche auf stderr, Ergebnis auf stdout
    for e in log.since(0) {
        eprintln!("{} {:<4} {}", e.time, e.dir, e.text);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
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

/// Beim Doppelklick unter Windows schliesst sich das Fenster sofort; Fehler sonst unsichtbar.
fn wait_for_enter() {
    if cfg!(windows) {
        println!("Enter = Ende / quit");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.list_ports || args.cmd.is_some() {
        return run_cli(&args);
    }
    if let Err(e) = serve(&args).await {
        eprintln!("Fehler / error: {e:#}");
        wait_for_enter();
        std::process::exit(1);
    }
    Ok(())
}

fn run_cli(args: &Args) -> anyhow::Result<()> {
    if args.list_ports {
        for p in link::list_ports() {
            println!("{p}");
        }
        return Ok(());
    }
    match &args.cmd {
        Some(cmd) => run_command(args, cmd),
        None => Ok(()),
    }
}

async fn serve(args: &Args) -> anyhow::Result<()> {
    let data_dir = args.data_dir.clone().unwrap_or_else(db::default_dir);
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
    let listener = match tokio::net::TcpListener::bind((host, args.http_port)).await {
        Ok(l) => l,
        // Port belegt: meist läuft mbusio schon (zweiter Doppelklick), dann nur den Browser öffnen
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && !args.no_browser => {
            let url = format!("http://localhost:{}", args.http_port);
            println!(
                "Port {} belegt, mbusio läuft vermutlich schon / already running: {url}",
                args.http_port
            );
            open_browser(&url);
            return Ok(());
        }
        Err(e) => {
            return Err(e).with_context(|| format!("Cannot listen on {host}:{}", args.http_port))
        }
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
    let port = listener.local_addr()?.port();
    let url = format!("http://localhost:{port}");
    println!("mbusio {}: {url}", env!("CARGO_PKG_VERSION"));
    println!("Daten / data: {}", data_dir.join("mbusio.db").display());
    if args.lan {
        println!("LAN: http://<IP>:{port}");
        if args.token.is_none() {
            println!("Hinweis/Note: --token empfohlen / recommended");
        }
    }
    println!("Ctrl+C = Ende / quit");
    if !args.no_browser {
        open_browser(&url);
    }

    axum::serve(listener, web::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
