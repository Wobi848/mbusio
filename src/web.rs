//! HTTP-API und eingebettete Weboberfläche.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::export;
use crate::i18n::{self, tr, Lang};
use crate::link;
use crate::mbus::decode::decode_rsp_ud;
use crate::mbus::frame::{hex, Frame};
use crate::mbus::master::{
    mask_matches, Bauds, FrameLog, Master, MeterReading, Progress, Reply, ScanHit, Target,
};
use crate::db::{Db, Gateway, Line, Meter, Project};

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

/// Höchstens so viele Auslesungen einer Linie an die Oberfläche bzw. in den Export
const MAX_READINGS: usize = 5000;

#[derive(Clone, Serialize)]
pub struct ConnInfo {
    pub port: String,
    pub baud: u32,
    pub timeout_ms: u64,
    /// Timeout wird aus der Baudrate berechnet (für die Dauer-Schätzung im Browser)
    pub timeout_auto: bool,
}

#[derive(Clone, Default, Serialize)]
pub struct JobStatus {
    pub kind: String,
    pub running: bool,
    pub done: u32,
    pub total: u32,
    pub message: String,
    pub hits: Vec<ScanHit>,
    pub error: Option<String>,
}

pub struct Shared {
    bus: Mutex<Option<Master>>,
    conn: Mutex<Option<ConnInfo>>,
    job: Mutex<JobStatus>,
    cancel: AtomicBool,
    log: Arc<FrameLog>,
    /// Projekte, Linien, Zähler, Auslesungen; Zählerliste und Scan gelten für die aktive Linie
    db: Mutex<Db>,
    /// Optionaler Zugriffsschlüssel für die API (bei LAN-Zugriff)
    token: Option<String>,
}

impl Shared {
    pub fn new(token: Option<String>, db: Db) -> Self {
        Shared {
            bus: Mutex::default(),
            conn: Mutex::default(),
            job: Mutex::default(),
            cancel: AtomicBool::default(),
            log: Arc::default(),
            db: Mutex::new(db),
            token,
        }
    }

    fn line_id(&self) -> Result<i64, ApiError> {
        Ok(self.db.lock().unwrap().active_line()?.id)
    }

    /// Auslesung speichern (Verlauf und Zählerliste der Linie); Speicherfehler nur ins Log
    fn record(&self, line: i64, target: &Target, result: Result<&MeterReading, String>) {
        if let Err(e) = self.db.lock().unwrap().record(line, target, result) {
            self.log.push("INFO", format!("DB: {e:#}"));
        }
    }
}

fn bad_request(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, format!("{e:#}"))
}

fn parse_target(address: &str) -> Result<Target, ApiError> {
    Target::parse(address).map_err(bad_request)
}

/// Sprache aus `X-Lang` oder `Accept-Language`.
fn lang_of(headers: &HeaderMap) -> Lang {
    headers
        .get("x-lang")
        .or_else(|| headers.get(header::ACCEPT_LANGUAGE))
        .and_then(|v| v.to_str().ok())
        .map(Lang::parse)
        .unwrap_or_default()
}

pub type AppState = Arc<Shared>;

pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError(StatusCode::BAD_GATEWAY, format!("{e:#}"))
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

fn busy() -> ApiError {
    ApiError(
        StatusCode::CONFLICT,
        tr(
            "Bus ist belegt (Scan läuft?)",
            "Bus is busy (scan running?)",
        ),
    )
}

fn not_connected() -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        tr("Nicht verbunden", "Not connected"),
    )
}

