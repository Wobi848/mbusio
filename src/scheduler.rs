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
use crate::i18n::{self, tr, Lang};
use crate::line::{is_link_error, Work};
use crate::mbus::master::{Master, MeterReading, MeterRefused, Target};
use crate::plan::{self, Due, Plan, SLOT_FORMAT};
use crate::storage::{self, DiskState, Level};
use crate::web::{read_listed, AppState};

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
fn note(st: &AppState, msg: String) {
    println!("{} {msg}", clock::stamp());
    st.log.push("INFO", msg);
}

/// Läuft im eigenen Thread, solange mbusio läuft
pub fn run(st: AppState) {
    i18n::set(Lang::De);
    loop {
        if let Err(e) = tick(&st) {
            note(&st, format!("{}: {e:#}", tr("Zeitplan", "Schedule")));
        }
        // im Zeitraffer öfter prüfen, sonst würden Termine übersprungen
        std::thread::sleep(TICK.div_f64(clock::scale()).max(Duration::from_millis(500)));
    }
}

/// Freien Speicher prüfen; bei fast vollem Speicher keine Auslesungen mehr speichern
fn check_disk(st: &AppState) {
    let (dir, size) = {
        let db = st.db.lock().unwrap();
        (
            db.path().and_then(|p| p.parent()).map(|p| p.to_path_buf()),
            db.size(),
        )
    };
    let Some(dir) = dir else { return };
    let new = storage::check(&dir, size);
    let old = std::mem::replace(&mut *st.planner.disk.lock().unwrap(), new.clone());
    st.db.lock().unwrap().readings_paused = new.level == Level::Full;
    if new.level != old.level {
        let msg = match new.level {
            Level::Ok => tr("Speicher wieder in Ordnung", "Storage OK again"),
            Level::Low => tr("Speicher wird knapp", "Storage running low"),
            Level::Full => tr(
                "Speicher fast voll: neue Auslesungen werden nicht mehr gespeichert",
                "Storage almost full: new readings are no longer stored",
            ),
        };
        note(
            st,
            format!("{msg} ({:.0} % {})", new.free_pct, tr("frei", "free")),
        );
    }
}

/// Verlauf ausdünnen, wo eingeschaltet (höchstens einmal pro Stunde)
fn thin(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    {
        let mut last = st.planner.last_thin.lock().unwrap();
        if last.is_some_and(|t| (now - t).num_minutes().abs() < 60) {
            return Ok(());
        }
        *last = Some(now);
    }
    let dropped = st.db.lock().unwrap().mqtt_trim()?;
    if dropped > 0 {
        note(
            st,
            format!(
                "MQTT: {dropped} {}",
                tr(
                    "alte Nachrichten verworfen (Broker zu lange nicht erreichbar)",
                    "old messages dropped (broker unreachable for too long)"
                )
            ),
        );
    }
    let n = st.db.lock().unwrap().thin_readings(now)?;
    if n > 0 {
        note(
            st,
            format!(
                "{}: {n} {}",
                tr("Verlauf ausgedünnt", "History thinned"),
                tr("Auslesungen gelöscht", "readings deleted")
            ),
        );
    }
    Ok(())
}

/// Zeitquelle prüfen und Sprünge der Uhr erkennen
fn check_clock(st: &AppState, now: NaiveDateTime) {
    let p = &st.planner;
    let mut last = p.last_clock_check.lock().unwrap();
    if last.is_none_or(|t| t.elapsed() >= CLOCK_CHECK) {
        *last = Some(Instant::now());
        if let Some(msg) = clock::check_system() {
            note(st, msg);
        }
        drop(last);
        check_disk(st);
        // geänderte Qualität (veraltet, gestört) an den Broker melden
        if let Err(e) = st.db.lock().unwrap().mqtt_refresh() {
            note(st, format!("MQTT: {e:#}"));
        }
    } else {
        drop(last);
    }
    let mut tick = p.last_tick.lock().unwrap();
    if let Some((wall, mono)) = *tick {
        let elapsed = mono.elapsed().mul_f64(clock::scale());
        let expected = wall + chrono::Duration::from_std(elapsed).unwrap_or_default();
        let jump = (now - expected).num_seconds();
        if jump.abs() > 60 {
            note(
                st,
                format!(
                    "{} {:+} min ({} → {}); {}",
                    tr("Uhr gesprungen um", "Clock jumped by"),
                    jump / 60,
                    expected.format("%d.%m. %H:%M"),
                    now.format("%d.%m. %H:%M"),
                    tr("Termine richten sich neu aus", "terms realign")
                ),
            );
        }
    }
    *tick = Some((now, Instant::now()));
}

