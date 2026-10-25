//! Zählerliste: anlegen, auslesen, Verlauf, Import/Export, Liste füllen, Zählertausch

use super::*;
use crate::line::read_listed;

/// Zählerliste der aktiven Linie, je Zähler mit seinem Plan (gültiger Plan, nächster Termin …)
pub(super) async fn meters_list(State(st): State<AppState>) -> ApiResult<Vec<serde_json::Value>> {
    let db = st.db.lock().unwrap();
    let line = db.active_line()?;
    let project_plan = db.project_plan_of_line(line.id)?;
    let now = clock::now();
    let out = db
        .meters(line.id)?
        .into_iter()
        .map(|m| {
            let info = Db::meter_plan_info(&m, line.plan.as_deref(), &project_plan, now);
            let mut v = serde_json::to_value(&m).unwrap_or_default();
            v["plan_info"] = info;
            v
        })
        .collect();
    Ok(Json(out))
}

pub(super) async fn meters_add(
    State(st): State<AppState>,
    Json(m): Json<Meter>,
) -> ApiResult<Meter> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    Ok(Json(db.add_meter(line, m).map_err(bad_request)?))
}

pub(super) async fn meters_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(m): Json<Meter>,
) -> ApiResult<Meter> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_meter(id, m)
            .map_err(bad_request)?,
    ))
}

pub(super) async fn meters_clear(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    let n = db.clear_meters(line).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

pub(super) async fn meters_remove(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .remove_meter(id)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn meters_read_one(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<MeterReading> {
    let meter = st.db.lock().unwrap().meter(id).map_err(bad_request)?;
    let line = meter.line_id;
    let target = parse_target(&meter.address)?;
    let result = with_bus(&st, move |m| read_listed(m, &meter)).await;
    match result {
        Ok((_, r)) => {
            st.record(line, &target, Ok(&r));
            Ok(Json(r))
        }
        Err(e) => {
            // nur Busfehler vermerken, nicht "belegt" oder "nicht verbunden"
            if e.0 == StatusCode::BAD_GATEWAY {
                st.record(line, &target, Err(e.1.clone()));
            }
            Err(e)
        }
    }
}

/// Verlauf eines Zählers: je Auslesung die aktuellen Zahlenwerte (Speicher 0), als Reihen nach
/// Messgrösse, Einheit und Tarif, damit die Oberfläche eine davon als Linie zeichnen kann
pub(super) async fn meter_history(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    let meter = db.meter(id).map_err(bad_request)?;
    let points: Vec<serde_json::Value> = db
        .meter_history(id, 2000)?
        .into_iter()
        .map(|r| {
            let values: Vec<serde_json::Value> = r
                .records
                .iter()
                .filter(|x| x.storage == 0 && x.function == "instantaneous" && x.value.is_number())
                .map(|x| {
                    json!({
                        "key": format!("{}|{}|{}", x.quantity, x.unit, x.tariff),
                        "quantity": x.quantity,
                        "unit": x.unit,
                        "tariff": x.tariff,
                        "value": x.value,
                        "text": x.value_text,
                    })
                })
                .collect();
            json!({ "timestamp": r.timestamp, "time_unsure": r.time_unsure, "values": values })
        })
        .collect();
    Ok(Json(json!({ "meter": meter, "points": points })))
}

pub(super) async fn meters_read_all(State(st): State<AppState>) -> ApiResult<JobStatus> {
    let (line, meters) = {
        let db = st.db.lock().unwrap();
        let line = db.active_line()?.id;
        (line, db.meters(line)?)
    };
    if meters.is_empty() {
        return Err(bad_request(tr(
            "Die Zählerliste ist leer",
            "The meter list is empty",
        )));
    }
    let s = st.clone();
    start_job(
        &st,
        "read_all",
        String::new(),
        Vec::new(),
        move |m, progress| {
            let total = meters.len() as u32;
            for (k, meter) in meters.iter().enumerate() {
                let name = if meter.name.is_empty() {
                    &meter.address
                } else {
                    &meter.name
                };
                if !progress(Progress::Step {
                    done: k as u32,
                    total,
                    message: name.clone(),
                }) {
                    break;
                }
                match read_listed(m, meter) {
                    Ok((target, r)) => s.record(line, &target, Ok(&r)),
                    Err(e) => {
                        if let Ok(target) = Target::parse(&meter.address) {
                            s.record(line, &target, Err(format!("{e:#}")));
                        }
                    }
                }
            }
            progress(Progress::Step {
                done: total,
                total,
                message: tr("Fertig", "Done"),
            });
            Ok(())
        },
    )
}

#[derive(Deserialize)]
pub(super) struct ImportReq {
    meters: Vec<Meter>,
}

pub(super) async fn meters_import(
    State(st): State<AppState>,
    Json(req): Json<ImportReq>,
) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    let n = db.import_meters(line, req.meters).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "added": n })))
}

