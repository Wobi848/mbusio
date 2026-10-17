//! Datenpunkte: Auswahl, Vorlagen, Bearbeiten, CSV

use super::*;

pub(super) async fn datapoints(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    Ok(Json(
        json!({ "datapoints": db.dp_list(Some(line), None)?, "templates": db.templates(line)? }),
    ))
}

/// Werte eines Zählers zur Auswahl als Datenpunkte
pub(super) async fn meter_values(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .dp_candidates(id, lang)
            .map_err(bad_request)?,
    ))
}

#[derive(Deserialize)]
pub(super) struct TemplateOpt {
    /// auch auf später hinzukommende Zähler anwenden
    #[serde(default)]
    auto: bool,
}

#[derive(Deserialize)]
pub(super) struct SelectReq {
    keys: Vec<String>,
    /// neue Datenpunkte gleich per MQTT weitergeben (auch in der Vorlage)
    #[serde(default)]
    mqtt: bool,
    /// zusätzlich als Vorlage für alle Zähler dieses Mediums auf der Linie
    #[serde(default)]
    template: Option<TemplateOpt>,
}

pub(super) async fn meter_datapoints(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<SelectReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    let mut db = st.db.lock().unwrap();
    let (added, removed) = db
        .dp_select_mqtt(id, &r.keys, r.mqtt, lang)
        .map_err(bad_request)?;
    let mut others = 0;
    if let Some(t) = r.template {
        let m = db.meter(id)?;
        let Some(medium) = m.medium.clone() else {
            return Err(bad_request(tr(
                "Medium unbekannt, Zähler zuerst auslesen",
                "Medium unknown, read the meter first",
            )));
        };
        others = db
            .template_save(m.line_id, &medium, &r.keys, t.auto, r.mqtt, lang)
            .map_err(bad_request)?;
    }
    db.mqtt_refresh()?;
    Ok(Json(
        json!({ "ok": true, "added": added, "removed": removed, "others": others }),
    ))
}

pub(super) async fn datapoint_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(d): Json<crate::datapoint::Datapoint>,
) -> ApiResult<crate::datapoint::Datapoint> {
    i18n::set(lang_of(&headers));
    let mut db = st.db.lock().unwrap();
    let d = db.dp_update(id, &d).map_err(bad_request)?;
    db.mqtt_refresh()?;
    Ok(Json(d))
}

#[derive(Deserialize)]
pub(super) struct BulkReq {
    ids: Vec<i64>,
    /// "on", "off", "delete", "send:every|change|daily"
    action: String,
}

pub(super) async fn datapoints_bulk(
    State(st): State<AppState>,
    Json(r): Json<BulkReq>,
) -> ApiResult<serde_json::Value> {
    let n = {
        let mut db = st.db.lock().unwrap();
        let n = db.dp_bulk(&r.ids, &r.action).map_err(bad_request)?;
        db.mqtt_refresh()?;
        n
    };
    Ok(Json(json!({ "ok": true, "changed": n })))
}

pub(super) async fn template_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().template_delete(id)?;
    Ok(Json(json!({ "ok": true })))
}

/// Datenpunkt-Liste des Projekts als CSV (Excel), auch für den Integrator
pub(super) async fn datapoints_csv(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LangQuery>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let lang = q
        .lang
        .as_deref()
        .map(Lang::parse)
        .unwrap_or_else(|| lang_of(&headers));
    i18n::set(lang);
    let db = st.db.lock().unwrap();
    let name = db.project(id)?.name;
    // BOM, damit Excel die Umlaute richtig liest
    let body = format!("\u{feff}{}", db.dp_csv(id, lang)?);
    let file: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    Ok(download(
        body,
        "text/csv; charset=utf-8",
        &format!("mbusio-datenpunkte-{file}.csv"),
    ))
}

#[derive(Deserialize)]
pub(super) struct CsvImportReq {
    csv: String,
    /// false = nur zeigen, was sich ändert
    #[serde(default)]
    apply: bool,
}

pub(super) async fn datapoints_import(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<CsvImportReq>,
) -> ApiResult<crate::datapoint::ImportResult> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .dp_import(id, &r.csv, r.apply, lang)
            .map_err(bad_request)?,
    ))
}
