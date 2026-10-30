//! Befehle der Kommandozeile ohne Weboberfläche (scan, read, …): Ergebnis als JSON auf stdout.

use super::*;

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
    /// Lasttest: grosse Anlage erzeugen und messen, wie schnell mbusio antwortet (z.B. auf dem
    /// Raspberry Pi vor dem Einsatz). Ohne --data-dir in einem eigenen Ordner, der danach gelöscht
    /// wird; nie in einem Ordner mit Datenbank
    Lasttest {
        #[arg(long, default_value_t = 60)]
        meters: u32,
        #[arg(long, default_value_t = 3)]
        years: u32,
        /// Ausleseplan in Minuten
        #[arg(long, default_value_t = 15)]
        minutes: u32,
    },
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
        // braucht keinen Bus; läuft schon in run_cli
        Cmd::Lasttest { .. } => unreachable!("Lasttest in run_cli"),
    };
    // Telegramm-Log für die Fehlersuche auf stderr, Ergebnis auf stdout
    for e in log.since(0) {
        eprintln!("{} {:<4} {}", e.time, e.dir, e.text);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// Befehle der Kommandozeile (oder Ports auflisten) statt der Weboberfläche
pub(crate) fn run_cli(args: &Args) -> anyhow::Result<()> {
    if args.list_ports {
        for p in link::list_ports() {
            if p.label.is_empty() {
                println!("{}", p.port);
            } else {
                println!("{:<16} {}", p.port, p.label);
            }
        }
        return Ok(());
    }
    match &args.cmd {
        Some(Cmd::Lasttest {
            meters,
            years,
            minutes,
        }) => crate::loadtest::run(
            args.data_dir.clone(),
            crate::loadtest::Size {
                meters: *meters,
                years: *years,
                minutes: *minutes,
            },
        ),
        Some(cmd) => run_command(args, cmd),
        None => Ok(()),
    }
}
