//! Zählerliste: anlegen, auslesen, Verlauf, Import/Export, Liste füllen, Zählertausch

use super::*;

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
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<MeterReading> {
    let meter = st.db.lock().unwrap().meter(id).map_err(bad_request)?;
    let line = meter.line_id;
    let target = parse_target(&meter.address)?;
    let result = with_bus(&st, lang_of(&headers), move |m| {
        let home = m.baud();
        let r = read_listed(m, &meter, home);
        m.set_baud(home)?;
        r
    })
    .await;
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

pub(super) async fn meters_read_all(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<JobStatus> {
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
    start_job(&st, "read_all", lang_of(&headers), move |m, progress| {
        let total = meters.len() as u32;
        let home = m.baud();
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
            match read_listed(m, meter, home) {
                Ok((target, r)) => s.record(line, &target, Ok(&r)),
                Err(e) => {
                    if let Ok(target) = Target::parse(&meter.address) {
                        s.record(line, &target, Err(format!("{e:#}")));
                    }
                }
            }
        }
        m.set_baud(home)?;
        progress(Progress::Step {
            done: total,
            total,
            message: tr("Fertig", "Done"),
        });
        Ok(())
    })
}

pub(super) async fn meters_export(State(st): State<AppState>) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let body = serde_json::to_string_pretty(&json!({ "meters": db.meters(db.active_line()?.id)? }))
        .unwrap_or_default();
    Ok(download(body, "application/json", "mbusio-meters.json"))
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
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<ExchangeReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    let meter = st.db.lock().unwrap().meter(id).map_err(bad_request)?;
    parse_target(&r.address)?;
    let probe = Meter {
        address: r.address.clone(),
        baud: r.baud.or(meter.baud),
        ..Default::default()
    };
    let (_, reading) = with_bus(&st, lang, move |m| {
        let home = m.baud();
        let res = read_listed(m, &probe, home);
        m.set_baud(home)?;
        res
    })
    .await?;
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
    headers: HeaderMap,
    Json(r): Json<ListImportReq>,
) -> ApiResult<crate::meterlist::ListImport> {
    i18n::set(lang_of(&headers));
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
    headers: HeaderMap,
    Json(r): Json<NamesReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    let list = db.auto_names(
        line,
        &r.pattern,
        r.ids.as_deref(),
        r.only_empty,
        r.apply,
        lang,
    )?;
    Ok(Json(
        json!({ "ok": true, "names": list, "applied": r.apply }),
    ))
}

pub(super) async fn meters_csv(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LangQuery>,
) -> Result<Response, ApiError> {
    let lang = q
        .lang
        .as_deref()
        .map(Lang::parse)
        .unwrap_or_else(|| lang_of(&headers));
    i18n::set(lang);
    let db = st.db.lock().unwrap();
    let line = db.active_line()?;
    let body = format!("\u{feff}{}", db.meters_csv(line.id, lang)?);
    let file: String = line
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    Ok(download(
        body,
        "text/csv; charset=utf-8",
        &format!("mbusio-zaehler-{file}.csv"),
    ))
}

#[derive(Deserialize)]
pub(super) struct ExportQuery {
    #[serde(default)]
    format: String,
    #[serde(default)]
    lang: String,
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
            export::csv(&readings, Lang::parse(&q.lang)),
            "text/csv; charset=utf-8",
            "csv",
        ),
    };
    Ok(download(body, mime, &format!("mbusio-{stamp}.{ext}")))
}
