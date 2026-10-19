//! Verbindung, Scan, Auslesen, Adresse/Baudrate ändern, Rohtelegramme, Log

use super::*;

pub(super) async fn ports() -> Json<Vec<link::PortInfo>> {
    Json(
        tokio::task::spawn_blocking(link::list_ports)
            .await
            .unwrap_or_default(),
    )
}

pub(super) async fn status(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let line = st.service().ok().map(|s| s.status());
    let plans = st.planner.runs();
    let conn = line.as_ref().and_then(|l| l.connection.clone());
    let running = st.job.lock().unwrap().running;
    let db = st.db.lock().unwrap();
    let active = db.active()?;
    Ok(Json(json!({
        "connected": conn.is_some(),
        "connection": conn,
        "job_running": running,
        "readings": db.reading_count(active.line.id)?,
        "version": env!("CARGO_PKG_VERSION"),
        "token_required": st.token.is_some(),
        "meters_file": db.path().map(|p| p.display().to_string()),
        "active": active,
        // Zustand des Linien-Dienstes: arbeitet, Warteschlange, Verbindungsfehler
        "line": line,
        // laufende geplante Auslesungen je Linie
        "plans": plans,
        "plan_active": st.planner.any_planned(),
        "held": st.planner.is_held(active.line.id),
        "time": clock::state(),
        "disk": st.planner.disk(),
        "mqtt": mqtt_status(&st, &db),
        "now": clock::stamp(),
        // läuft weiter ohne Anmeldung (Windows-Dienst, systemd); sonst Hinweis im Dauerbetrieb
        "background": crate::service::background(),
        "os": std::env::consts::OS,
    })))
}

#[derive(Deserialize)]
pub(super) struct ConnectReq {
    port: String,
    baud: u32,
    #[serde(default)]
    timeout_ms: u64,
}

pub(super) async fn connect(
    State(st): State<AppState>,
    Json(req): Json<ConnectReq>,
) -> ApiResult<serde_json::Value> {
    if st.job.lock().unwrap().running {
        return Err(busy());
    }
    let line = st.line_id()?;
    let conf = LineConf {
        connection: req.port.clone(),
        baud: req.baud,
        timeout_ms: req.timeout_ms,
    };
    let s = st.clone();
    blocking(move || s.connect_line(line, conf))
        .await?
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    // erfolgreiche Verbindung gehört zur aktiven Linie: beim nächsten Mal wieder so verbinden
    st.db
        .lock()
        .unwrap()
        .set_line_connection(line, &req.port, req.baud, req.timeout_ms)?;
    Ok(Json(json!({ "ok": true, "connection": st.conn_info() })))
}

pub(super) async fn disconnect(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let line = st.line_id()?;
    // von Hand getrennt (z.B. um den Anschluss für ein anderes Programm frei zu machen):
    // der Ausleseplan verbindet nicht von selbst wieder, bis man wieder verbindet
    st.planner.hold(line);
    let planned = st.planner.is_planned(line);
    st.stop_line(line);
    Ok(Json(json!({ "ok": true, "plan_paused": planned })))
}

pub(super) fn start_job<F>(
    st: &AppState,
    kind: &str,
    scope: String,
    hits: Vec<ScanHit>,
    work: F,
) -> ApiResult<JobStatus>
where
    F: FnOnce(&mut Master, &mut dyn FnMut(Progress) -> bool) -> anyhow::Result<()> + Send + 'static,
{
    let svc = st.service()?;
    {
        let mut job = st.job.lock().unwrap();
        if job.running {
            return Err(busy());
        }
        *job = JobStatus {
            kind: kind.into(),
            scope,
            hits,
            running: true,
            message: String::new(),
            ..Default::default()
        };
    }
    st.cancel.store(false, Ordering::SeqCst);
    st.log
        .push("INFO", format!("{} {kind}", tr("Starte", "Starting")));
    // Ergebnis gehört zur Linie, die beim Start aktiv war
    let line = st.line_id()?;
    let lang = i18n::current();
    let s = st.clone();
    // als Auftrag an den Linien-Dienst; der Fortschritt läuft über job
    svc.submit(Box::new(move |m| {
        i18n::set(lang);
        let result = match m {
            Err(e) => Err(anyhow::anyhow!(e)),
            Ok(master) => {
                let mut on_progress = |p: Progress| {
                    let mut job = s.job.lock().unwrap();
                    match p {
                        Progress::Step {
                            done,
                            total,
                            message,
                        } => {
                            job.done = done;
                            job.total = total;
                            job.message = message;
                        }
                        Progress::Hit(h) => job.hits.push(h),
                        Progress::Stopped { mask, baud } => {
                            job.resume = Some(json!({ "mask": mask, "baud": baud }))
                        }
                    }
                    !s.cancel.load(Ordering::SeqCst)
                };
                work(master, &mut on_progress)
            }
        };
        let link_ok = !matches!(&result, Err(e) if is_link_error(e));
        let mut job = s.job.lock().unwrap();
        job.running = false;
        if let Err(e) = result {
            job.error = Some(format!("{e:#}"));
        } else if s.cancel.load(Ordering::SeqCst) {
            job.message = tr("Abgebrochen", "Cancelled");
        }
        s.log.push(
            "INFO",
            format!(
                "{} {}: {}",
                job.kind,
                tr("beendet, Treffer", "finished, hits"),
                job.hits.len()
            ),
        );
        if job.kind != "read_all" {
            // nur ein vollständiger Scan wird verglichen und für den nächsten Vergleich gemerkt
            let complete = job.error.is_none() && !s.cancel.load(Ordering::SeqCst);
            let mut db = s.db.lock().unwrap();
            if complete {
                job.previous = db.previous_scan(line, &job.kind, &job.scope).ok().flatten();
            }
            let mut value = serde_json::to_value(&*job).unwrap_or_default();
            // „PA ändern“ führt den gemerkten Scan nur nach, wenn dies ein vollständiger Scan war
            value["complete"] = json!(complete);
            if let Err(e) = db.set_last_scan(line, &value, complete) {
                s.log.push("INFO", format!("DB: {e:#}"));
            }
        }
        link_ok
    }));
    Ok(Json(st.job.lock().unwrap().clone()))
}

