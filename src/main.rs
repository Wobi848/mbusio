mod export;
mod i18n;
mod link;
mod mbus;
mod project;
mod sim;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};

use mbus::master::{FrameLog, Master, Progress, Reply, Target};

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
    },
    /// Sekundärscan (Wildcard-Suche über die ID)
    ScanSecondary,
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

fn run_command(args: &Args, cmd: &Cmd) -> anyhow::Result<()> {
    let Some(port) = &args.port else {
        bail!("--port fehlt / missing");
    };
    let log = Arc::new(FrameLog::default());
    let link = link::open(port, args.baud)?;
    let mut m = Master::new(link, args.baud, args.timeout_ms, log.clone());
    let target = |a: &str| Target::parse(a);
    let out = match cmd {
        Cmd::Scan { from, to, retries } => {
            let mut hits = Vec::new();
            m.scan_primary(*from, *to, *retries, &mut |p| {
                if let Progress::Hit(h) = p {
                    hits.push(h);
                }
                true
            })?;
            serde_json::to_value(hits)?
        }
        Cmd::ScanSecondary => {
            let mut hits = Vec::new();
            m.scan_secondary(&mut |p| {
                if let Progress::Hit(h) = p {
                    hits.push(h);
                }
                true
            })?;
            serde_json::to_value(hits)?
        }
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.list_ports {
        for p in link::list_ports() {
            println!("{p}");
        }
        return Ok(());
    }
    if let Some(cmd) = &args.cmd {
        return run_command(&args, cmd);
    }

    let data_dir = args.data_dir.clone().unwrap_or_else(project::default_dir);
    let meters = project::Store::load(&data_dir)?;
    let state: web::AppState = Arc::new(web::Shared::new(
        args.token.clone().filter(|t| !t.is_empty()),
        meters,
    ));
    if let Some(port) = &args.port {
        web::connect_blocking(&state, port, args.baud, args.timeout_ms)?;
        println!("Verbunden mit {port} ({} Bd)", args.baud);
    }

    let host = if args.lan { "0.0.0.0" } else { "127.0.0.1" };
    let listener = tokio::net::TcpListener::bind((host, args.http_port))
        .await
        .with_context(|| format!("Cannot listen on {host}:{}", args.http_port))?;
    let port = listener.local_addr()?.port();
    println!(
        "mbusio {}: http://localhost:{port}",
        env!("CARGO_PKG_VERSION")
    );
    println!("Zählerliste / meter list: {}", data_dir.join("meters.json").display());
    if args.lan {
        println!("LAN: http://<IP>:{port}");
        if args.token.is_none() {
            println!("Hinweis/Note: --token empfohlen / recommended");
        }
    }
    println!("Ctrl+C = Ende / quit");

    axum::serve(listener, web::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
