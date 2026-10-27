//! Meldungen (Konzept E-Mail 5.1): offene Probleme für die Statusleiste, „gesehen“

use super::*;

/// Offene Probleme, die lange genug bestehen, neueste zuerst
pub(super) async fn warnings_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let s = st.db.lock().unwrap().mail_config().warn;
    let now = st.planner.now();
    let w = st.planner.warnings.lock().unwrap();
    let mut items: Vec<serde_json::Value> = w
        .visible(&s, now)
        .into_iter()
        .map(|o| {
            json!({ "key": o.key, "kind": o.kind, "text": o.text(), "place": o.place, "since": o.since,
                    "line": o.line, "meter": o.meter, "project": o.project, "seen": o.seen })
        })
        .collect();
    items.sort_by(|a, b| b["since"].as_str().cmp(&a["since"].as_str()));
    Ok(Json(json!({ "items": items })))
}

/// Zahlen für die Statusleiste: offen, davon nicht gesehen
pub(super) fn warnings_count(st: &AppState, db: &Db) -> serde_json::Value {
    let s = db.mail_config().warn;
    let now = st.planner.now();
    let w = st.planner.warnings.lock().unwrap();
    let v = w.visible(&s, now);
    json!({ "open": v.len(), "unseen": v.iter().filter(|o| !o.seen).count() })
}

#[derive(Deserialize)]
pub(super) struct SeenReq {
    /// leer = alle
    #[serde(default)]
    keys: Vec<String>,
}

pub(super) async fn warnings_seen(
    State(st): State<AppState>,
    Json(r): Json<SeenReq>,
) -> ApiResult<serde_json::Value> {
    let state = {
        let mut w = st.planner.warnings.lock().unwrap();
        w.see(&r.keys).then(|| w.clone())
    };
    if let Some(state) = state {
        st.db.lock().unwrap().set_warn_state(&state)?;
    }
    Ok(Json(json!({ "ok": true })))
}