/// Führt `f` mit exklusivem Buszugriff in einem Blocking-Thread aus.
async fn with_bus<T, F>(state: &AppState, lang: Lang, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Master) -> anyhow::Result<T> + Send + 'static,
{
    let st = state.clone();
    tokio::task::spawn_blocking(move || {
        i18n::set(lang);
        let mut guard = match st.bus.try_lock() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return Err(busy()),
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
        };
        let master = guard.as_mut().ok_or_else(not_connected)?;
        f(master).map_err(ApiError::from)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/ports", get(ports))
        .route("/api/status", get(status))
        .route("/api/connect", post(connect))
        .route("/api/disconnect", post(disconnect))
        .route("/api/scan/primary", post(scan_primary))
        .route("/api/scan/secondary", post(scan_secondary))
        .route("/api/job", get(job))
        .route("/api/last-scan", get(last_scan))
        .route("/api/job/cancel", post(cancel))
        .route("/api/read", post(read))
        .route("/api/set-address", post(set_address))
        .route("/api/switch-baud", post(switch_baud))
        .route("/api/detect-baud", post(detect_baud))
        .route("/api/bus-bauds", post(bus_bauds))
        .route("/api/free-address", post(free_address))
        .route("/api/reset", post(app_reset))
        .route("/api/raw", post(raw_send))
        .route("/api/analyze", post(analyze))
        .route(
            "/api/meters",
            get(meters_list).post(meters_add).delete(meters_clear),
        )
        .route("/api/meters/{id}", put(meters_update).delete(meters_remove))
        .route("/api/meters/read-all", post(meters_read_all))
        .route("/api/meters/{id}/read", post(meters_read_one))
        .route("/api/meters/{id}/history", get(meter_history))
        .route("/api/meters/export", get(meters_export))
        .route("/api/meters/import", post(meters_import))
        .route("/api/readings", get(readings).delete(clear_readings))
        .route("/api/export", get(export_readings))
        .route("/api/log", get(log).delete(clear_log))
        .route("/api/labels", get(labels))
        .route("/api/projects", get(tree).post(project_create))
        .route("/api/projects/import", post(project_import))
        .route("/api/projects/{id}", put(project_update).delete(project_delete))
        .route("/api/projects/{id}/export", get(project_export))
        .route("/api/projects/{id}/changes", axum::routing::delete(changes_clear))
        .route("/api/projects/{id}/gateways", post(gateway_create))
        .route("/api/gateways/{id}", put(gateway_update).delete(gateway_delete))
        .route("/api/gateways/{id}/lines", post(line_create))
        .route("/api/lines/{id}", put(line_update).delete(line_delete))
        .route("/api/lines/{id}/activate", post(line_activate))
        .route("/api/lines/{id}/issues", put(line_issues))
        .route("/api/lines/{id}/addressing", put(line_addressing))
        .route("/api/lines/{id}/move", post(line_move))
        .route("/api/gateways/{id}/move", post(gateway_move))
        .route("/api/report", get(report))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .fallback(static_file)
        .with_state(state)
}

/// Prüft den Zugriffsschlüssel (Header `X-Token` oder `?token=`), falls einer gesetzt ist.
async fn auth(State(st): State<AppState>, req: Request, next: Next) -> Response {
    i18n::set(lang_of(req.headers()));
    if let Some(token) = &st.token {
        let header_ok =
            req.headers().get("x-token").and_then(|v| v.to_str().ok()) == Some(token.as_str());
        let query_ok = req.uri().query().is_some_and(|q| {
            q.split('&')
                .any(|kv| kv.strip_prefix("token=") == Some(token.as_str()))
        });
        if !header_ok && !query_ok {
            return ApiError(
                StatusCode::UNAUTHORIZED,
                tr(
                    "Zugriffsschlüssel fehlt oder ist falsch",
                    "Access token missing or wrong",
                ),
            )
            .into_response();
        }
    }
    next.run(req).await
}

pub fn connect_blocking(
    state: &Shared,
    port: &str,
    baud: u32,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let link = link::open(port, baud)?;
    let master = Master::new(link, baud, timeout_ms, state.log.clone());
    let effective = master.timeout.as_millis() as u64;
    let mut bus = state
        .bus
        .try_lock()
        .map_err(|_| anyhow::anyhow!(tr("Bus ist belegt", "Bus is busy")))?;
    *bus = Some(master);
    *state.conn.lock().unwrap() = Some(ConnInfo {
        port: port.into(),
        baud,
        timeout_ms: effective,
        timeout_auto: timeout_ms == 0,
    });
    state.log.push(
        "INFO",
        format!(
            "{} {port}, {baud} Bd, 8E1, Timeout {effective} ms",
            tr("Verbunden mit", "Connected to")
        ),
    );
    Ok(())
}

async fn ports() -> Json<Vec<String>> {
    Json(
        tokio::task::spawn_blocking(link::list_ports)
            .await
            .unwrap_or_default(),
    )
}

async fn status(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let conn = st.conn.lock().unwrap().clone();
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
    })))
}

