//! Ein geplanter Zähler wird gelesen: Versuch (auch Ablehnung durch den Zähler), Ergebnis
//! speichern, Durchgang einer Linie abschliessen.

use super::*;

pub(super) enum Attempt {
    Read(Target, Box<MeterReading>, f64),
    Failed(String),
    /// Zähler lehnt ab: 08 beschäftigt, 09 zu viele Auslesungen
    Refused(String),
    /// Linie nicht verbunden oder Verbindung beim Lesen abgebrochen: bleibt fällig
    NoLink(String),
}

/// Einmal lesen und bei einem Fehler einmal wiederholen (nicht, wenn der Zähler ablehnt)
pub(super) fn attempt(master: &mut Master, meter: &Meter) -> Attempt {
    let mut result = Attempt::Failed(String::new());
    for _ in 0..2 {
        let t0 = Instant::now();
        match read_listed(master, meter) {
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
    result
}

pub(super) fn meter_name(m: &Meter) -> String {
    if !m.name.is_empty() {
        m.name.clone()
    } else {
        m.meter_id.clone().unwrap_or_else(|| m.address.clone())
    }
}

pub(super) fn work(
    st: AppState,
    line_id: i64,
    meter: Meter,
    plan: Plan,
    slot: NaiveDateTime,
) -> Work {
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

pub(super) fn finish(
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
pub(super) fn finish_run(st: &AppState, run: Run) -> anyhow::Result<()> {
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
    {
        let mut db = st.db.lock().unwrap();
        db.set_plan_result(run.line_id, &result)?;
        // Linie weg oder wieder da: Qualität gleich an die Ausgänge, nicht erst bei der nächsten Prüfung
        if let Err(e) = db.outputs_refresh() {
            drop(db);
            note(st, format!("MQTT: {e:#}"));
        }
    }
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
