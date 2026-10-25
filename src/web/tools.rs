//! Werkzeuge: auslesen, Adresse und Baudrate ändern (mit Nachtrag in der Trefferliste), Reset,
//! Rohtelegramme, Analyse, Protokoll

use super::*;

#[derive(Deserialize)]
pub(super) struct ReadReq {
    address: String,
    /// Mit anderer Baudrate als der Bus lesen (Treffer aus dem Scan über alle Baudraten)
    #[serde(default)]
    baud: Option<u32>,
}

pub(super) async fn read(
    State(st): State<AppState>,
    Json(req): Json<ReadReq>,
) -> ApiResult<MeterReading> {
    let target = parse_target(&req.address)?;
    let line = st.line_id()?;
    let t = target.clone();
    let baud = req.baud;
    let result = with_bus(&st, move |m| m.at_baud(baud, |m| m.read_meter(&t))).await;
    // Ergebnis speichern (Verlauf) und in der Zählerliste vermerken, falls der Zähler dort steht
    match &result {
        Ok(r) => st.record(line, &target, Ok(r)),
        Err(ApiError(StatusCode::BAD_GATEWAY, e)) => st.record(line, &target, Err(e.clone())),
        Err(_) => {}
    }
    Ok(Json(result?))
}

#[derive(Deserialize)]
pub(super) struct SetAddressReq {
    address: String,
    new_address: u8,
    /// Zähler läuft mit anderer Baudrate als der Bus
    #[serde(default)]
    from_baud: Option<u32>,
}

/// Führt `f` mit der Baudrate `baud` aus (falls angegeben) und stellt danach die alte wieder her.
/// Was an einem Treffer geändert wurde
#[derive(Clone, Copy)]
pub(super) enum HitUpdate {
    Pa(u8),
    Baud(u32),
}

/// Änderung am Zähler auch in der Trefferliste nachtragen (laufender Job und letzter Scan der
/// Linie in der Datenbank), damit sie nach dem Neuladen der Seite oder auf einem zweiten Gerät
/// stimmt. Liefert Name (ID) und alten Wert des Zählers fürs Änderungsprotokoll.
pub(super) fn update_hits(
    st: &AppState,
    line: i64,
    target: &Target,
    from_baud: Option<u32>,
    upd: HitUpdate,
) -> Option<(String, String)> {
    let bus = st.conn_info()?.baud;
    let baud = from_baud.unwrap_or(bus);
    let matches = |status: &str, pa: Option<u8>, sa: Option<&str>, hb: Option<u32>| {
        hb == Some(baud)
            && match target {
                Target::Primary(a) => status == "ok" && pa == Some(*a),
                Target::Secondary(mask) => sa.is_some_and(|s| mask_matches(mask, s)),
            }
    };
    let mut found = None;
    {
        let mut job = st.job.lock().unwrap();
        if job.kind != "read_all" {
            for h in job.hits.iter_mut() {
                if !matches(
                    &h.status,
                    h.primary_address,
                    h.secondary_address.as_deref(),
                    h.baud,
                ) {
                    continue;
                }
                let old = match upd {
                    HitUpdate::Pa(_) => h.primary_address.map(|a| a.to_string()),
                    HitUpdate::Baud(_) => h.baud.map(|b| b.to_string()),
                };
                found.get_or_insert((
                    h.id.clone().unwrap_or_else(|| target.label()),
                    old.unwrap_or_default(),
                ));
                match upd {
                    HitUpdate::Pa(a) => h.primary_address = Some(a),
                    HitUpdate::Baud(b) => h.baud = Some(b),
                }
            }
        }
    }
    let mut db = st.db.lock().unwrap();
    if let Ok(Some(mut scan)) = db.last_scan(line) {
        let mut changed = false;
        for h in scan["hits"].as_array_mut().into_iter().flatten() {
            let pa = h["primary_address"].as_u64().map(|a| a as u8);
            let hb = h["baud"].as_u64().map(|b| b as u32);
            if !matches(
                h["status"].as_str().unwrap_or(""),
                pa,
                h["secondary_address"].as_str(),
                hb,
            ) {
                continue;
            }
            let old = match upd {
                HitUpdate::Pa(_) => pa.map(|a| a.to_string()),
                HitUpdate::Baud(_) => hb.map(|b| b.to_string()),
            };
            found.get_or_insert((
                h["id"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| target.label()),
                old.unwrap_or_default(),
            ));
            match upd {
                HitUpdate::Pa(a) => h["primary_address"] = json!(a),
                HitUpdate::Baud(b) => h["baud"] = json!(b),
            }
            changed = true;
        }
        if changed {
            let _ = db.replace_last_scan(line, &scan);
        }
    }
    found
}

/// Zählerliste nachführen; klappt das nicht (z.B. PA schon von einem anderen Eintrag belegt), steht
/// es im Protokoll, die Änderung am Zähler selbst ist trotzdem gemacht
fn follow_list(st: &AppState, line: i64, target: &Target, pa: Option<u8>, baud: Option<u32>) {
    if let Err(e) = st
        .db
        .lock()
        .unwrap()
        .follow_meter_change(line, target, pa, baud)
    {
        st.log.push(
            "INFO",
            format!("{}: {e:#}", tr("Zählerliste", "Meter list")),
        );
    }
}

