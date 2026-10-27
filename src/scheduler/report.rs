//! Bericht (Konzept E-Mail 6): ist er fällig, Zahlen sammeln und senden. Gesendet wird ohne die
//! Datenbank zu sperren.

use super::*;
use crate::mail::{self, MailConfig, Message};
use crate::report::{self, ReportData};

pub(super) fn report(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    let (config, mut state) = {
        let db = st.db.lock().unwrap();
        (db.mail_config(), db.report_state())
    };
    // E-Mail aus oder unvollständig: der Bericht wartet und kommt danach als einer
    let Ok(from) = config.ready() else {
        return Ok(());
    };
    let (changed, since) = state.due(&config.report, now);
    let Some(since) = since else {
        if changed {
            st.db.lock().unwrap().set_report_state(&state)?;
        }
        return Ok(());
    };
    let data = gather(st, &config, since, now)?;
    let (subject, text) = report::mail(&config.report, &data, &config.device_name(), since, now);
    let message = Message {
        from,
        to: mail::recipients(&config.report.to)?,
        subject,
        text,
        attachments: Vec::new(),
    };
    let result = mail::send(&config, &message);
    state.done(result.is_ok(), now);
    let mut db = st.db.lock().unwrap();
    db.mail_record(&message, &result)?;
    db.set_report_state(&state)
}

fn gather(
    st: &AppState,
    config: &MailConfig,
    since: NaiveDateTime,
    now: NaiveDateTime,
) -> anyhow::Result<ReportData> {
    let problems = st
        .planner
        .warnings
        .lock()
        .unwrap()
        .visible(&config.warn, now)
        .into_iter()
        .cloned()
        .collect();
    let since = since.format("%Y-%m-%d %H:%M:%S").to_string();
    let db = st.db.lock().unwrap();
    let billing = match db.path().and_then(|p| p.parent()) {
        Some(dir) => db.billing_report(dir, &since)?,
        None => Vec::new(),
    };
    Ok(ReportData {
        reading: db.report_reading(&since)?,
        outputs: db.report_outputs(&since)?,
        problems,
        billing,
        disk: st
            .planner
            .disk()
            .with_growth(db.growth_per_day().unwrap_or(0)),
    })
}
