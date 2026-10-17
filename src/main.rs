mod export;
mod i18n;
mod link;
mod mbus;
mod sim;
mod web;

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;

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
    /// Beim Start direkt verbinden, z.B. /dev/ttyUSB0, COM3 oder SIMULATOR
    #[arg(long)]
    port: Option<String>,
    /// Baudrate
    #[arg(long, default_value_t = 2400)]
    baud: u32,
    /// Antwort-Timeout in ms (0 = automatisch aus Baudrate)
    #[arg(long, default_value_t = 0)]
    timeout_ms: u64,
    /// Ports auflisten und beenden
    #[arg(long)]
    list_ports: bool,
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

    let state: web::AppState = Arc::new(web::Shared::new(args.token.clone().filter(|t| !t.is_empty())));
    if let Some(port) = &args.port {
        web::connect_blocking(&state, port, args.baud, args.timeout_ms)?;
        println!("Verbunden mit {port} ({} Bd)", args.baud);
    }

    let host = if args.lan { "0.0.0.0" } else { "127.0.0.1" };
    let listener = tokio::net::TcpListener::bind((host, args.http_port))
        .await
        .with_context(|| format!("Cannot listen on {host}:{}", args.http_port))?;
    let port = listener.local_addr()?.port();
    println!("mbusio {}: http://localhost:{port}", env!("CARGO_PKG_VERSION"));
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
