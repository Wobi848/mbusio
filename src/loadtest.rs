//! Lasttest (Konzept Dauerbetrieb, Abnahme Kriterium 10): eine grosse Anlage mit Messwerten über
//! Jahre erzeugen, mbusio darauf starten und messen, wie schnell die Oberfläche antwortet. Gedacht
//! für den Raspberry Pi vor dem Einsatz: `mbusio lasttest`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chrono::{Duration as Days, Local, NaiveDateTime};

use crate::db::{Db, Line, Meter};
use crate::mbus::master::{FrameLog, Master, Target};

/// Wie der Speicherschutz: so viele Tage mit jeder Auslesung, davor eine je Tag
const FULL_DAYS: i64 = 365;
/// je Messung so oft wiederholen, gezeigt wird der mittlere Wert
const REPEAT: usize = 3;

pub struct Size {
    pub meters: u32,
    pub years: u32,
    /// Ausleseplan in Minuten
    pub minutes: u32,
}

/// Anlage erzeugen und messen. Ohne `dir` in einem eigenen Ordner, der danach gelöscht wird; mit
/// `dir` bleibt er zum Anschauen, darf aber noch keine Datenbank enthalten (nie echte Daten).
pub fn run(dir: Option<PathBuf>, size: Size) -> Result<()> {
    let temp = dir.is_none();
    let dir = dir.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("mbusio-lasttest-{}", std::process::id()))
    });
    if dir.join("mbusio.db").exists() {
        bail!(
            "{}: dort liegt schon eine Datenbank; der Lasttest braucht einen leeren Ordner",
            dir.display()
        );
    }
    let result =
        generate(&dir, &size).and_then(|(db, meter, project)| measure(db, meter, project, &size));
    if temp {
        std::fs::remove_dir_all(&dir).ok();
    }
    result
}

/// Linie mit `meters` Zählern und Auslesungen über `years` Jahre; liefert die Datenbank, einen
/// Zähler und das Projekt für die Messungen
fn generate(dir: &Path, size: &Size) -> Result<(Db, i64, i64)> {
    let started = Instant::now();
    let mut db = Db::open(dir)?;
    let active = db.active()?;
    db.update_line(
        active.line.id,
        Line {
            name: "Lasttest".into(),
            connection: "SIMULATOR".into(),
            ..active.line.clone()
        },
    )?;
    // ein echtes Telegramm vom Simulator, gespeichert wie im Dauerbetrieb (kompakt, nur roh)
    let link = crate::link::open("SIMULATOR", 2400)?;
    let mut master = Master::new(link, 2400, 60, Arc::new(FrameLog::default()));
    let reading = master.read_meter(&Target::Primary(1))?;
    let raw = reading.raw_bytes();
    let mut ids = Vec::new();
    for a in 1..=size.meters {
        let meter = Meter {
            address: a.to_string(),
            name: format!("Whg {a}"),
            ..Default::default()
        };
        ids.push(db.add_meter(active.line.id, meter)?.id);
    }
    let now = Local::now().naive_local();
    let times = times(now, size);
    println!(
        "Erzeuge {} Zähler × {} Auslesungen = {} …",
        size.meters,
        times.len(),
        size.meters as usize * times.len()
    );
    // in Stücken, damit der Speicher klein bleibt (Pi)
    for chunk in times.chunks(20_000 / ids.len().max(1) + 1) {
        let rows: Vec<(i64, String, i64)> = chunk
            .iter()
            .flat_map(|t| {
                let ts = t.format("%Y-%m-%d %H:%M:%S").to_string();
                let utc = t.and_utc().timestamp();
                ids.iter().map(move |id| (*id, ts.clone(), utc))
            })
            .collect();
        db.bulk_readings(active.line.id, "1", &raw, &rows)?;
    }
    // jeder Zähler einmal richtig gelesen: Zählerliste mit Wert wie im Betrieb
    for a in 1..=size.meters {
        db.record(
            active.line.id,
            &Target::Primary(a as u8),
            Ok(&reading),
            true,
        )?;
    }
    println!("erzeugt in {:.1} s", started.elapsed().as_secs_f64());
    Ok((db, ids[0], active.project.id))
}