#[derive(Deserialize)]
struct ConnectReq {
    port: String,
    baud: u32,
    #[serde(default)]
    timeout_ms: u64,
}

async fn connect(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ConnectReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    if st.job.lock().unwrap().running {
        return Err(busy());
    }
    let s = st.clone();
    tokio::task::spawn_blocking(move || {
        i18n::set(lang);
        connect_blocking(&s, &req.port, req.baud, req.timeout_ms)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    // erfolgreiche Verbindung gehört zur aktiven Linie: beim nächsten Mal wieder so verbinden
    if let Some(c) = st.conn.lock().unwrap().clone() {
        let mut db = st.db.lock().unwrap();
        let line = db.active_line()?.id;
        db.set_line_connection(line, &c.port, c.baud, req_timeout(&c))?;
    }
    Ok(Json(
        json!({ "ok": true, "connection": st.conn.lock().unwrap().clone() }),
    ))
}

/// Timeout wie eingestellt speichern: 0 = automatisch
fn req_timeout(c: &ConnInfo) -> u64 {
    if c.timeout_auto {
        0
    } else {
        c.timeout_ms
    }
}

async fn disconnect(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut bus = st.bus.try_lock().map_err(|_| busy())?;
    *bus = None;
    *st.conn.lock().unwrap() = None;
    st.log
        .push("INFO", tr("Verbindung getrennt", "Disconnected"));
    Ok(Json(json!({ "ok": true })))
}

fn start_job<F>(st: &AppState, kind: &str, lang: Lang, work: F) -> ApiResult<JobStatus>
where
    F: FnOnce(&mut Master, &mut dyn FnMut(Progress) -> bool) -> anyhow::Result<()> + Send + 'static,
{
    if st.conn.lock().unwrap().is_none() {
        return Err(not_connected());
    }
    {
        let mut job = st.job.lock().unwrap();
        if job.running {
            return Err(busy());
        }
        *job = JobStatus {
            kind: kind.into(),
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
    let s = st.clone();
    std::thread::spawn(move || {
        i18n::set(lang);
        let result = {
            let mut guard = s.bus.lock().unwrap_or_else(|p| p.into_inner());
            match guard.as_mut() {
                None => Err(anyhow::anyhow!(tr("Nicht verbunden", "Not connected"))),
                Some(master) => {
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
                        }
                        !s.cancel.load(Ordering::SeqCst)
                    };
                    work(master, &mut on_progress)
                }
            }
        };
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
            let value = serde_json::to_value(&*job).unwrap_or_default();
            if let Err(e) = s.db.lock().unwrap().set_last_scan(line, &value) {
                s.log.push("INFO", format!("DB: {e:#}"));
            }
        }
    });
    Ok(Json(st.job.lock().unwrap().clone()))
}

#[derive(Deserialize)]
struct PrimaryScanReq {
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
struct SecondaryScanReq {
    #[serde(default)]
    bauds: Vec<u32>,
}

fn bauds_of(list: Vec<u32>) -> Bauds {
    if list.is_empty() {
        Bauds::Current
    } else {
        Bauds::List(list)
    }
}

fn default_to() -> u8 {
    250
}

fn default_retries() -> u32 {
    1
}

async fn scan_primary(
    State(st): State<AppState>,
    headers: HeaderMap,
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
    start_job(&st, "primary", lang_of(&headers), move |m, p| {
        m.scan_with(bauds_of(req.bauds), p, &mut |m, p| {
            m.scan_primary(from, to, retries, p)
        })
    })
}

async fn scan_secondary(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SecondaryScanReq>,
) -> ApiResult<JobStatus> {
    start_job(&st, "secondary", lang_of(&headers), move |m, p| {
        m.scan_with(bauds_of(req.bauds), p, &mut |m, p| m.scan_secondary(p))
    })
}

#[derive(Deserialize)]
struct FreeAddressReq {
    #[serde(default)]
    from: u8,
    /// bekannte belegte Adressen
    #[serde(default)]
    skip: Vec<u8>,
}

async fn free_address(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FreeAddressReq>,
) -> ApiResult<serde_json::Value> {
    let address = with_bus(&st, lang_of(&headers), move |m| m.free_primary(req.from, &req.skip)).await?;
    Ok(Json(json!({ "address": address })))
}

#[derive(Deserialize, Default)]
struct BusBaudsReq {
    /// leer = alle Standard-Baudraten
    #[serde(default)]
    bauds: Vec<u32>,
}

async fn bus_bauds(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BusBaudsReq>,
) -> ApiResult<serde_json::Value> {
    let bauds = with_bus(&st, lang_of(&headers), move |m| m.bus_bauds(&req.bauds)).await?;
    Ok(Json(json!({ "bauds": bauds })))
}

/// Letzter abgeschlossener Scan (für die Trefferliste nach dem Neuladen)
/// Letzter Scan der aktiven Linie (für die Trefferliste nach dem Neuladen)
async fn last_scan(State(st): State<AppState>) -> ApiResult<Option<serde_json::Value>> {
    let db = st.db.lock().unwrap();
    Ok(Json(db.last_scan(db.active_line()?.id)?))
}

async fn job(State(st): State<AppState>) -> Json<JobStatus> {
    Json(st.job.lock().unwrap().clone())
}

async fn cancel(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.cancel.store(true, Ordering::SeqCst);
    Json(json!({ "ok": true }))
}

#[derive(Deserialize)]
struct ReadReq {
    address: String,
    /// Mit anderer Baudrate als der Bus lesen (Treffer aus dem Scan über alle Baudraten)
    #[serde(default)]
    baud: Option<u32>,
}

async fn read(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReadReq>,
) -> ApiResult<MeterReading> {
    let target = parse_target(&req.address)?;
    let line = st.line_id()?;
    let t = target.clone();
    let baud = req.baud;
    let result = with_bus(&st, lang_of(&headers), move |m| at_baud(m, baud, |m| m.read_meter(&t))).await;
    // Ergebnis speichern (Verlauf) und in der Zählerliste vermerken, falls der Zähler dort steht
    match &result {
        Ok(r) => st.record(line, &target, Ok(r)),
        Err(ApiError(StatusCode::BAD_GATEWAY, e)) => st.record(line, &target, Err(e.clone())),
        Err(_) => {}
    }
    Ok(Json(result?))
}

#[derive(Deserialize)]
struct SetAddressReq {
    address: String,
    new_address: u8,
    /// Zähler läuft mit anderer Baudrate als der Bus
    #[serde(default)]
    from_baud: Option<u32>,
}

/// Führt `f` mit der Baudrate `baud` aus (falls angegeben) und stellt danach die alte wieder her.
fn at_baud<T>(
    m: &mut Master,
    baud: Option<u32>,
    f: impl FnOnce(&mut Master) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let home = m.baud();
    m.set_baud(baud.unwrap_or(home))?;
    let r = f(m);
    m.set_baud(home)?;
    r
}

/// Was an einem Treffer geändert wurde
#[derive(Clone, Copy)]
enum HitUpdate {
    Pa(u8),
    Baud(u32),
}

/// Änderung am Zähler auch in der Trefferliste nachtragen (laufender Job und letzter Scan der
/// Linie in der Datenbank), damit sie nach dem Neuladen der Seite oder auf einem zweiten Gerät
/// stimmt. Liefert Name (ID) und alten Wert des Zählers fürs Änderungsprotokoll.
fn update_hits(
    st: &AppState,
    line: i64,
    target: &Target,
    from_baud: Option<u32>,
    upd: HitUpdate,
) -> Option<(String, String)> {
    let bus = st.conn.lock().unwrap().as_ref().map(|c| c.baud)?;
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
                if !matches(h.status, h.primary_address, h.secondary_address.as_deref(), h.baud) {
                    continue;
                }
                let old = match upd {
                    HitUpdate::Pa(_) => h.primary_address.map(|a| a.to_string()),
                    HitUpdate::Baud(_) => h.baud.map(|b| b.to_string()),
                };
                found.get_or_insert((h.id.clone().unwrap_or_else(|| target.label()), old.unwrap_or_default()));
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
            if !matches(h["status"].as_str().unwrap_or(""), pa, h["secondary_address"].as_str(), hb) {
                continue;
            }
            let old = match upd {
                HitUpdate::Pa(_) => pa.map(|a| a.to_string()),
                HitUpdate::Baud(_) => hb.map(|b| b.to_string()),
            };
            found.get_or_insert((
                h["id"].as_str().map(String::from).unwrap_or_else(|| target.label()),
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

async fn set_address(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetAddressReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    let target = parse_target(&req.address)?;
    let new = req.new_address;
    let line = st.line_id()?;
    let t = target.clone();
    with_bus(&st, lang, move |m| {
        at_baud(m, req.from_baud, |m| m.set_primary(&t, new))
    })
    .await?;
    i18n::set(lang);
    let (name, old) = update_hits(&st, line, &target, req.from_baud, HitUpdate::Pa(new))
        .unwrap_or_else(|| (target.label(), "?".into()));
    let text = format!("{name}: {} {old} → {new}", tr("Primäradresse", "primary address"));
    let _ = st.db.lock().unwrap().add_change(line, &text);
    Ok(Json(json!({ "ok": true, "new_address": new })))
}

#[derive(Deserialize)]
struct SwitchBaudReq {
    address: String,
    baud: u32,
    /// Baudrate, mit der der Zähler heute läuft (sonst die des Busses)
    #[serde(default)]
    from_baud: Option<u32>,
}

async fn switch_baud(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SwitchBaudReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    let target = parse_target(&req.address)?;
    let baud = req.baud;
    let line = st.line_id()?;
    let t = target.clone();
    with_bus(&st, lang, move |m| {
        at_baud(m, req.from_baud, |m| m.switch_baud(&t, baud))
    })
    .await?;
    i18n::set(lang);
    let (name, old) = update_hits(&st, line, &target, req.from_baud, HitUpdate::Baud(baud))
        .unwrap_or_else(|| (target.label(), "?".into()));
    let text = format!("{name}: {} {old} → {baud} Bd", tr("Baudrate", "baud rate"));
    let _ = st.db.lock().unwrap().add_change(line, &text);
    Ok(Json(json!({ "ok": true, "baud": baud })))
}

async fn detect_baud(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReadReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let bauds = with_bus(&st, lang_of(&headers), move |m| m.detect_baud(&target)).await?;
    Ok(Json(json!({ "bauds": bauds })))
}

#[derive(Deserialize)]
struct ResetReq {
    address: String,
    #[serde(default)]
    subcode: Option<u8>,
    #[serde(default)]
    from_baud: Option<u32>,
}

async fn app_reset(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ResetReq>,
) -> ApiResult<serde_json::Value> {
    let target = parse_target(&req.address)?;
    let sub = req.subcode;
    with_bus(&st, lang_of(&headers), move |m| {
        at_baud(m, req.from_baud, |m| m.application_reset(&target, sub))
    })
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct RawReq {
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
async fn analyze(headers: HeaderMap, Json(req): Json<RawReq>) -> ApiResult<crate::mbus::analyze::Analysis> {
    i18n::set(lang_of(&headers));
    let bytes = parse_hex(&req.hex).map_err(bad_request)?;
    Ok(Json(crate::mbus::analyze::analyze(&bytes)))
}

async fn raw_send(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RawReq>,
) -> ApiResult<serde_json::Value> {
    let bytes = parse_hex(&req.hex).map_err(bad_request)?;
    let reply = with_bus(&st, lang_of(&headers), move |m| m.raw_send(&bytes)).await?;
    Ok(Json(match reply {
        Reply::None => json!({ "kind": "none" }),
        Reply::Collision(b) => json!({ "kind": "collision", "hex": hex(&b) }),
        Reply::Frame(f) => {
            // Datentelegramm gleich decodieren, wenn möglich
            let decoded = match &f {
                Frame::Long { ci, data, .. } => decode_rsp_ud(*ci, data).ok().map(|t| {
                    json!({ "header": t.header, "records": t.records })
                }),
                _ => None,
            };
            let kind = if f == Frame::Ack { "ack" } else { "frame" };
            json!({ "kind": kind, "hex": hex(&f.encode()), "decoded": decoded })
        }
    }))
}

// ------------------------------------------------------------ Zählerliste

async fn meters_list(State(st): State<AppState>) -> ApiResult<Vec<Meter>> {
    let db = st.db.lock().unwrap();
    Ok(Json(db.meters(db.active_line()?.id)?))
}

async fn meters_add(State(st): State<AppState>, Json(m): Json<Meter>) -> ApiResult<Meter> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    Ok(Json(db.add_meter(line, m).map_err(bad_request)?))
}

async fn meters_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(m): Json<Meter>,
) -> ApiResult<Meter> {
    Ok(Json(st.db.lock().unwrap().update_meter(id, m).map_err(bad_request)?))
}

async fn meters_clear(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    let n = db.clear_meters(line).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

async fn meters_remove(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().remove_meter(id).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Liest einen Zähler der Liste, mit seiner eigenen Baudrate falls angegeben.
fn read_listed(m: &mut Master, meter: &Meter, home: u32) -> anyhow::Result<(Target, MeterReading)> {
    let target = Target::parse(&meter.address)?;
    m.set_baud(meter.baud.unwrap_or(home))?;
    let r = m.read_meter(&target)?;
    Ok((target, r))
}

async fn meters_read_one(
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
async fn meter_history(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
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
            json!({ "timestamp": r.timestamp, "values": values })
        })
        .collect();
    Ok(Json(json!({ "meter": meter, "points": points })))
}

async fn meters_read_all(
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

fn download(body: String, mime: &str, filename: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

async fn meters_export(State(st): State<AppState>) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let body = serde_json::to_string_pretty(&json!({ "meters": db.meters(db.active_line()?.id)? }))
        .unwrap_or_default();
    Ok(download(body, "application/json", "mbusio-meters.json"))
}

#[derive(Deserialize)]
struct ImportReq {
    meters: Vec<Meter>,
}

async fn meters_import(
    State(st): State<AppState>,
    Json(req): Json<ImportReq>,
) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    let n = db.import_meters(line, req.meters).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "added": n })))
}

async fn readings(State(st): State<AppState>) -> ApiResult<Vec<MeterReading>> {
    let db = st.db.lock().unwrap();
    Ok(Json(db.readings(db.active_line()?.id, MAX_READINGS)?))
}

async fn clear_readings(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let line = db.active_line()?.id;
    db.clear_readings(line)?;
    Ok(Json(json!({ "ok": true })))
}

// ------------------------------------------------------------ Projekte, Gateways, Linien

async fn tree(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    Ok(Json(json!({ "projects": db.tree()?, "active": db.active()? })))
}

/// Projekt anlegen und gleich seine erste Linie aktivieren
async fn project_create(State(st): State<AppState>, Json(p): Json<Project>) -> ApiResult<serde_json::Value> {
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

async fn project_update(State(st): State<AppState>, Path(id): Path<i64>, Json(p): Json<Project>) -> ApiResult<Project> {
    Ok(Json(st.db.lock().unwrap().update_project(id, p).map_err(bad_request)?))
}

async fn project_delete(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db.lock().unwrap().delete_project(id).map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn gateway_create(State(st): State<AppState>, Path(project): Path<i64>, Json(g): Json<Gateway>) -> ApiResult<Gateway> {
    Ok(Json(st.db.lock().unwrap().create_gateway(project, g).map_err(bad_request)?))
}

async fn gateway_update(State(st): State<AppState>, Path(id): Path<i64>, Json(g): Json<Gateway>) -> ApiResult<Gateway> {
    Ok(Json(st.db.lock().unwrap().update_gateway(id, g).map_err(bad_request)?))
}

async fn gateway_delete(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db.lock().unwrap().delete_gateway(id).map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn line_create(State(st): State<AppState>, Path(gateway): Path<i64>, Json(l): Json<Line>) -> ApiResult<Line> {
    Ok(Json(st.db.lock().unwrap().create_line(gateway, l).map_err(bad_request)?))
}

async fn line_update(State(st): State<AppState>, Path(id): Path<i64>, Json(l): Json<Line>) -> ApiResult<Line> {
    Ok(Json(st.db.lock().unwrap().update_line(id, l).map_err(bad_request)?))
}

async fn line_delete(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db.lock().unwrap().delete_line(id).map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

fn ensure_idle(st: &AppState) -> Result<(), ApiError> {
    if st.job.lock().unwrap().running {
        return Err(busy());
    }
    Ok(())
}

/// Verbindung trennen, z.B. weil eine andere Linie aktiv wird
fn disconnect_bus(st: &AppState) {
    if let Ok(mut bus) = st.bus.try_lock() {
        if bus.take().is_some() {
            st.log.push("INFO", tr("Verbindung getrennt", "Disconnected"));
        }
    }
    *st.conn.lock().unwrap() = None;
    *st.job.lock().unwrap() = JobStatus::default();
}

#[derive(Deserialize)]
struct MoveReq {
    /// -1 nach oben, +1 nach unten
    dir: i64,
}

async fn gateway_move(State(st): State<AppState>, Path(id): Path<i64>, Json(r): Json<MoveReq>) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().move_gateway(id, r.dir).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

async fn line_move(State(st): State<AppState>, Path(id): Path<i64>, Json(r): Json<MoveReq>) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().move_line(id, r.dir).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AddressingReq {
    mode: String,
}

/// Merken über SA oder PA; stellt die Zählerliste der Linie mit um
async fn line_addressing(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<AddressingReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let (changed, kept) = st.db.lock().unwrap().set_addressing(id, &req.mode).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "changed": changed, "kept": kept })))
}

/// Auffälligkeiten der Linie merken (berechnet die Oberfläche aus dem letzten Scan)
async fn line_issues(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(v): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().set_issues(id, &v).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Linie aktivieren und, wenn sie eine Verbindung hat, gleich damit verbinden
async fn line_activate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let lang = lang_of(&headers);
    let line = st.db.lock().unwrap().set_active(id).map_err(bad_request)?;
    disconnect_bus(&st);
    if line.connection.is_empty() {
        return Ok(Json(json!({ "ok": true, "connected": false })));
    }
    let s = st.clone();
    let result = tokio::task::spawn_blocking(move || {
        i18n::set(lang);
        connect_blocking(&s, &line.connection, line.baud, line.timeout_ms)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Linie bleibt aktiv, auch wenn der Anschluss gerade nicht erreichbar ist
    Ok(Json(match result {
        Ok(()) => json!({ "ok": true, "connected": true }),
        Err(e) => json!({ "ok": true, "connected": false, "error": format!("{e:#}") }),
    }))
}

#[derive(Deserialize)]
struct ProjectQuery {
    #[serde(default)]
    project: Option<i64>,
}

fn project_or_active(st: &AppState, q: &ProjectQuery) -> Result<i64, ApiError> {
    match q.project {
        Some(id) => Ok(id),
        None => Ok(st.db.lock().unwrap().active()?.project.id),
    }
}

/// Alles für den Bericht eines Projekts (Standard: aktives Projekt)
async fn report(State(st): State<AppState>, Query(q): Query<ProjectQuery>) -> ApiResult<serde_json::Value> {
    let id = project_or_active(&st, &q)?;
    Ok(Json(st.db.lock().unwrap().report(id).map_err(bad_request)?))
}

async fn changes_clear(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    let n = st.db.lock().unwrap().clear_changes(id)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

async fn project_export(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let v = db.export_project(id).map_err(bad_request)?;
    let name: String = db
        .project(id)?
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    Ok(download(
        serde_json::to_string_pretty(&v).unwrap_or_default(),
        "application/json",
        &format!("mbusio-{name}.json"),
    ))
}

async fn project_import(State(st): State<AppState>, Json(v): Json<serde_json::Value>) -> ApiResult<serde_json::Value> {
    let id = st.db.lock().unwrap().import_project(&v).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "project": id })))
}

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default)]
    format: String,
    #[serde(default)]
    lang: String,
}

async fn export_readings(State(st): State<AppState>, Query(q): Query<ExportQuery>) -> Result<Response, ApiError> {
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

#[derive(Deserialize)]
struct LogQuery {
    #[serde(default)]
    since: u64,
}

async fn log(State(st): State<AppState>, Query(q): Query<LogQuery>) -> Json<serde_json::Value> {
    Json(json!(st.log.since(q.since)))
}

#[derive(Deserialize)]
struct LabelsQuery {
    #[serde(default)]
    lang: String,
}

async fn labels(Query(q): Query<LabelsQuery>) -> Json<serde_json::Map<String, serde_json::Value>> {
    Json(i18n::labels(Lang::parse(&q.lang)))
}

async fn clear_log(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.log.clear();
    Json(json!({ "ok": true }))
}

async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
