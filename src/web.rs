//! HTTP-API und eingebettete Weboberfläche.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::export;
use crate::i18n::{self, tr, Lang};
use crate::link::{self, SerialLink, SIMULATOR_PORT};
use crate::mbus::master::{FrameLog, Master, MeterReading, Progress, ScanHit, Target};
use crate::sim::Simulator;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

const MAX_READINGS: usize = 2000;

#[derive(Clone, Serialize)]
pub struct ConnInfo {
    pub port: String,
    pub baud: u32,
    pub timeout_ms: u64,
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

#[derive(Default)]
pub struct Shared {
    bus: Mutex<Option<Master>>,
    conn: Mutex<Option<ConnInfo>>,
    job: Mutex<JobStatus>,
    cancel: AtomicBool,
    readings: Mutex<Vec<MeterReading>>,
    log: Arc<FrameLog>,
    /// Optionaler Zugriffsschlüssel für die API (bei LAN-Zugriff)
    token: Option<String>,
}

impl Shared {
    pub fn new(token: Option<String>) -> Self {
        Shared { token, ..Default::default() }
    }
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
    ApiError(StatusCode::CONFLICT, tr("Bus ist belegt (Scan läuft?)", "Bus is busy (scan running?)"))
}

fn not_connected() -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, tr("Nicht verbunden", "Not connected"))
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
        .route("/api/job/cancel", post(cancel))
        .route("/api/read", post(read))
        .route("/api/set-address", post(set_address))
        .route("/api/readings", get(readings).delete(clear_readings))
        .route("/api/export", get(export_readings))
        .route("/api/log", get(log).delete(clear_log))
        .route("/api/labels", get(labels))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .fallback(static_file)
        .with_state(state)
}

/// Prüft den Zugriffsschlüssel (Header `X-Token` oder `?token=`), falls einer gesetzt ist.
async fn auth(State(st): State<AppState>, req: Request, next: Next) -> Response {
    i18n::set(lang_of(req.headers()));
    if let Some(token) = &st.token {
        let header_ok = req.headers().get("x-token").and_then(|v| v.to_str().ok()) == Some(token.as_str());
        let query_ok = req
            .uri()
            .query()
            .is_some_and(|q| q.split('&').any(|kv| kv.strip_prefix("token=") == Some(token.as_str())));
        if !header_ok && !query_ok {
            return ApiError(StatusCode::UNAUTHORIZED, tr("Zugriffsschlüssel fehlt oder ist falsch", "Access token missing or wrong"))
                .into_response();
        }
    }
    next.run(req).await
}

pub fn connect_blocking(state: &Shared, port: &str, baud: u32, timeout_ms: u64) -> anyhow::Result<()> {
    let link: Box<dyn link::Link> = if port == SIMULATOR_PORT {
        Box::new(Simulator::new(baud))
    } else {
        Box::new(SerialLink::open(port, baud)?)
    };
    let master = Master::new(link, baud, timeout_ms, state.log.clone());
    let effective = master.timeout.as_millis() as u64;
    let mut bus = state.bus.try_lock().map_err(|_| anyhow::anyhow!(tr("Bus ist belegt", "Bus is busy")))?;
    *bus = Some(master);
    *state.conn.lock().unwrap() = Some(ConnInfo { port: port.into(), baud, timeout_ms: effective });
    state.log.push("INFO", format!("{} {port}, {baud} Bd, 8E1, Timeout {effective} ms", tr("Verbunden mit", "Connected to")));
    Ok(())
}

async fn ports() -> Json<Vec<String>> {
    Json(tokio::task::spawn_blocking(link::list_ports).await.unwrap_or_default())
}

async fn status(State(st): State<AppState>) -> Json<serde_json::Value> {
    let conn = st.conn.lock().unwrap().clone();
    let job = st.job.lock().unwrap();
    Json(json!({
        "connected": conn.is_some(),
        "connection": conn,
        "job_running": job.running,
        "readings": st.readings.lock().unwrap().len(),
        "version": env!("CARGO_PKG_VERSION"),
        "token_required": st.token.is_some(),
    }))
}