#[derive(Deserialize)]
pub(super) struct PrimaryScanReq {
    #[serde(default)]
    from: u8,
    #[serde(default = "default_to")]
    to: u8,
    #[serde(default = "default_retries")]
    retries: u32,
    /// Baudraten für den Scan; leer = eingestellte Baudrate
    #[serde(default)]
    bauds: Vec<u32>,
}

#[derive(Deserialize)]
pub(super) struct SecondaryScanReq {
    #[serde(default)]
    bauds: Vec<u32>,
    /// nur IDs, die so beginnen (0–7 Ziffern)
    #[serde(default)]
    prefix: String,
    /// abgebrochenen Scan fortsetzen: Stelle {mask, baud} aus dem Job
    #[serde(default)]
    resume: Option<Resume>,
}

#[derive(Deserialize)]
pub(super) struct Resume {
    mask: String,
    baud: Option<u32>,
}

pub(super) fn bauds_of(list: Vec<u32>) -> Bauds {
    if list.is_empty() {
        Bauds::Current
    } else {
        Bauds::List(list)
    }
}

pub(super) fn default_to() -> u8 {
    250
}

pub(super) fn default_retries() -> u32 {
    1
}

pub(super) async fn scan_primary(
    State(st): State<AppState>,
    Json(req): Json<PrimaryScanReq>,
) -> ApiResult<JobStatus> {
    let (from, to) = (req.from.min(250), req.to.min(250));
    if from > to {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            tr(
                "Startadresse grösser als Endadresse",
                "Start address greater than end address",
            ),
        ));
    }
    let retries = req.retries.min(5);
    let scope = format!("{from}-{to} {:?}", req.bauds);
    start_job(&st, "primary", scope, Vec::new(), move |m, p| {
        m.scan_with(bauds_of(req.bauds), p, &mut |m, p| {
            m.scan_primary(from, to, retries, p)
        })
    })
}

pub(super) async fn scan_secondary(
    State(st): State<AppState>,
    Json(req): Json<SecondaryScanReq>,
) -> ApiResult<JobStatus> {
    let prefix = req.prefix.trim().to_string();
    let scope = format!("{:?} {prefix}", req.bauds);
    // Weitersuchen: bisherige Treffer übernehmen, Baudraten vor der Stelle des Abbruchs überspringen
    let hits = match &req.resume {
        Some(_) => {
            let db = st.db.lock().unwrap();
            db.last_scan(db.active_line()?.id)?
                .filter(|s| s["kind"] == "secondary")
                .and_then(|s| serde_json::from_value(s["hits"].clone()).ok())
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
    let mut resume = req.resume;
    start_job(&st, "secondary", scope, hits, move |m, p| {
        m.scan_with(bauds_of(req.bauds), p, &mut |m, p| match &resume {
            Some(r) if r.baud.is_some_and(|b| b != m.baud()) => Ok(()),
            Some(r) => {
                let mask = r.mask.clone();
                resume = None;
                m.scan_secondary_from(&prefix, Some(&mask), p)
            }
            None => m.scan_secondary_from(&prefix, None, p),
        })
    })
}

#[derive(Deserialize)]
pub(super) struct FreeAddressReq {
    #[serde(default)]
    from: u8,
    /// bekannte belegte Adressen
    #[serde(default)]
    skip: Vec<u8>,
}

pub(super) async fn free_address(
    State(st): State<AppState>,
    Json(req): Json<FreeAddressReq>,
) -> ApiResult<serde_json::Value> {
    let address = with_bus(&st, move |m| m.free_primary(req.from, &req.skip)).await?;
    Ok(Json(json!({ "address": address })))
}

#[derive(Deserialize, Default)]
pub(super) struct BusBaudsReq {
    /// leer = alle Standard-Baudraten
    #[serde(default)]
    bauds: Vec<u32>,
}

pub(super) async fn bus_bauds(
    State(st): State<AppState>,
    Json(req): Json<BusBaudsReq>,
) -> ApiResult<serde_json::Value> {
    let bauds = with_bus(&st, move |m| m.bus_bauds(&req.bauds)).await?;
    Ok(Json(json!({ "bauds": bauds })))
}

/// Letzter abgeschlossener Scan (für die Trefferliste nach dem Neuladen)
/// Letzter Scan der aktiven Linie (für die Trefferliste nach dem Neuladen)
pub(super) async fn last_scan(State(st): State<AppState>) -> ApiResult<Option<serde_json::Value>> {
    let db = st.db.lock().unwrap();
    Ok(Json(db.last_scan(db.active_line()?.id)?))
}

pub(super) async fn job(State(st): State<AppState>) -> Json<JobStatus> {
    Json(st.job.lock().unwrap().clone())
}

pub(super) async fn cancel(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.cancel.store(true, Ordering::SeqCst);
    Json(json!({ "ok": true }))
}

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
