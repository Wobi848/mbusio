//! Projekte, Gateways, Linien, Ausleseplan, Uhrzeit, Bericht, Projekt-Export

use super::*;

/// Zustand der Linien-Dienste in den Baum eintragen: verbunden, Fehler, von Hand getrennt
pub(super) fn annotate_lines(st: &AppState, tree: &mut serde_json::Value) {
    let services = st.lines.lock().unwrap().clone();
    for p in tree.as_array_mut().into_iter().flatten() {
        for g in p["gateways"].as_array_mut().into_iter().flatten() {
            for l in g["lines"].as_array_mut().into_iter().flatten() {
                let Some(id) = l["id"].as_i64() else { continue };
                l["service"] = json!(services.get(&id).map(|s| s.status()));
                l["held"] = json!(st.planner.is_held(id));
            }
        }
    }
}

pub(super) async fn tree(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let (mut projects, active) = {
        let db = st.db.lock().unwrap();
        (
            serde_json::to_value(db.tree()?).unwrap_or_default(),
            db.active()?,
        )
    };
    annotate_lines(&st, &mut projects);
    Ok(Json(json!({ "projects": projects, "active": active })))
}

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
        },
    );
    Ok(Json(
        json!({ "ok": true, "offset_s": offset, "time": clock::state() }),
    ))
}

/// Projekt anlegen und gleich seine erste Linie aktivieren
pub(super) async fn project_create(
    State(st): State<AppState>,
    Json(p): Json<Project>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let mut db = st.db.lock().unwrap();
    let p = db.create_project(p).map_err(bad_request)?;
    let tree = db.tree()?;
    let line = tree
        .iter()
        .find(|n| n.project.id == p.id)
        .and_then(|n| n.gateways.first()?.lines.first().map(|l| l.line.id));
    if let Some(line) = line {
        db.set_active(line)?;
    }
    drop(db);
    disconnect_bus(&st);
    Ok(Json(json!({ "ok": true, "project": p })))
}

pub(super) async fn project_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(p): Json<Project>,
) -> ApiResult<Project> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_project(id, p)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn project_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db
        .lock()
        .unwrap()
        .delete_project(id)
        .map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

/// Projekt leeren (Zähler, Auslesungen, letzter Scan); nicht, solange eine seiner Linien arbeitet
pub(super) async fn project_clear(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let lines = st
        .db
        .lock()
        .unwrap()
        .lines_of_project(id)
        .map_err(bad_request)?;
    if lines.iter().any(|l| st.line_busy(l.id)) {
        return Err(ApiError(
            StatusCode::CONFLICT,
            tr(
                "Eine Linie des Projekts liest gerade aus. Warten, bis sie fertig ist, dann leeren.",
                "A line of this project is reading. Wait until it is done, then clear.",
            ),
        ));
    }
    let n = st
        .db
        .lock()
        .unwrap()
        .clear_project(id)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

pub(super) async fn gateway_create(
    State(st): State<AppState>,
    Path(project): Path<i64>,
    Json(g): Json<Gateway>,
) -> ApiResult<Gateway> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .create_gateway(project, g)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn gateway_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(g): Json<Gateway>,
) -> ApiResult<Gateway> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_gateway(id, g)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn gateway_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db
        .lock()
        .unwrap()
        .delete_gateway(id)
        .map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn line_create(
    State(st): State<AppState>,
    Path(gateway): Path<i64>,
    Json(l): Json<Line>,
) -> ApiResult<Line> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .create_line(gateway, l)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn line_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(l): Json<Line>,
) -> ApiResult<Line> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_line(id, l)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn line_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db.lock().unwrap().delete_line(id).map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct MoveReq {
    /// -1 nach oben, +1 nach unten
    dir: i64,
}

pub(super) async fn gateway_move(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<MoveReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .move_gateway(id, r.dir)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn line_move(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<MoveReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .move_line(id, r.dir)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct AddressingReq {
    mode: String,
}

/// Merken über SA oder PA; stellt die Zählerliste der Linie mit um
pub(super) async fn line_addressing(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AddressingReq>,
) -> ApiResult<serde_json::Value> {
    let (changed, kept) = st
        .db
        .lock()
        .unwrap()
        .set_addressing(id, &req.mode)
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "changed": changed, "kept": kept }),
    ))
}

/// Auffälligkeiten der Linie merken (berechnet die Oberfläche aus dem letzten Scan)
pub(super) async fn line_issues(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(v): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_issues(id, &v)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Linie aktivieren und, wenn sie eine Verbindung hat, gleich damit verbinden
pub(super) async fn line_activate(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let line = st.db.lock().unwrap().set_active(id).map_err(bad_request)?;
    disconnect_bus(&st);
    if line.connection.is_empty() {
        return Ok(Json(json!({ "ok": true, "connected": false })));
    }
    let s = st.clone();
    let conf = LineConf::of(&line);
    let result = blocking(move || s.connect_line(id, conf)).await?;
    // Linie bleibt aktiv, auch wenn der Anschluss gerade nicht erreichbar ist
    Ok(Json(match result {
        Ok(()) => json!({ "ok": true, "connected": true }),
        Err(e) => json!({ "ok": true, "connected": false, "error": format!("{e:#}") }),
    }))
}

#[derive(Deserialize)]
pub(super) struct ProjectQuery {
    #[serde(default)]
    project: Option<i64>,
}

pub(super) fn project_or_active(st: &AppState, q: &ProjectQuery) -> Result<i64, ApiError> {
    match q.project {
        Some(id) => Ok(id),
        None => Ok(st.db.lock().unwrap().active()?.project.id),
    }
}

/// Alles für den Bericht eines Projekts (Standard: aktives Projekt)
pub(super) async fn report(
    State(st): State<AppState>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<serde_json::Value> {
    let id = project_or_active(&st, &q)?;
    Ok(Json(st.db.lock().unwrap().report(id).map_err(bad_request)?))
}

pub(super) async fn changes_clear(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let n = st.db.lock().unwrap().clear_changes(id)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

pub(super) async fn project_export(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let v = db.export_project(id).map_err(bad_request)?;
    let name: String = db
        .project(id)?
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(download(
        serde_json::to_string_pretty(&v).unwrap_or_default(),
        "application/json",
        &format!("mbusio-{name}.json"),
    ))
}

pub(super) async fn project_import(
    State(st): State<AppState>,
    Json(v): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let id = st
        .db
        .lock()
        .unwrap()
        .import_project(&v)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "project": id })))
}