#[derive(Deserialize)]
struct ConnectReq {
    port: String,
    baud: u32,
    #[serde(default)]
    timeout_ms: u64,
}

async fn connect(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<ConnectReq>) -> ApiResult<serde_json::Value> {
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
    Ok(Json(json!({ "ok": true, "connection": st.conn.lock().unwrap().clone() })))
}

async fn disconnect(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut bus = st.bus.try_lock().map_err(|_| busy())?;
    *bus = None;
    *st.conn.lock().unwrap() = None;
    st.log.push("INFO", tr("Verbindung getrennt", "Disconnected"));
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
        *job = JobStatus { kind: kind.into(), running: true, message: String::new(), ..Default::default() };
    }
    st.cancel.store(false, Ordering::SeqCst);
    st.log.push("INFO", format!("{} {kind}", tr("Starte", "Starting")));
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
                            Progress::Step { done, total, message } => {
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
        s.log.push("INFO", format!("{} {}: {}", job.kind, tr("beendet, Treffer", "finished, hits"), job.hits.len()));
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
}

fn default_to() -> u8 {
    250
}

fn default_retries() -> u32 {
    1
}

async fn scan_primary(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<PrimaryScanReq>) -> ApiResult<JobStatus> {
    let (from, to) = (req.from.min(250), req.to.min(250));
    if from > to {
        return Err(ApiError(StatusCode::BAD_REQUEST, tr("Startadresse grösser als Endadresse", "Start address greater than end address")));
    }
    let retries = req.retries.min(5);
    start_job(&st, "primary", lang_of(&headers), move |m, p| m.scan_primary(from, to, retries, p))
}

async fn scan_secondary(State(st): State<AppState>, headers: HeaderMap) -> ApiResult<JobStatus> {
    start_job(&st, "secondary", lang_of(&headers), |m, p| m.scan_secondary(p))
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
}

async fn read(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<ReadReq>) -> ApiResult<MeterReading> {
    let target = Target::parse(&req.address).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let reading = with_bus(&st, lang_of(&headers), move |m| m.read_meter(&target)).await?;
    let mut all = st.readings.lock().unwrap();
    all.push(reading.clone());
    if all.len() > MAX_READINGS {
        let excess = all.len() - MAX_READINGS;
        all.drain(..excess);
    }
    Ok(Json(reading))
}

#[derive(Deserialize)]
struct SetAddressReq {
    address: String,
    new_address: u8,
}

async fn set_address(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetAddressReq>,
) -> ApiResult<serde_json::Value> {
    let target = Target::parse(&req.address).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let new = req.new_address;
    with_bus(&st, lang_of(&headers), move |m| m.set_primary(&target, new)).await?;
    Ok(Json(json!({ "ok": true, "new_address": new })))
}

async fn readings(State(st): State<AppState>) -> Json<Vec<MeterReading>> {
    Json(st.readings.lock().unwrap().clone())
}

async fn clear_readings(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.readings.lock().unwrap().clear();
    Json(json!({ "ok": true }))
}

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default)]
    format: String,
    #[serde(default)]
    lang: String,
}

async fn export_readings(State(st): State<AppState>, Query(q): Query<ExportQuery>) -> Response {
    let readings = st.readings.lock().unwrap().clone();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let (body, mime, ext) = match q.format.as_str() {
        "json" => (export::json(&readings), "application/json", "json"),
        _ => (export::csv(&readings, Lang::parse(&q.lang)), "text/csv; charset=utf-8", "csv"),
    };
    (
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"mbusio-{stamp}.{ext}\"")),
        ],
        body,
    )
        .into_response()
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
            ([(header::CONTENT_TYPE, mime.as_ref().to_string())], file.data.into_owned()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