/// Einmal prüfen, was fällig ist, und es in Auftrag geben
pub fn tick(st: &AppState) -> anyhow::Result<()> {
    let now = (st.planner.now)();
    check_clock(st, now);
    thin(st, now)?;
    let lines = st.db.lock().unwrap().planned()?;
    let mut planned = HashSet::new();
    for pl in lines {
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

enum Attempt {
    Read(Target, Box<MeterReading>, f64),
    Failed(String),
    /// Zähler lehnt ab: 08 beschäftigt, 09 zu viele Auslesungen
    Refused(String),
    /// Linie nicht verbunden oder Verbindung beim Lesen abgebrochen: bleibt fällig
    NoLink(String),
}

/// Einmal lesen und bei einem Fehler einmal wiederholen (nicht, wenn der Zähler ablehnt)
fn attempt(master: &mut Master, meter: &Meter) -> Attempt {
    let home = master.baud();
    let mut result = Attempt::Failed(String::new());
    for _ in 0..2 {
        let t0 = Instant::now();
        match read_listed(master, meter, home) {
            Ok((target, r)) => {
                result = Attempt::Read(target, Box::new(r), t0.elapsed().as_secs_f64() * 1000.0);
                break;
            }
            Err(e) => {
                if e.downcast_ref::<MeterRefused>()
                    .is_some_and(|r| matches!(r.code, 8 | 9))
                {
                    result = Attempt::Refused(format!("{e:#}"));
                    break;
                }
                if is_link_error(&e) {
                    result = Attempt::NoLink(format!("{e:#}"));
                    break;
                }
                result = Attempt::Failed(format!("{e:#}"));
            }
        }
    }
    let _ = master.set_baud(home);
    result
}

fn meter_name(m: &Meter) -> String {
    if !m.name.is_empty() {
        m.name.clone()
    } else {
        m.meter_id.clone().unwrap_or_else(|| m.address.clone())
    }
}

fn work(st: AppState, line_id: i64, meter: Meter, plan: Plan, slot: NaiveDateTime) -> Work {
    Box::new(move |m| {
        i18n::set(Lang::De);
        let outcome = match m {
            Err(e) => Attempt::NoLink(e),
            Ok(master) => attempt(master, &meter),
        };
        let link_ok = !matches!(outcome, Attempt::NoLink(_));
        if let Err(e) = finish(&st, line_id, &meter, plan, slot, outcome) {
            note(&st, format!("{}: {e:#}", tr("Zeitplan", "Schedule")));
        }
        link_ok
    })
}

fn finish(
    st: &AppState,
    line_id: i64,
    meter: &Meter,
    plan: Plan,
    slot: NaiveDateTime,
    outcome: Attempt,
) -> anyhow::Result<()> {
    let now = (st.planner.now)();
    // hat es lange gedauert, gilt auch ein inzwischen fälliger Termin als erledigt (nie doppelt)
    let slot = plan
        .prev_slot(now)
        .map_or(slot, |p| p.max(slot))
        .format(SLOT_FORMAT)
        .to_string();
    let target = Target::parse(&meter.address);
    let mut read = false;
    let mut attempted = true;
    let mut error = None;
    // Auslesung, Zählerliste und Termin zusammen speichern: ganz oder gar nicht
    let save = |result: Result<(&Target, &MeterReading), String>, outcome: PlanOutcome| {
        st.db.lock().unwrap().tx(|db| {
            match (result, &target) {
                (Ok((t, r)), _) => db.record(line_id, t, Ok(r), true)?,
                (Err(e), Ok(t)) => db.record(line_id, t, Err(e), true)?,
                (Err(_), Err(_)) => {}
            }
            db.plan_done(meter.id, &slot, outcome)
        })
    };
    match outcome {
        Attempt::Read(t, r, ms) => {
            save(Ok((&t, &r)), PlanOutcome::Read { ms })?;
            read = true;
            // Meldungen der Datenpunkte (z.B. Zählerstand gesunken)
            let notes = std::mem::take(&mut st.db.lock().unwrap().notes);
            for n in notes {
                note(st, n);
            }
        }
        Attempt::Failed(e) => {
            let fails = save(Err(e.clone()), PlanOutcome::Failed)?;
            if fails == DISTURBED_AFTER {
                note(
                    st,
                    format!(
                        "{} {}: {} {fails} {} ({e})",
                        tr("Zähler", "Meter"),
                        meter_name(meter),
                        tr("gestört,", "disturbed,"),
                        tr(
                            "Termine nacheinander nicht gelesen",
                            "terms in a row not read"
                        )
                    ),
                );
            }
            error = Some(e);
        }
        Attempt::Refused(e) => {
            let until = plan::pause_until(now).format(SLOT_FORMAT).to_string();
            save(Err(e.clone()), PlanOutcome::Refused { until })?;
            note(
                st,
                format!(
                    "{} {}: {e}; {}",
                    tr("Zähler", "Meter"),
                    meter_name(meter),
                    tr("für heute ausgesetzt", "suspended for today")
                ),
            );
            error = Some(e);
        }
        Attempt::NoLink(e) => {
            attempted = false;
            error = Some(e);
        }
    }
    st.planner.queued.lock().unwrap().remove(&meter.id);
    let finished = {
        let mut runs = st.planner.runs.lock().unwrap();
        let Some(run) = runs.get_mut(&line_id) else {
            return Ok(());
        };
        run.done += 1;
        run.ok += read as u32;
        run.attempted += attempted as u32;
        if error.is_some() {
            run.last_error = error;
        }
        if run.done < run.total {
            return Ok(());
        }
        runs.remove(&line_id)
    };
    if let Some(run) = finished {
        finish_run(st, run)?;
    }
    Ok(())
}

/// Termin einer Linie abgeschlossen: Ergebnis speichern, Linie gestört melden
fn finish_run(st: &AppState, run: Run) -> anyhow::Result<()> {
    let offline = run.attempted == 0;
    {
        let mut saved = st.planner.offline_saved.lock().unwrap();
        if offline && !saved.insert(run.line_id) {
            return Ok(());
        }
        if !offline {
            saved.remove(&run.line_id);
        }
    }
    let line_down = run.attempted >= 2 && run.ok == 0;
    let result = json!({
        "time": clock::stamp(),
        "ok": run.ok,
        "total": run.attempted,
        "offline": offline,
        "line_down": line_down,
        "error": if run.ok < run.attempted || offline { run.last_error.clone() } else { None },
    });
    st.db
        .lock()
        .unwrap()
        .set_plan_result(run.line_id, &result)?;
    if offline {
        note(
            st,
            format!(
                "{} {}: {} ({})",
                tr("Plan", "Plan"),
                run.label,
                tr(
                    "Linie nicht verbunden, Zähler bleiben fällig",
                    "line not connected, meters stay due"
                ),
                run.last_error.unwrap_or_default()
            ),
        );
    } else if line_down {
        note(
            st,
            format!(
                "{} {}: {}",
                tr("Linie", "Line"),
                run.label,
                tr(
                    "gestört, kein einziger Zähler hat geantwortet (Leitung, Wandler, Gateway?)",
                    "disturbed, not a single meter answered (cable, converter, gateway?)"
                )
            ),
        );
    } else {
        note(
            st,
            format!(
                "{} {}: {}/{} {}",
                tr("Plan", "Plan"),
                run.label,
                run.ok,
                run.attempted,
                tr("gelesen", "read")
            ),
        );
    }
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
}
