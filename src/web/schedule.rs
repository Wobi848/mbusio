//! Ausleseplan (Projekt, Linie, Zähler) mit Warnung zur Buslast, Dauerbetrieb, Ausdünnen des
//! Verlaufs, Uhrzeit vom Gerät

use super::*;

#[derive(Deserialize)]
pub(super) struct PlanReq {
    /// "off", "daily 00:05", …; None = erben (bei Linie und Zähler)
    plan: Option<String>,
}

/// Warnungen zur Buslast der Linien eines Projekts (nach dem Ändern eines Plans)
pub(super) fn load_warnings(
    st: &AppState,
    project_id: Option<i64>,
    line_id: Option<i64>,
) -> Result<Vec<String>, ApiError> {
    let db = st.db.lock().unwrap();
    let mut out = Vec::new();
    for p in db.tree()? {
        if project_id.is_some_and(|id| id != p.project.id) {
            continue;
        }
        for g in &p.gateways {
            for l in &g.lines {
                if line_id.is_some_and(|id| id != l.line.id) {
                    continue;
                }
                if l.plan_info["load_warn"].as_bool().unwrap_or(false) {
                    out.push(format!(
                        "{} › {}: {} {:.0} % {}",
                        g.gateway.name,
                        l.line.name,
                        tr(
                            "geplante Auslesungen brauchen ca.",
                            "planned reads need about"
                        ),
                        l.plan_info["load"].as_f64().unwrap_or(0.0) * 100.0,
                        tr(
                            "der Buszeit. Seltener auslesen oder auf mehrere Linien verteilen.",
                            "of the bus time. Read less often or split across lines."
                        )
                    ));
                }
            }
        }
    }
    Ok(out)
}

pub(super) async fn project_plan(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    let plan = r.plan.unwrap_or_else(|| "off".into());
    st.db
        .lock()
        .unwrap()
        .set_project_plan(id, &plan)
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, Some(id), None)? }),
    ))
}

pub(super) async fn line_plan(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_line_plan(id, r.plan.as_deref())
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, None, Some(id))? }),
    ))
}

#[derive(Deserialize)]
pub(super) struct OperationReq {
    on: bool,
}

/// Dauerbetrieb der Linie ein/aus (Konzept Bedienung §13); der Zeitplan nimmt sie beim nächsten
/// Durchgang (10 s) auf bzw. beendet ihre Hintergrund-Verbindung
pub(super) async fn line_operation(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<OperationReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_operation(id, r.on)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn meter_plan(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    let line = {
        let mut db = st.db.lock().unwrap();
        db.set_meter_plan(id, r.plan.as_deref())
            .map_err(bad_request)?;
        db.meter(id)?.line_id
    };
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, None, Some(line))? }),
    ))
}

#[derive(Deserialize)]
pub(super) struct ThinReq {
    /// nach so vielen Tagen nur noch ein Wert pro Tag; None = alles behalten
    days: Option<i64>,
}

pub(super) async fn project_thin(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<ThinReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_thin_days(id, r.days)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct TimeReq {
    /// Uhrzeit des Browsers, Millisekunden seit 1970 (UTC)
    epoch_ms: i64,
}

/// „Uhrzeit vom Gerät übernehmen“
pub(super) async fn set_time(
    State(st): State<AppState>,
    Json(r): Json<TimeReq>,
) -> ApiResult<serde_json::Value> {
    let offset = clock::set_from_browser(r.epoch_ms).ok_or_else(|| bad_request("epoch_ms"))?;
    st.log.push(
        "INFO",
        both(|| {
            if offset == 0.0 {
                tr("Uhrzeit vom Gerät bestätigt", "Time confirmed by device")
            } else {
                format!(
                    "{} {offset:+.0} s",
                    tr(
                        "Uhrzeit vom Gerät übernommen, Korrektur",
                        "Time taken from device, correction"
                    )
                )
            }
        }),
    );
    Ok(Json(
        json!({ "ok": true, "offset_s": offset, "time": clock::state() }),
    ))
}