pub(super) async fn readings(State(st): State<AppState>) -> ApiResult<Vec<MeterReading>> {
    let db = st.db.lock().unwrap();
    Ok(Json(db.readings(db.active_line()?.id, MAX_READINGS)?))
}

pub(super) async fn clear_readings(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    db.clear_readings(line)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct ExchangeReq {
    /// Adresse des neuen Zählers (PA, ID oder Sekundäradresse)
    address: String,
    /// eigene Baudrate des neuen Zählers
    #[serde(default)]
    baud: Option<u32>,
}

/// Zähler getauscht: neuen Zähler auslesen, Zählerstände fortführen, Adresse übernehmen
pub(super) async fn meter_exchange(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<ExchangeReq>,
) -> ApiResult<serde_json::Value> {
    let meter = st.db.lock().unwrap().meter(id).map_err(bad_request)?;
    parse_target(&r.address)?;
    let probe = Meter {
        address: r.address.clone(),
        baud: r.baud.or(meter.baud),
        ..Default::default()
    };
    let (_, reading) = with_bus(&st, move |m| read_listed(m, &probe)).await?;
    let (lines, notes) = {
        let mut db = st.db.lock().unwrap();
        let lines = db
            .exchange_meter(id, &r.address, &reading)
            .map_err(bad_request)?;
        (lines, std::mem::take(&mut db.notes))
    };
    for n in notes.into_iter().chain(lines.iter().cloned()) {
        st.log.push("INFO", n);
    }
    Ok(Json(
        json!({ "ok": true, "details": lines, "reading": reading }),
    ))
}

#[derive(Deserialize)]
pub(super) struct ListImportReq {
    rows: Vec<crate::meterlist::ListRow>,
    #[serde(default)]
    apply: bool,
}

/// Liste vom Kunden/Installateur: Zähler über die ID anlegen bzw. benennen (zuerst Vorschau)
pub(super) async fn meters_import_list(
    State(st): State<AppState>,
    Json(r): Json<ListImportReq>,
) -> ApiResult<crate::changeset::ChangeSet> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    Ok(Json(
        db.import_list(line, &r.rows, r.apply)
            .map_err(bad_request)?,
    ))
}

#[derive(Deserialize)]
pub(super) struct NamesReq {
    pattern: String,
    /// nur diese Zähler; leer = alle
    #[serde(default)]
    ids: Option<Vec<i64>>,
    /// nur Zähler ohne Namen
    #[serde(default)]
    only_empty: bool,
    #[serde(default)]
    apply: bool,
}

pub(super) async fn meters_names(
    State(st): State<AppState>,
    Json(r): Json<NamesReq>,
) -> ApiResult<crate::changeset::ChangeSet> {
    let lang = i18n::current();
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    Ok(Json(db.auto_names(
        line,
        &r.pattern,
        r.ids.as_deref(),
        r.only_empty,
        r.apply,
        lang,
    )?))
}

pub(super) async fn meters_csv(State(st): State<AppState>) -> Result<Response, ApiError> {
    let lang = i18n::current();
    let db = st.db.lock().unwrap();
    let line = db.active_line()?;
    Ok(csv_download(
        db.meters_csv(line.id, lang)?,
        "zaehler",
        &line.name,
    ))
}

#[derive(Deserialize)]
pub(super) struct ExportQuery {
    #[serde(default)]
    format: String,
}

pub(super) async fn export_readings(
    State(st): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let readings = {
        let db = st.db.lock().unwrap();
        db.readings(db.active_line()?.id, MAX_READINGS)?
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let (body, mime, ext) = match q.format.as_str() {
        "json" => (export::json(&readings), "application/json", "json"),
        _ => (
            export::csv(&readings, i18n::current()),
            "text/csv; charset=utf-8",
            "csv",
        ),
    };
    Ok(download(body, mime, &format!("mbusio-{stamp}.{ext}")))
}