/// Zeitpunkte der Auslesungen, älteste zuerst: im letzten Jahr nach Plan, davor eine je Tag
fn times(now: NaiveDateTime, size: &Size) -> Vec<NaiveDateTime> {
    let step = Days::minutes(size.minutes.max(1) as i64);
    let full_from = now - Days::days(FULL_DAYS);
    let start = now - Days::days(365 * size.years.max(1) as i64);
    let mut out = Vec::new();
    let mut t = start;
    while t < full_from {
        out.push(t);
        t += Days::days(1);
    }
    let mut t = full_from;
    while t <= now {
        out.push(t);
        t += step;
    }
    out
}

/// mbusio auf der erzeugten Datenbank starten und die Abläufe der Oberfläche messen
fn measure(db: Db, meter: i64, project: i64, size: &Size) -> Result<()> {
    let db_file = db.path().map(Path::to_path_buf);
    let state: crate::web::AppState = Arc::new(crate::web::Shared::new(None, db));
    let rt = tokio::runtime::Runtime::new()?;
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))?;
    let port = listener.local_addr()?.port();
    rt.spawn(async move { axum::serve(listener, crate::web::router(state)).await });

    let checks: [(&str, String); 11] = [
        ("Status (alle 5 s)", "status".into()),
        ("Projektbaum", "projects".into()),
        ("Zählerliste", "meters".into()),
        ("Datenpunkte", "datapoints?scope=project".into()),
        ("Werte eines Zählers", format!("meters/{meter}/values")),
        ("Verlauf eines Zählers", format!("meters/{meter}/history")),
        ("Bericht", "report".into()),
        (
            "Abrechnung (Vorschau)",
            format!("projects/{project}/billing"),
        ),
        ("Messwerte als CSV", "export?format=csv".into()),
        ("Projekt sichern", format!("projects/{project}/export")),
        ("Sicherung herunterladen", "backup".into()),
    ];
    println!();
    println!("{:<26} {:>9} {:>10}", "Ablauf", "Zeit", "Grösse");
    for (name, path) in &checks {
        let mut times = Vec::new();
        let mut bytes = 0;
        for _ in 0..REPEAT {
            let (status, n, t) = get(port, path)?;
            if status != 200 {
                bail!("{name}: HTTP {status}");
            }
            bytes = n;
            times.push(t);
        }
        times.sort();
        println!(
            "{name:<26} {:>7.0} ms {:>10}",
            times[REPEAT / 2].as_secs_f64() * 1000.0,
            human(bytes as u64)
        );
    }
    if let Some(file) = db_file {
        let size_db = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        println!();
        println!(
            "Datenbank: {} · {} Zähler, {} Jahre, Plan alle {} min",
            human(size_db),
            size.meters,
            size.years,
            size.minutes
        );
    }
    Ok(())
}

/// GET auf die eigene Schnittstelle: HTTP-Status, Grösse der Antwort, Dauer
fn get(port: u16, path: &str) -> Result<(u16, usize, Duration)> {
    let started = Instant::now();
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    write!(
        s,
        "GET /api/{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut answer = Vec::new();
    s.read_to_end(&mut answer)?;
    let took = started.elapsed();
    let status = std::str::from_utf8(answer.get(9..12).unwrap_or_default())
        .ok()
        .and_then(|c| c.parse().ok())
        .context("keine HTTP-Antwort")?;
    let body = answer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(0, |i| answer.len() - i - 4);
    Ok((status, body, took))
}

fn human(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.0} KB", b as f64 / (1 << 10) as f64),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_load_test_runs_and_never_touches_an_existing_database() {
        let dir = std::env::temp_dir().join(format!("mbusio-lasttest-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let size = || Size {
            meters: 3,
            years: 2,
            minutes: 60,
        };
        run(Some(dir.clone()), size()).unwrap();
        // zweiter Lauf im selben Ordner: dort liegt jetzt eine Datenbank, nichts überschreiben
        assert!(run(Some(dir.clone()), size()).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
