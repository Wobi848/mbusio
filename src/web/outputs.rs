//! Ausgänge: MQTT-Einstellungen und Zustand

use super::*;

pub(super) fn mqtt_status(st: &AppState, db: &Db) -> crate::mqtt::MqttStatus {
    let mut s = st.mqtt.status.lock().unwrap().clone();
    s.queued = db.mqtt_queued().unwrap_or(0);
    s
}

/// Einstellungen ohne Passwort, dazu ob eines gespeichert ist, und der Zustand
pub(super) async fn mqtt_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    let mut c = db.mqtt_config();
    let has_password = !c.password.is_empty();
    c.password.clear();
    Ok(Json(
        json!({ "config": c, "has_password": has_password, "status": mqtt_status(&st, &db) }),
    ))
}

#[derive(Deserialize)]
pub(super) struct MqttReq {
    config: crate::mqtt::MqttConfig,
    /// None = gespeichertes Passwort behalten, "" = löschen
    #[serde(default)]
    password: Option<String>,
}

pub(super) async fn mqtt_put(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<MqttReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    {
        let mut db = st.db.lock().unwrap();
        let mut c = r.config;
        c.password = r.password.unwrap_or_else(|| db.mqtt_config().password);
        c.host = c.host.trim().to_string();
        c.base = c.base.trim().trim_end_matches('/').to_string();
        db.set_mqtt_config(&c).map_err(bad_request)?;
        // eingeschaltet: aktuelle Werte gleich vormerken
        db.mqtt_refresh()?;
    }
    st.mqtt.version.fetch_add(1, Ordering::SeqCst);
    st.log.push(
        "INFO",
        tr("MQTT-Einstellungen gespeichert", "MQTT settings saved"),
    );
    Ok(Json(json!({ "ok": true })))
}
