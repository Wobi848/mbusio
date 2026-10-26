//! Zeitplan: liest Zähler nach ihrem Ausleseplan aus (Konzept Dauerbetrieb, 4.3 und 4.4).
//!
//! Alle paar Sekunden wird geprüft, welche Zähler fällig sind. Fällige Zähler gehen einzeln als
//! geplante Aufträge an den Linien-Dienst ihrer Linie; dort kommt die Bedienung immer zuerst dran,
//! zwischen zwei Zählern. Der Zustand je Zähler (letzter Termin, Fehler, ausgesetzt) steht in der
//! Datenbank, damit nach einem Neustart nichts doppelt und nichts verloren geht.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::json;

use crate::clock;
use crate::db::{Meter, PlanOutcome, DISTURBED_AFTER};
use crate::i18n::{self, both, tr, Text};
use crate::line::read_listed;
use crate::line::{is_link_error, Work};
use crate::mbus::master::{Master, MeterReading, MeterRefused, Target};
use crate::plan::{self, Due, Plan, SLOT_FORMAT};
use crate::storage::{self, DiskState, Level};
use crate::web::AppState;

mod reading;
mod upkeep;

use self::reading::*;
use self::upkeep::*;

/// So oft wird geprüft, ob etwas fällig ist
const TICK: Duration = Duration::from_secs(10);
/// So oft werden Zeitquelle und freier Speicher geprüft
const CLOCK_CHECK: Duration = Duration::from_secs(60);

/// Fortschritt eines Termins auf einer Linie (für die Statusleiste)
#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub line_id: i64,
    pub label: String,
    pub total: u32,
    pub done: u32,
    /// gelesen
    pub ok: u32,
    /// versucht (ohne die, bei denen die Linie nicht verbunden war)
    pub attempted: u32,
    pub started: String,
    #[serde(skip)]
    last_error: Option<String>,
}

pub struct Planner {
    /// Zähler, die schon in einer Warteschlange stehen (nie doppelt)
    queued: Mutex<HashSet<i64>>,
    /// laufende Termine je Linie
    runs: Mutex<HashMap<i64, Run>>,
    /// von Hand getrennte Linien: Plan pausiert, bis wieder verbunden wird
    held: Mutex<HashSet<i64>>,
    /// Linien mit eingeschaltetem Plan (Stand der letzten Prüfung)
    planned: Mutex<HashSet<i64>>,
    /// letzter Termin je Linie ohne Verbindung schon gespeichert (nicht alle 10 s neu schreiben)
    offline_saved: Mutex<HashSet<i64>>,
    /// Uhr; in Tests verstellbar
    now: Box<dyn Fn() -> NaiveDateTime + Send + Sync>,
    /// zum Erkennen von Uhrsprüngen: Uhrzeit und Monotonuhr bei der letzten Prüfung
    last_tick: Mutex<Option<(NaiveDateTime, Instant)>>,
    last_clock_check: Mutex<Option<Instant>>,
    /// freier Speicher beim Datenordner
    disk: Mutex<DiskState>,
    /// wann zuletzt ausgedünnt wurde
    last_thin: Mutex<Option<NaiveDateTime>>,
}

impl Default for Planner {
    fn default() -> Self {
        Planner::with_clock(Box::new(clock::now))
    }
}

impl Planner {
    pub fn with_clock(now: Box<dyn Fn() -> NaiveDateTime + Send + Sync>) -> Self {
        Planner {
            queued: Mutex::default(),
            runs: Mutex::default(),
            held: Mutex::default(),
            planned: Mutex::default(),
            offline_saved: Mutex::default(),
            now,
            last_tick: Mutex::default(),
            last_clock_check: Mutex::default(),
            disk: Mutex::default(),
            last_thin: Mutex::default(),
        }
    }

    pub fn disk(&self) -> DiskState {
        self.disk.lock().unwrap().clone()
    }

    /// Laufende Termine, für die Statusleiste
    pub fn runs(&self) -> Vec<Run> {
        let mut v: Vec<Run> = self.runs.lock().unwrap().values().cloned().collect();
        v.sort_by_key(|r| r.line_id);
        v
    }

    pub fn is_planned(&self, line_id: i64) -> bool {
        self.planned.lock().unwrap().contains(&line_id)
    }

    pub fn planned_lines(&self) -> HashSet<i64> {
        self.planned.lock().unwrap().clone()
    }

