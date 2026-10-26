//! Ausgänge: Einstellungen und Zustand von MQTT und Modbus; Adressen und Liste für den Integrator

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
    Json(r): Json<MqttReq>,
) -> ApiResult<serde_json::Value> {
    {
        let mut db = st.db.lock().unwrap();
        let mut c = r.config;
        c.password = r.password.unwrap_or_else(|| db.mqtt_config().password);
        c.host = c.host.trim().to_string();
        c.base = c.base.trim().trim_end_matches('/').to_string();
        db.set_mqtt_config(&c).map_err(bad_request)?;
        // eingeschaltet: aktuelle Werte gleich vormerken
        db.outputs_refresh()?;
    }
    st.mqtt.version.fetch_add(1, Ordering::SeqCst);
    st.log.push(
        "INFO",
        both(|| tr("MQTT-Einstellungen gespeichert", "MQTT settings saved")),
    );
    Ok(Json(json!({ "ok": true })))
}

pub(super) fn modbus_status(st: &AppState) -> crate::modbus::ModbusStatus {
    st.modbus.status.lock().unwrap().clone()
}

/// Einstellungen, Zustand und die Projekte zur Auswahl
pub(super) async fn modbus_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    let projects: Vec<serde_json::Value> = db
        .tree()?
        .iter()
        .map(|p| json!({ "id": p.project.id, "name": p.project.name }))
        .collect();
    Ok(Json(json!({
        "config": db.modbus_config(),
        "status": modbus_status(&st),
        "projects": projects,
    })))
}

pub(super) async fn modbus_put(
    State(st): State<AppState>,
    Json(mut c): Json<crate::modbus::ModbusConfig>,
) -> ApiResult<serde_json::Value> {
    c.bind = c.bind.trim().to_string();
    {
        let mut db = st.db.lock().unwrap();
        db.set_modbus_config(&c).map_err(bad_request)?;
        db.outputs_refresh()?;
    }
    st.modbus.version.fetch_add(1, Ordering::SeqCst);
    st.log.push(
        "INFO",
        both(|| tr("Modbus-Einstellungen gespeichert", "Modbus settings saved")),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct ApplyReq {
    /// false = nur zeigen, was sich ändert
    #[serde(default)]
    apply: bool,
}

/// Alle Adressen eines Ausgangs im Projekt nach dem Muster neu setzen (Vorschau → Übernehmen)
pub(super) async fn output_readdress(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
    Json(r): Json<ApplyReq>,
) -> ApiResult<crate::changeset::ChangeSet> {
    let mut db = st.db.lock().unwrap();
    let cs = db.dp_readdress(id, &key, r.apply).map_err(bad_request)?;
    if cs.applied {
        // neue Topics: aktuelle Werte gleich unter der neuen Adresse senden
        db.outputs_refresh()?;
    }
    Ok(Json(cs))
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    #[serde(default)]
    format: String,
}

/// Liste für den Integrator: CSV (Excel) oder JSON mit allem, was man zum Einbinden braucht
pub(super) async fn output_list(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
    Query(q): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let lang = i18n::current();
    let db = st.db.lock().unwrap();
    let project = db.project(id)?.name;
    let out = crate::datapoint::output(&key).map_err(bad_request)?;
    let rows = db.output_list(id, &key, lang).map_err(bad_request)?;
    let what = format!("{key}-liste");
    if q.format == "json" {
        let mut v = json!({ "project": project, "output": key, "datapoints": rows });
        // was ein Leitsystem zusätzlich wissen muss, z.B. bei MQTT Basis- und Status-Topic
        if let Some(info) = out.list_info(&db) {
            v[out.key()] = info;
        }
        let name: String = project
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        return Ok(download(
            serde_json::to_string_pretty(&v).unwrap_or_default(),
            "application/json",
            &format!("mbusio-{what}-{name}.json"),
        ));
    }
    let de = lang == Lang::De;
    let head: Vec<String> = if de {
        [
            "Nr",
            out.address_label().as_str(),
            "Name",
            "Einheit",
            "Faktor",
            "Weitergabe",
            "Zähler",
            "ID",
            "PA",
            "Ort",
            "Medium",
            "Linie",
            "Kennung",
        ]
        .map(String::from)
        .to_vec()
    } else {
        [
            "No",
            out.address_label().as_str(),
            "Name",
            "Unit",
            "Factor",
            "Send",
            "Meter",
            "ID",
            "PA",
            "Location",
            "Medium",
            "Line",
            "Key",
        ]
        .map(String::from)
        .to_vec()
    };
    let mut csv = crate::export::row(&head);
    let s = |v: &serde_json::Value| match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        x => x.to_string(),
    };
    for r in &rows {
        csv.push_str(&crate::export::row(
            &[
                "no", "address", "name", "unit", "factor", "send", "meter", "id", "pa", "location",
                "medium", "line", "key",
            ]
            .map(|k| s(&r[k])),
        ));
    }
    Ok(csv_download(csv, &what, &project))
}
