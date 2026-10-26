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
mod tests {
    use super::*;
    use crate::db::{Db, Line, Meter};
    use crate::web::Shared;
    use std::sync::Arc;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    struct Rig {
        st: AppState,
        time: Arc<Mutex<NaiveDateTime>>,
        line: i64,
    }

    impl Rig {
        fn new(addresses: &[&str]) -> Rig {
            let mut db = Db::memory().unwrap();
            let line = db.active_line().unwrap();
            db.update_line(
                line.id,
                Line {
                    connection: "SIMULATOR".into(),
                    baud: 2400,
                    timeout_ms: 60,
                    ..line.clone()
                },
            )
            .unwrap();
            db.operation_on(line.id);
            for a in addresses {
                db.add_meter(
                    line.id,
                    Meter {
                        address: a.to_string(),
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            let time = Arc::new(Mutex::new(at("2026-10-17 10:30")));
            let t = time.clone();
            let planner = Planner::with_clock(Box::new(move || *t.lock().unwrap()));
            let st = Arc::new(Shared::with_planner(None, db, planner));
            Rig {
                st,
                time,
                line: line.id,
            }
        }

        fn set_time(&self, s: &str) {
            *self.time.lock().unwrap() = at(s);
        }

        /// prüfen und warten, bis alle Aufträge erledigt sind
        fn tick(&self) {
            tick(&self.st).unwrap();
            let until = Instant::now() + Duration::from_secs(20);
            while !self.st.planner.idle() {
                assert!(Instant::now() < until, "Zeitplan hängt");
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn meter(&self, address: &str) -> Meter {
            let db = self.st.db.lock().unwrap();
            db.meters(self.line)
                .unwrap()
                .into_iter()
                .find(|m| m.address == address)
                .unwrap()
        }

        fn readings(&self, address: &str) -> usize {
            let id = self.meter(address).id;
            self.st
                .db
                .lock()
                .unwrap()
                .meter_history(id, 1000)
                .unwrap()
                .len()
        }
    }

    #[test]
    fn reads_on_schedule_and_handles_faulty_meters() {
        // 1, 2: gut · 4: falsche Prüfsumme · 11: beschäftigt (08)
        let r = Rig::new(&["1", "2", "4", "11"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "hourly")
            .unwrap();
        // eingeschaltet: erst ab dem nächsten Termin
        r.tick();
        assert_eq!(r.readings("1"), 0);
        assert!(r.st.planner.is_planned(r.line));
        r.set_time("2026-10-17 11:00");
        r.tick();
        assert_eq!((r.readings("1"), r.readings("2")), (1, 1));
        // im selben Termin nicht noch einmal
        r.set_time("2026-10-17 11:20");
        r.tick();
        assert_eq!(r.readings("1"), 1);
        let busy = r.meter("11");
        assert_eq!(busy.plan_pause.as_deref(), Some("2026-10-18 00:00:00"));
        assert_eq!(busy.plan_fails, 0);
        assert_eq!(r.meter("4").plan_fails, 1);
        // zwei weitere Termine: der defekte Zähler gilt als gestört, die anderen laufen weiter
        for t in ["2026-10-17 12:00", "2026-10-17 13:00"] {
            r.set_time(t);
            r.tick();
        }
        assert_eq!(r.meter("4").plan_fails, DISTURBED_AFTER);
        assert_eq!(r.readings("2"), 3);
        assert!(r.meter("11").last_error.is_some());
        // nächster Tag: der ausgesetzte Zähler wird wieder versucht
        r.set_time("2026-10-18 00:00");
        r.tick();
        assert_eq!(
            r.meter("11").plan_slot.as_deref(),
            Some("2026-10-18 00:00:00")
        );
        let result = r.st.db.lock().unwrap().tree().unwrap()[0].gateways[0].lines[0].plan_info
            ["result"]
            .clone();
        assert_eq!(result["ok"], 2);
        assert_eq!(result["total"], 4);
    }

    #[test]
    fn missed_terms_are_read_once_and_plan_off_forgets() {
        let r = Rig::new(&["1"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "daily 00:05")
            .unwrap();
        r.tick();
        // drei Tage aus: genau einmal nachlesen
        r.set_time("2026-10-20 09:00");
        r.tick();
        r.tick();
        assert_eq!(r.readings("1"), 1);
        assert_eq!(
            r.meter("1").plan_slot.as_deref(),
            Some("2026-10-20 00:05:00")
        );
        // Zähler selbst aus: Zustand weg, kein Nachholen beim Wiedereinschalten
        let id = r.meter("1").id;
        r.st.db
            .lock()
            .unwrap()
            .set_meter_plan(id, Some("off"))
            .unwrap();
        r.tick();
        assert!(r.meter("1").plan_key.is_none());
        assert!(!r.st.planner.is_planned(r.line));
        r.set_time("2026-10-22 09:00");
        r.st.db.lock().unwrap().set_meter_plan(id, None).unwrap();
        r.tick();
        assert_eq!(r.readings("1"), 1);
    }

    /// Abnahmekriterium 6: Zähler mit "zu viele Auslesungen" wird für den Tag ausgesetzt
    #[test]
    fn meter_with_quota_is_suspended_until_next_day() {
        let r = Rig::new(&["19"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "hourly")
            .unwrap();
        r.tick();
        for t in ["2026-10-17 11:00", "2026-10-17 12:00", "2026-10-17 13:00"] {
            r.set_time(t);
            r.tick();
        }
        // zwei Auslesungen gehen, die dritte lehnt der Zähler ab
        assert_eq!(r.readings("19"), 2);
        let m = r.meter("19");
        assert_eq!(m.plan_pause.as_deref(), Some("2026-10-18 00:00:00"));
        assert!(m.last_error.unwrap().contains("09"));
        assert_eq!(m.plan_fails, 0, "Ablehnung ist kein Fehler des Zählers");
        // bis Mitternacht wird er nicht mehr gefragt
        r.set_time("2026-10-17 14:00");
        r.tick();
        assert_eq!(
            r.meter("19").plan_slot.as_deref(),
            Some("2026-10-17 13:00:00")
        );
        r.set_time("2026-10-18 00:00");
        r.tick();
        assert_eq!(
            r.meter("19").plan_slot.as_deref(),
            Some("2026-10-18 00:00:00")
        );
    }

    /// Geplante Auslesungen werden kompakt gespeichert (Rohtelegramme) und beim Anzeigen
    /// decodiert, mit denselben Werten wie beim Auslesen
    #[test]
    fn planned_readings_are_stored_compact() {
        let r = Rig::new(&["1", "2"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "hourly")
            .unwrap();
        r.tick();
        r.set_time("2026-10-17 11:00");
        r.tick();
        let db = r.st.db.lock().unwrap();
        let (data, raw): (String, Vec<u8>) = db
            .conn_for_tests()
            .query_row(
                "SELECT data, raw FROM readings ORDER BY id LIMIT 1",
                [],
                |x| Ok((x.get(0)?, x.get(1)?)),
            )
            .unwrap();
        assert!(data.is_empty());
        assert!(raw.len() < 400, "Rohtelegramme {} Byte", raw.len());
        drop(db);
        let id = r.meter("1").id;
        let h = r.st.db.lock().unwrap().meter_history(id, 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].header.as_ref().unwrap().id, "12345678");
        assert!(h[0].utc.is_some());
        assert!(h[0].records.iter().any(|x| x.unit == "kWh"));
        assert_eq!(r.meter("1").last_value.as_deref(), Some("123456 kWh"));
    }

    /// Abnahmekriterium 11: Speicher voll, Werte werden gelesen, aber nicht mehr gespeichert
    #[test]
    fn full_storage_keeps_reading_without_storing() {
        let r = Rig::new(&["2"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "hourly")
            .unwrap();
        r.tick();
        r.st.db.lock().unwrap().readings_paused = true;
        r.set_time("2026-10-17 11:00");
        r.tick();
        assert_eq!(r.readings("2"), 0);
        let m = r.meter("2");
        assert!(m.last_value.is_some() && m.last_error.is_none());
        assert_eq!(m.plan_slot.as_deref(), Some("2026-10-17 11:00:00"));
    }

    /// Abnahmekriterium 8: Uhr springt zurück, kein doppelter Termin; danach geht es weiter
    #[test]
    fn clock_jump_back_realigns_without_duplicates() {
        let r = Rig::new(&["1"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "hourly")
            .unwrap();
        r.tick();
        r.set_time("2026-10-17 11:00");
        r.tick();
        assert_eq!(r.readings("1"), 1);
        // einen Tag zurück: neu ausrichten, nicht lesen
        r.set_time("2026-10-16 11:30");
        r.tick();
        assert_eq!(r.readings("1"), 1);
        assert_eq!(
            r.meter("1").plan_slot.as_deref(),
            Some("2026-10-16 11:00:00")
        );
        r.set_time("2026-10-16 12:00");
        r.tick();
        assert_eq!(r.readings("1"), 2);
    }

    /// Abnahmekriterium 10 (Teil Rechenzeit): 20 Linien mit zusammen 2000 Zählern; eine Prüfung
    /// muss schnell gehen, damit die Oberfläche bedienbar bleibt
    #[test]
    fn reference_scenario_tick_is_fast() {
        let mut db = Db::memory().unwrap();
        let gw = db.active().unwrap().gateway.id;
        for l in 0..20 {
            let line = db
                .create_line(
                    gw,
                    Line {
                        name: format!("L{l}"),
                        baud: 2400,
                        ..Default::default()
                    },
                )
                .unwrap();
            let meters = (0..100)
                .map(|k| Meter {
                    address: format!("{:08}", l * 1000 + k),
                    ..Default::default()
                })
                .collect();
            db.import_meters(line.id, meters).unwrap();
        }
        db.set_project_plan(1, "daily 00:05").unwrap();
        let st: AppState = Arc::new(Shared::with_planner(
            None,
            db,
            Planner::with_clock(Box::new(|| at("2026-10-17 10:30"))),
        ));
        let t0 = Instant::now();
        tick(&st).unwrap();
        let first = t0.elapsed();
        let t0 = Instant::now();
        tick(&st).unwrap();
        let again = t0.elapsed();
        let tree = Instant::now();
        st.db.lock().unwrap().tree().unwrap();
        let tree = tree.elapsed();
        println!("erste Prüfung {first:?}, weitere {again:?}, Projektbaum {tree:?}");
        assert!(
            first < Duration::from_secs(5)
                && again < Duration::from_secs(2)
                && tree < Duration::from_secs(2)
        );
    }

    #[test]
    fn held_line_is_not_read() {
        let r = Rig::new(&["1"]);
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "15min")
            .unwrap();
        r.tick();
        r.st.planner.hold(r.line);
        r.set_time("2026-10-17 10:45");
        r.tick();
        assert_eq!(r.readings("1"), 0);
        // wieder verbunden: der verpasste Termin wird nachgeholt
        r.st.planner.release(r.line);
        r.tick();
        assert_eq!(r.readings("1"), 1);
    }

    #[test]
    fn plan_alone_does_not_read_without_operation() {
        let r = Rig::new(&["1"]);
        r.st.db
            .lock()
            .unwrap()
            .set_operation(r.line, false)
            .unwrap();
        r.st.db
            .lock()
            .unwrap()
            .set_project_plan(1, "15min")
            .unwrap();
        r.tick();
        r.set_time("2026-10-17 10:45");
        r.tick();
        assert_eq!(r.readings("1"), 0, "Plan ohne Dauerbetrieb liest nicht");
        assert!(!r.st.planner.is_planned(r.line));
        // eingeschaltet: ab dem nächsten Termin, wie beim erstmaligen Setzen eines Plans
        r.st.db.lock().unwrap().set_operation(r.line, true).unwrap();
        r.tick();
        assert!(r.st.planner.is_planned(r.line));
        r.set_time("2026-10-17 11:00");
        r.tick();
        assert_eq!(r.readings("1"), 1);
    }
}