    /// Ist irgendwo ein Plan eingeschaltet?
    pub fn any_planned(&self) -> bool {
        !self.planned.lock().unwrap().is_empty()
    }

    /// Linie von Hand getrennt: nicht von selbst wieder verbinden
    pub fn hold(&self, line_id: i64) {
        self.held.lock().unwrap().insert(line_id);
    }

    pub fn release(&self, line_id: i64) {
        self.held.lock().unwrap().remove(&line_id);
    }

    pub fn is_held(&self, line_id: i64) -> bool {
        self.held.lock().unwrap().contains(&line_id)
    }

    /// Nichts in den Warteschlangen (für Tests)
    #[cfg(test)]
    fn idle(&self) -> bool {
        self.queued.lock().unwrap().is_empty()
    }
}

/// Meldung ins Protokoll der Oberfläche und auf die Konsole (beim Dienst: ins Systemprotokoll)
fn note(st: &AppState, msg: impl Into<Text>) {
    let msg = msg.into();
    println!("{} {}", clock::stamp(), msg.get());
    st.log.push("INFO", msg);
}

/// Läuft im eigenen Thread, solange mbusio läuft
pub fn run(st: AppState) {
    loop {
        if let Err(e) = tick(&st) {
            note(
                &st,
                both(|| format!("{}: {e:#}", tr("Zeitplan", "Schedule"))),
            );
        }
        // solange ein Plan läuft, nicht einschlafen (Windows)
        crate::service::keep_awake(st.planner.any_planned());
        // im Zeitraffer öfter prüfen, sonst würden Termine übersprungen
        std::thread::sleep(TICK.div_f64(clock::scale()).max(Duration::from_millis(500)));
    }
}

/// Einmal prüfen, was fällig ist, und es in Auftrag geben
pub fn tick(st: &AppState) -> anyhow::Result<()> {
    let now = (st.planner.now)();
    check_clock(st, now);
    thin(st, now)?;
    let lines = st.db.lock().unwrap().planned()?;
    let mut planned = HashSet::new();
    for pl in lines {
        // nur Linien im Dauerbetrieb (Konzept Bedienung §13); Plan allein liest nicht
        if !pl.line.operation {
            continue;
        }
        let mut due = Vec::new();
        let mut changes = Vec::new();
        for (m, p) in &pl.meters {
            let state = plan::State {
                key: m.plan_key.clone(),
                slot: m.plan_slot.as_deref().and_then(plan::parse_time),
                pause: m.plan_pause.as_deref().and_then(plan::parse_time),
            };
            match plan::due(*p, &state, now) {
                Due::No => {}
                Due::Read(slot) => due.push((m.clone(), *p, slot)),
                d => changes.push((m.id, *p, d)),
            }
        }
        // Zustand der ganzen Linie in einem Schreibvorgang (schont die SD-Karte)
        if !changes.is_empty() {
            st.db.lock().unwrap().tx(|db| {
                for (id, p, d) in &changes {
                    match d {
                        Due::Reset => db.plan_reset(*id)?,
                        Due::Start(slot) => {
                            db.plan_start(*id, &p.key(), &slot.format(SLOT_FORMAT).to_string())?
                        }
                        _ => {}
                    }
                }
                Ok(())
            })?;
        }
        if pl.meters.iter().all(|(_, p)| *p == Plan::Off) {
            continue;
        }
        planned.insert(pl.line.id);
        if due.is_empty() || pl.line.connection.is_empty() || st.planner.is_held(pl.line.id) {
            continue;
        }
        let Some(svc) = st.ensure_service(&pl.line) else {
            continue;
        };
        for (m, p, slot) in due {
            if !st.planner.queued.lock().unwrap().insert(m.id) {
                continue;
            }
            st.planner
                .runs
                .lock()
                .unwrap()
                .entry(pl.line.id)
                .or_insert_with(|| Run {
                    line_id: pl.line.id,
                    label: pl.label.clone(),
                    total: 0,
                    done: 0,
                    ok: 0,
                    attempted: 0,
                    started: clock::stamp(),
                    last_error: None,
                })
                .total += 1;
            svc.submit_planned(work(st.clone(), pl.line.id, m, p, slot));
        }
    }
    *st.planner.planned.lock().unwrap() = planned;
    st.stop_unused_services();
    Ok(())
}

#[cfg(test)]
mod tests;
