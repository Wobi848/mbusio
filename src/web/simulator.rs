//! Simulator-Reiter: Simulationen anlegen, ändern, verwenden

use super::*;

pub(super) async fn sim_get(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let active = st.db.lock().unwrap().active_line()?;
    Ok(Json(json!({
        "scenarios": crate::sim::all(),
        "media": crate::sim::MEDIA.iter().map(|(k, _)| k).collect::<Vec<_>>(),
        "faults": crate::sim::FAULTS,
        "line": { "id": active.id, "name": active.name, "connection": active.connection, "baud": active.baud },
    })))
}

#[derive(Deserialize)]
pub(super) struct SimCreateReq {
    name: String,
    /// Kopie dieser Simulation; leer = ohne Zähler
    #[serde(default)]
    from: Option<String>,
}

pub(super) async fn sim_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<SimCreateReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let key = st
        .db
        .lock()
        .unwrap()
        .sim_create(&r.name, r.from.as_deref())
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "key": key })))
}

#[derive(Deserialize)]
pub(super) struct SimSaveReq {
    name: String,
    meters: Vec<crate::sim::SimMeter>,
}

/// Simulation speichern; Linien, die sie gerade verwenden, verbinden neu (neue Zähler gelten sofort)
pub(super) async fn sim_save(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(r): Json<SimSaveReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    st.db
        .lock()
        .unwrap()
        .sim_save(&key, &r.name, &r.meters)
        .map_err(bad_request)?;
    let s = st.clone();
    let restarted =
        blocking(move || s.restart_connection(&format!("SIMULATOR:{key}"), lang)).await?;
    Ok(Json(json!({ "ok": true, "restarted": restarted })))
}

pub(super) async fn sim_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    st.db
        .lock()
        .unwrap()
        .sim_delete(&key)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Simulation für die aktive Linie verwenden und verbinden
pub(super) async fn sim_use(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    ensure_idle(&st)?;
    let connection = if key == "praxis" {
        "SIMULATOR".to_string()
    } else {
        format!("SIMULATOR:{key}")
    };
    if !crate::sim::all().iter().any(|s| s.key == key) {
        return Err(bad_request(format!(
            "{} „{key}“ {}",
            tr("Simulation", "Simulation"),
            tr("gibt es nicht", "does not exist")
        )));
    }
    let line = st.db.lock().unwrap().active_line()?;
    let conf = LineConf {
        connection: connection.clone(),
        baud: line.baud,
        timeout_ms: line.timeout_ms,
    };
    let s = st.clone();
    let id = line.id;
    blocking(move || s.connect_line(id, conf, lang))
        .await?
        .map_err(bad_request)?;
    st.db
        .lock()
        .unwrap()
        .set_line_connection(id, &connection, line.baud, line.timeout_ms)?;
    Ok(Json(json!({ "ok": true, "connection": connection })))
}
