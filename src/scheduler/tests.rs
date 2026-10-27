//! Tests des Zeitplans (Termine, gestörte Zähler, Nachholen, Speicher, Uhrsprünge)

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
    let result =
        r.st.db.lock().unwrap().tree().unwrap()[0].gateways[0].lines[0].plan_info["result"].clone();
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

#[test]
fn disturbed_meter_becomes_warning_and_mail() {
    let r = Rig::new(&["1", "4"]);
    let (port, server) = crate::mail::tests::fake_server("PLAIN");
    {
        let mut db = r.st.db.lock().unwrap();
        db.set_project_plan(1, "hourly").unwrap();
        let mut c = crate::mail::tests::config(port, "geheim");
        c.device = "keller".into();
        c.warn.to = "hv@example.org".into();
        db.set_mail_config(&c).unwrap();
    }
    for t in [
        "2026-10-17 10:30",
        "2026-10-17 11:00",
        "2026-10-17 12:00",
        "2026-10-17 13:00",
        "2026-10-17 13:01",
    ] {
        r.set_time(t);
        r.tick();
    }
    let id = r.meter("4").id;
    let open = r.st.planner.warnings.lock().unwrap().open.clone();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].key, format!("disturbed:{id}"));
    assert!(open[0].mailed.is_some());
    let sent = server.join().unwrap();
    assert!(sent.contains("RCPT TO:<hv@example.org>"), "{sent}");
    // gespeichert: nach einem Neustart nicht noch einmal
    assert_eq!(r.st.db.lock().unwrap().warn_state().open, open);
}

/// Bericht (Konzept E-Mail 6): nach dem Einschalten nicht sofort, am nächsten Termin mit den
/// Zahlen seit dem Beginn des Zeitraums
#[test]
fn daily_report_with_readings_goes_out_at_its_time() {
    crate::i18n::set(crate::i18n::Lang::De);
    let r = Rig::new(&["1", "4"]);
    let (port, server) = crate::mail::tests::fake_server("PLAIN");
    {
        let mut db = r.st.db.lock().unwrap();
        db.set_project_plan(1, "hourly").unwrap();
        let mut c = crate::mail::tests::config(port, "geheim");
        c.device = "keller".into();
        c.report.to = "hv@example.org".into();
        db.set_mail_config(&c).unwrap();
    }
    for t in ["2026-10-17 10:30", "2026-10-17 11:00", "2026-10-18 06:59"] {
        r.set_time(t);
        r.tick();
    }
    // vor dem Termin keine Mail
    assert!(r.st.db.lock().unwrap().mail_last().is_none());
    let lines =
        r.st.db
            .lock()
            .unwrap()
            .report_reading("2026-10-17 07:00:00")
            .unwrap();
    assert_eq!((lines[0].read, lines[0].total), (1, 2));
    r.set_time("2026-10-18 07:00");
    r.tick();
    let sent = server.join().unwrap();
    assert!(sent.contains("RCPT TO:<hv@example.org>"), "{sent}");
    let last = r.st.db.lock().unwrap().mail_last().unwrap();
    assert_eq!(last.subject, "mbusio keller: Bericht 18.10.2026");
    assert!(last.error.is_none(), "{:?}", last.error);
}