pub(super) async fn set_address(
    State(st): State<AppState>,
    Json(req): Json<SetAddressReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let new = req.new_address;
    let line = st.line_id()?;
    let t = target.clone();
    with_bus(&st, move |m| {
        m.at_baud(req.from_baud, |m| m.set_primary(&t, new))
    })
    .await?;
    let (name, old) = update_hits(&st, line, &target, req.from_baud, HitUpdate::Pa(new))
        .unwrap_or_else(|| (target.label(), "?".into()));
    follow_list(&st, line, &target, Some(new), None);
    let text = format!(
        "{name}: {} {old} → {new}",
        tr("Primäradresse", "primary address")
    );
    let _ = st.db.lock().unwrap().add_change(line, &text);
    Ok(Json(json!({ "ok": true, "new_address": new })))
}

#[derive(Deserialize)]
pub(super) struct SwitchBaudReq {
    address: String,
    baud: u32,
    /// Baudrate, mit der der Zähler heute läuft (sonst die des Busses)
    #[serde(default)]
    from_baud: Option<u32>,
}

pub(super) async fn switch_baud(
    State(st): State<AppState>,
    Json(req): Json<SwitchBaudReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let baud = req.baud;
    let line = st.line_id()?;
    let t = target.clone();
    with_bus(&st, move |m| {
        m.at_baud(req.from_baud, |m| m.switch_baud(&t, baud))
    })
    .await?;
    let (name, old) = update_hits(&st, line, &target, req.from_baud, HitUpdate::Baud(baud))
        .unwrap_or_else(|| (target.label(), "?".into()));
    follow_list(&st, line, &target, None, Some(baud));
    let text = format!("{name}: {} {old} → {baud} Bd", tr("Baudrate", "baud rate"));
    let _ = st.db.lock().unwrap().add_change(line, &text);
    Ok(Json(json!({ "ok": true, "baud": baud })))
}

pub(super) async fn detect_baud(
    State(st): State<AppState>,
    Json(req): Json<ReadReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let bauds = with_bus(&st, move |m| m.detect_baud(&target)).await?;
    Ok(Json(json!({ "bauds": bauds })))
}

#[derive(Deserialize)]
pub(super) struct ResetReq {
    address: String,
    #[serde(default)]
    subcode: Option<u8>,
    #[serde(default)]
    from_baud: Option<u32>,
}

pub(super) async fn app_reset(
    State(st): State<AppState>,
    Json(req): Json<ResetReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let sub = req.subcode;
    with_bus(&st, move |m| {
        m.at_baud(req.from_baud, |m| m.application_reset(&target, sub))
    })
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct RawReq {
    hex: String,
}

/// Hex-Text in Bytes, Leerzeichen und Trennzeichen erlaubt.
pub fn parse_hex(text: &str) -> anyhow::Result<Vec<u8>> {
    let digits: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let junk = text
        .chars()
        .any(|c| !c.is_ascii_hexdigit() && !c.is_whitespace() && !matches!(c, ',' | ':' | '-'));
    if junk || digits.is_empty() || !digits.len().is_multiple_of(2) {
        anyhow::bail!(tr(
            "Bitte Bytes als Hex eingeben, z.B. 10 40 01 41 16",
            "Please enter bytes as hex, e.g. 10 40 01 41 16"
        ));
    }
    Ok((0..digits.len())
        .step_by(2)
        .map(|k| u8::from_str_radix(&digits[k..k + 2], 16).unwrap())
        .collect())
}

/// Telegramm erklären, ohne etwas zu senden
pub(super) async fn analyze(Json(req): Json<RawReq>) -> ApiResult<crate::mbus::analyze::Analysis> {
    let bytes = parse_hex(&req.hex).map_err(bad_request)?;
    Ok(Json(crate::mbus::analyze::analyze(&bytes)))
}

pub(super) async fn raw_send(
    State(st): State<AppState>,
    Json(req): Json<RawReq>,
) -> ApiResult<serde_json::Value> {
    let bytes = parse_hex(&req.hex).map_err(bad_request)?;
    let reply = with_bus(&st, move |m| m.raw_send(&bytes)).await?;
    Ok(Json(match reply {
        Reply::None => json!({ "kind": "none" }),
        Reply::Collision(b) => json!({ "kind": "collision", "hex": hex(&b) }),
        Reply::Frame(f) => {
            // Datentelegramm gleich decodieren, wenn möglich
            let decoded = match &f {
                Frame::Long { ci, data, .. } => decode_rsp_ud(*ci, data)
                    .ok()
                    .map(|t| json!({ "header": t.header, "records": t.records })),
                _ => None,
            };
            let kind = if f == Frame::Ack { "ack" } else { "frame" };
            json!({ "kind": kind, "hex": hex(&f.encode()), "decoded": decoded })
        }
    }))
}

#[derive(Deserialize)]
pub(super) struct LogQuery {
    #[serde(default)]
    since: u64,
}

pub(super) async fn log(
    State(st): State<AppState>,
    Query(q): Query<LogQuery>,
) -> Json<serde_json::Value> {
    Json(json!(st.log.since(q.since)))
}

pub(super) async fn labels() -> Json<serde_json::Map<String, serde_json::Value>> {
    Json(i18n::labels(i18n::current()))
}

pub(super) async fn clear_log(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.log.clear();
    Json(json!({ "ok": true }))
}
