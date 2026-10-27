//! Warnungen (Konzept E-Mail 5): offene Probleme bestimmen, Meldungen nachführen und
//! neue gebündelt per E-Mail. Gesendet wird ohne die Datenbank zu sperren.

use super::*;
use crate::db::PlannedLine;
use crate::mail::{self, Message};
use crate::warn::Problem;

/// Bei jeder Prüfung des Zeitplans (alle 10 s): billig, gespeichert wird nur bei Änderungen
pub(super) fn warn(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    let (config, lines, mut found) = {
        let db = st.db.lock().unwrap();
        let lines = db.planned()?;
        let mut found = db.meter_problems(&lines)?;
        if let Some(dir) = db.path().and_then(|p| p.parent()) {
            found.extend(db.billing_problems(dir, now)?);
        }
        (db.mail_config(), lines, found)
    };
    let mut unsure = Vec::new();
    line_problems(st, &lines, &mut found, &mut unsure);
    mqtt_problem(st, &mut found, &mut unsure);
    found.extend(disk_problem(st));

    let (state, due) = {
        let mut w = st.planner.warnings.lock().unwrap();
        let changed = w.update(found, &unsure, &config.warn, now);
        let due = config
            .ready()
            .ok()
            .and_then(|_| w.mail_due(&config.warn, &config.device_name(), now));
        ((changed || due.is_some()).then(|| w.clone()), due)
    };
    let Some(mut state) = state else {
        return Ok(());
    };
    if let Some(due) = due {
        let message = Message {
            from: config.ready()?,
            to: mail::recipients(&config.warn.to)?,
            subject: due.subject,
            text: due.text,
            attachments: Vec::new(),
        };
        let result = mail::send(&config, &message);
        st.db.lock().unwrap().mail_record(&message, &result)?;
        let mut w = st.planner.warnings.lock().unwrap();
        w.mail_done(&due.keys, result.is_ok(), now);
        state = w.clone();
    }
    st.db.lock().unwrap().set_warn_state(&state)
}

/// Linien im Dauerbetrieb, deren Verbindung nicht klappt; verbindet sie gerade oder läuft kein
/// Dienst (nach dem Start), ist es unklar
fn line_problems(
    st: &AppState,
    lines: &[PlannedLine],
    found: &mut Vec<Problem>,
    unsure: &mut Vec<String>,
) {
    for pl in lines {
        let id = pl.line.id;
        if !pl.line.operation
            || pl.line.connection.is_empty()
            || !st.planner.is_planned(id)
            || st.planner.is_held(id)
        {
            continue;
        }
        let key = format!("line:{id}");
        match st.line_status(id) {
            Some(s) if s.connected => {}
            Some(crate::line::LineStatus {
                error: Some(why), ..
            }) => found.push(Problem {
                key,
                kind: "line",
                text: both(|| format!("{}: {why}", tr("nicht verbunden", "not connected"))),
                place: pl.label.clone(),
                since: None,
                line: Some(id),
                meter: None,
                project: None,
            }),
            _ => unsure.push(key),
        }
    }
}

fn mqtt_problem(st: &AppState, found: &mut Vec<Problem>, unsure: &mut Vec<String>) {
    let s = st.mqtt.status.lock().unwrap().clone();
    match s.state.as_str() {
        "error" => found.push(Problem {
            key: "mqtt".into(),
            kind: "mqtt",
            text: both(|| {
                format!(
                    "MQTT {}: {}",
                    s.broker,
                    tr("nicht verbunden", "not connected")
                )
            }),
            place: String::new(),
            since: None,
            line: None,
            meter: None,
            project: None,
        }),
        "connecting" => unsure.push("mqtt".into()),
        _ => {}
    }
}

fn disk_problem(st: &AppState) -> Option<Problem> {
    let text = match st.planner.disk().level {
        Level::Ok => return None,
        Level::Low => both(|| tr("Speicher wird knapp", "Storage running low")),
        Level::Full => both(|| {
            tr(
                "Speicher fast voll: neue Auslesungen werden nicht mehr gespeichert",
                "Storage almost full: new readings are no longer stored",
            )
        }),
    };
    Some(Problem {
        key: "disk".into(),
        kind: "disk",
        text,
        place: String::new(),
        since: None,
        line: None,
        meter: None,
        project: None,
    })
}
