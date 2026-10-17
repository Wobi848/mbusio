//! HTTP-API und eingebettete Weboberfläche.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::clock;
use crate::db::{Db, Gateway, Line, Meter, Project};
use crate::export;
use crate::i18n::{self, tr, Lang};
use crate::line::{is_link_error, ConnInfo, LineConf, LineService};
use crate::link;
use crate::mbus::decode::decode_rsp_ud;
use crate::mbus::frame::{hex, Frame};
use crate::mbus::master::{
    mask_matches, Bauds, FrameLog, Master, MeterReading, Progress, Reply, ScanHit, Target,
};
use crate::scheduler::Planner;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

/// Höchstens so viele Auslesungen einer Linie an die Oberfläche bzw. in den Export
const MAX_READINGS: usize = 5000;

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
    /// Linien-Dienste, je verbundener Linie einer; nur er spricht mit dem Bus dieser Linie
    lines: Mutex<HashMap<i64, LineService>>,
    job: Mutex<JobStatus>,
    cancel: AtomicBool,
    pub(crate) log: Arc<FrameLog>,
    /// Projekte, Linien, Zähler, Auslesungen; Zählerliste und Scan gelten für die aktive Linie
    pub(crate) db: Mutex<Db>,
    /// Zeitplan der geplanten Auslesungen
    pub(crate) planner: Planner,
    /// MQTT-Ausgang: Zustand und Version der Einstellungen
    pub(crate) mqtt: crate::mqtt::MqttState,
    /// Optionaler Zugriffsschlüssel für die API (bei LAN-Zugriff)
    token: Option<String>,
}

impl Shared {
    pub fn new(token: Option<String>, db: Db) -> Self {
        Self::with_planner(token, db, Planner::default())
    }

    pub fn with_planner(token: Option<String>, db: Db, planner: Planner) -> Self {
        Shared {
            lines: Mutex::default(),
            job: Mutex::default(),
            cancel: AtomicBool::default(),
            log: Arc::default(),
            db: Mutex::new(db),
            planner,
            mqtt: Default::default(),
            token,
        }
    }

    fn line_id(&self) -> Result<i64, ApiError> {
        Ok(self.db.lock().unwrap().active_line()?.id)
    }

    /// Dienst der aktiven Linie; ohne Verbindung "Nicht verbunden"
    fn service(&self) -> Result<LineService, ApiError> {
        let id = self.line_id()?;
        self.lines
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(not_connected)
    }

    /// Verbindung der aktiven Linie, falls offen
    fn conn_info(&self) -> Option<ConnInfo> {
        self.service().ok()?.status().connection
    }

    /// Linie mit diesem Anschluss verbinden; ein vorhandener Dienst der Linie wird ersetzt.
    /// Klappt die Verbindung nicht, bleibt die Linie getrennt und der Fehler kommt zurück.
    pub fn connect_line(&self, line_id: i64, conf: LineConf, lang: Lang) -> anyhow::Result<()> {
        self.planner.release(line_id);
        // läuft schon mit diesem Anschluss (z.B. wegen des Ausleseplans): weiter benutzen
        let running = self.lines.lock().unwrap().get(&line_id).cloned();
        if let Some(svc) = running.filter(|s| *s.conf() == conf) {
            return svc.connect(lang);
        }
        self.stop_line_wait(line_id);
        let svc = LineService::start(line_id, conf, self.log.clone());
        match svc.connect(lang) {
            Ok(()) => {
                self.lines.lock().unwrap().insert(line_id, svc);
                Ok(())
            }
            Err(e) => {
                svc.stop();
                Err(e)
            }
        }
    }

    fn stop_line(&self, line_id: i64) {
        if let Some(old) = self.lines.lock().unwrap().remove(&line_id) {
            old.stop();
        }
    }

    /// Wie `stop_line`, wartet aber, bis der Anschluss wieder frei ist (vor dem Neuöffnen)
    fn stop_line_wait(&self, line_id: i64) {
        let old = self.lines.lock().unwrap().remove(&line_id);
        if let Some(old) = old {
            old.stop_wait(std::time::Duration::from_secs(15));
        }
    }

    /// Verbindungen trennen, z.B. weil eine andere Linie aktiv wird. Linien mit eingeschaltetem
    /// Ausleseplan bleiben verbunden.
    fn stop_lines(&self) {
        let keep = self.planner.planned_lines();
        self.lines.lock().unwrap().retain(|id, svc| {
            let k = keep.contains(id) && !self.planner.is_held(*id);
            if !k {
                svc.stop();
            }
            k
        });
    }

    /// Dienst für den Zeitplan: läuft schon, oder wird mit dem Anschluss aus der Datenbank
    /// gestartet (verbindet beim ersten Auftrag). Anschluss geändert: neu starten.
    pub(crate) fn ensure_service(&self, line: &Line) -> Option<LineService> {
        let conf = LineConf {
            connection: line.connection.clone(),
            baud: line.baud,
            timeout_ms: line.timeout_ms,
        };
        let running = self.lines.lock().unwrap().get(&line.id).cloned();
        match running {
            Some(svc) if *svc.conf() == conf => return Some(svc),
            // von Hand mit anderem Anschluss verbunden (aktive Linie): nicht dazwischenfunken
            Some(svc) if self.line_id().ok() == Some(line.id) && svc.status().connected => {
                return Some(svc);
            }
            Some(_) => self.stop_line_wait(line.id),
            None => {}
        }
        let svc = LineService::start(line.id, conf, self.log.clone());
        self.lines.lock().unwrap().insert(line.id, svc.clone());
        Some(svc)
    }

    /// Alle Linien mit diesem Anschluss neu verbinden (z.B. weil die Simulation geändert wurde).
    /// Liefert die Anzahl.
    pub(crate) fn restart_connection(&self, connection: &str, lang: Lang) -> usize {
        let ids: Vec<(i64, LineConf)> = self
            .lines
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.conf().connection == connection)
            .map(|(id, s)| (*id, s.conf().clone()))
            .collect();
        let active = self.line_id().ok();
        for (id, conf) in &ids {
            self.stop_line_wait(*id);
            // die angesehene Linie gleich wieder verbinden; Linien mit Plan startet der Zeitplan
            if Some(*id) == active {
                if let Err(e) = self.connect_line(*id, conf.clone(), lang) {
                    self.log.push("INFO", format!("{connection}: {e:#}"));
                }
            }
        }
        ids.len()
    }

    /// Dienste von Linien beenden, die weder einen Plan haben noch gerade angesehen werden
    pub(crate) fn stop_unused_services(&self) {
        let keep = self.planner.planned_lines();
        let active = self.line_id().ok();
        self.lines.lock().unwrap().retain(|id, svc| {
            let k = keep.contains(id) || Some(*id) == active;
            if !k {
                svc.stop();
            }
            k
        });
    }

    /// Auslesung speichern (Verlauf und Zählerliste der Linie); Speicherfehler nur ins Log
    pub(crate) fn record(&self, line: i64, target: &Target, result: Result<&MeterReading, String>) {
        let notes = {
            let mut db = self.db.lock().unwrap();
            if let Err(e) = db.record(line, target, result, false) {
                self.log.push("INFO", format!("DB: {e:#}"));
            }
            std::mem::take(&mut db.notes)
        };
        for n in notes {
            self.log.push("INFO", n);
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

/// Führt `f` auf dem Bus der aktiven Linie aus: als Auftrag an ihren Linien-Dienst, der ihn
/// nach den laufenden Aufträgen abarbeitet (kein "Bus belegt").
async fn with_bus<T, F>(state: &AppState, lang: Lang, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Master) -> anyhow::Result<T> + Send + 'static,
{
    let svc = state.service()?;
    tokio::task::spawn_blocking(move || svc.run(lang, f).map_err(ApiError::from))
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
        .route(
            "/api/projects/{id}",
            put(project_update).delete(project_delete),
        )
        .route("/api/projects/{id}/export", get(project_export))
        .route(
            "/api/projects/{id}/changes",
            axum::routing::delete(changes_clear),
        )
        .route("/api/projects/{id}/gateways", post(gateway_create))
        .route(
            "/api/gateways/{id}",
            put(gateway_update).delete(gateway_delete),
        )
        .route("/api/gateways/{id}/lines", post(line_create))
        .route("/api/lines/{id}", put(line_update).delete(line_delete))
        .route("/api/lines/{id}/activate", post(line_activate))
        .route("/api/lines/{id}/issues", put(line_issues))
        .route("/api/lines/{id}/addressing", put(line_addressing))
        .route("/api/lines/{id}/move", post(line_move))
        .route("/api/gateways/{id}/move", post(gateway_move))
        .route("/api/report", get(report))
        .route("/api/projects/{id}/plan", put(project_plan))
        .route("/api/projects/{id}/thin", put(project_thin))
        .route("/api/mqtt", get(mqtt_get).put(mqtt_put))
        .route("/api/meters/import-list", post(meters_import_list))
        .route("/api/meters/names", post(meters_names))
        .route("/api/meters/csv", get(meters_csv))
        .route("/api/sim", get(sim_get).post(sim_create))
        .route("/api/sim/{key}", put(sim_save).delete(sim_delete))
        .route("/api/sim/{key}/use", post(sim_use))
        .route("/api/datapoints", get(datapoints))
        .route("/api/datapoints/bulk", post(datapoints_bulk))
        .route("/api/datapoints/{id}", put(datapoint_update))
        .route("/api/meters/{id}/values", get(meter_values))
        .route("/api/meters/{id}/datapoints", put(meter_datapoints))
        .route("/api/meters/{id}/exchange", post(meter_exchange))
        .route(
            "/api/templates/{id}",
            axum::routing::delete(template_delete),
        )
        .route("/api/projects/{id}/datapoints/csv", get(datapoints_csv))
        .route(
            "/api/projects/{id}/datapoints/import",
            post(datapoints_import),
        )
        .route("/api/lines/{id}/plan", put(line_plan))
        .route("/api/meters/{id}/plan", put(meter_plan))
        .route("/api/time", post(set_time))
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

async fn ports() -> Json<Vec<String>> {
    Json(
        tokio::task::spawn_blocking(link::list_ports)
            .await
            .unwrap_or_default(),
    )
}

async fn status(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
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
    let line = st.line_id()?;
    let conf = LineConf {
        connection: req.port.clone(),
        baud: req.baud,
        timeout_ms: req.timeout_ms,
    };
    let s = st.clone();
    tokio::task::spawn_blocking(move || s.connect_line(line, conf, lang))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    // erfolgreiche Verbindung gehört zur aktiven Linie: beim nächsten Mal wieder so verbinden
    st.db
        .lock()
        .unwrap()
        .set_line_connection(line, &req.port, req.baud, req.timeout_ms)?;
    Ok(Json(json!({ "ok": true, "connection": st.conn_info() })))
}

async fn disconnect(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let line = st.line_id()?;
    // von Hand getrennt (z.B. um den Anschluss für ein anderes Programm frei zu machen):
    // der Ausleseplan verbindet nicht von selbst wieder, bis man wieder verbindet
    st.planner.hold(line);
    let planned = st.planner.is_planned(line);
    st.stop_line(line);
    Ok(Json(json!({ "ok": true, "plan_paused": planned })))
}

fn start_job<F>(st: &AppState, kind: &str, lang: Lang, work: F) -> ApiResult<JobStatus>
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
            let value = serde_json::to_value(&*job).unwrap_or_default();
            if let Err(e) = s.db.lock().unwrap().set_last_scan(line, &value) {
                s.log.push("INFO", format!("DB: {e:#}"));
            }
        }
        link_ok
    }));
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
    let address = with_bus(&st, lang_of(&headers), move |m| {
        m.free_primary(req.from, &req.skip)
    })
    .await?;
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
    let result = with_bus(&st, lang_of(&headers), move |m| {
        at_baud(m, baud, |m| m.read_meter(&t))
    })
    .await;
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
                    h.status,
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
    let text = format!(
        "{name}: {} {old} → {new}",
        tr("Primäradresse", "primary address")
    );
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
async fn analyze(
    headers: HeaderMap,
    Json(req): Json<RawReq>,
) -> ApiResult<crate::mbus::analyze::Analysis> {
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

// ------------------------------------------------------------ Zählerliste

/// Zählerliste der aktiven Linie, je Zähler mit seinem Plan (gültiger Plan, nächster Termin …)
async fn meters_list(State(st): State<AppState>) -> ApiResult<Vec<serde_json::Value>> {
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
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_meter(id, m)
            .map_err(bad_request)?,
    ))
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
    st.db
        .lock()
        .unwrap()
        .remove_meter(id)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Liest einen Zähler der Liste, mit seiner eigenen Baudrate falls angegeben.
pub(crate) fn read_listed(
    m: &mut Master,
    meter: &Meter,
    home: u32,
) -> anyhow::Result<(Target, MeterReading)> {
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
async fn meter_history(
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

async fn meters_read_all(State(st): State<AppState>, headers: HeaderMap) -> ApiResult<JobStatus> {
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

/// Zustand der Linien-Dienste in den Baum eintragen: verbunden, Fehler, von Hand getrennt
fn annotate_lines(st: &AppState, tree: &mut serde_json::Value) {
    let services = st.lines.lock().unwrap().clone();
    for p in tree.as_array_mut().into_iter().flatten() {
        for g in p["gateways"].as_array_mut().into_iter().flatten() {
            for l in g["lines"].as_array_mut().into_iter().flatten() {
                let Some(id) = l["id"].as_i64() else { continue };
                l["service"] = json!(services.get(&id).map(|s| s.status()));
                l["held"] = json!(st.planner.is_held(id));
            }
        }
    }
}

async fn tree(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let (mut projects, active) = {
        let db = st.db.lock().unwrap();
        let mut projects = serde_json::to_value(db.tree()?).unwrap_or_default();
        // Datenpunkte je Linie: Anzahl und ungültige
        for p in projects.as_array_mut().into_iter().flatten() {
            for g in p["gateways"].as_array_mut().into_iter().flatten() {
                for l in g["lines"].as_array_mut().into_iter().flatten() {
                    let (n, invalid) = db.dp_counts(l["id"].as_i64().unwrap_or(0))?;
                    l["datapoints"] = json!({ "count": n, "invalid": invalid });
                }
            }
        }
        (projects, db.active()?)
    };
    annotate_lines(&st, &mut projects);
    Ok(Json(json!({ "projects": projects, "active": active })))
}

// ------------------------------------------------------------ Ausleseplan, Uhrzeit

#[derive(Deserialize)]
struct PlanReq {
    /// "off", "daily 00:05", …; None = erben (bei Linie und Zähler)
    plan: Option<String>,
}

/// Warnungen zur Buslast der Linien eines Projekts (nach dem Ändern eines Plans)
fn load_warnings(
    st: &AppState,
    project_id: Option<i64>,
    line_id: Option<i64>,
) -> Result<Vec<String>, ApiError> {
    let db = st.db.lock().unwrap();
    let mut out = Vec::new();
    for p in db.tree()? {
        if project_id.is_some_and(|id| id != p.project.id) {
            continue;
        }
        for g in &p.gateways {
            for l in &g.lines {
                if line_id.is_some_and(|id| id != l.line.id) {
                    continue;
                }
                if l.plan_info["load_warn"].as_bool().unwrap_or(false) {
                    out.push(format!(
                        "{} › {}: {} {:.0} % {}",
                        g.gateway.name,
                        l.line.name,
                        tr(
                            "geplante Auslesungen brauchen ca.",
                            "planned reads need about"
                        ),
                        l.plan_info["load"].as_f64().unwrap_or(0.0) * 100.0,
                        tr(
                            "der Buszeit. Seltener auslesen oder auf mehrere Linien verteilen.",
                            "of the bus time. Read less often or split across lines."
                        )
                    ));
                }
            }
        }
    }
    Ok(out)
}

async fn project_plan(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let plan = r.plan.unwrap_or_else(|| "off".into());
    st.db
        .lock()
        .unwrap()
        .set_project_plan(id, &plan)
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, Some(id), None)? }),
    ))
}

async fn line_plan(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    st.db
        .lock()
        .unwrap()
        .set_line_plan(id, r.plan.as_deref())
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, None, Some(id))? }),
    ))
}

async fn meter_plan(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<PlanReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let line = {
        let mut db = st.db.lock().unwrap();
        db.set_meter_plan(id, r.plan.as_deref())
            .map_err(bad_request)?;
        db.meter(id)?.line_id
    };
    Ok(Json(
        json!({ "ok": true, "warnings": load_warnings(&st, None, Some(line))? }),
    ))
}

// ------------------------------------------------------------ Datenpunkte

async fn datapoints(
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
async fn meter_values(
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
struct TemplateOpt {
    /// auch auf später hinzukommende Zähler anwenden
    #[serde(default)]
    auto: bool,
}

#[derive(Deserialize)]
struct SelectReq {
    keys: Vec<String>,
    /// neue Datenpunkte gleich per MQTT weitergeben (auch in der Vorlage)
    #[serde(default)]
    mqtt: bool,
    /// zusätzlich als Vorlage für alle Zähler dieses Mediums auf der Linie
    #[serde(default)]
    template: Option<TemplateOpt>,
}

async fn meter_datapoints(
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

async fn datapoint_update(
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
struct BulkReq {
    ids: Vec<i64>,
    /// "on", "off", "delete", "send:every|change|daily"
    action: String,
}

async fn datapoints_bulk(
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

async fn template_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    st.db.lock().unwrap().template_delete(id)?;
    Ok(Json(json!({ "ok": true })))
}

/// Datenpunkt-Liste des Projekts als CSV (Excel), auch für den Integrator
async fn datapoints_csv(
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
struct LangQuery {
    #[serde(default)]
    lang: Option<String>,
}

#[derive(Deserialize)]
struct CsvImportReq {
    csv: String,
    /// false = nur zeigen, was sich ändert
    #[serde(default)]
    apply: bool,
}

async fn datapoints_import(
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

#[derive(Deserialize)]
struct ExchangeReq {
    /// Adresse des neuen Zählers (PA, ID oder Sekundäradresse)
    address: String,
    /// eigene Baudrate des neuen Zählers
    #[serde(default)]
    baud: Option<u32>,
}

/// Zähler getauscht: neuen Zähler auslesen, Zählerstände fortführen, Adresse übernehmen
async fn meter_exchange(
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

// ------------------------------------------------------------ Zählerliste schnell füllen

#[derive(Deserialize)]
struct ListImportReq {
    rows: Vec<crate::meterlist::ListRow>,
    #[serde(default)]
    apply: bool,
}

/// Liste vom Kunden/Installateur: Zähler über die ID anlegen bzw. benennen (zuerst Vorschau)
async fn meters_import_list(
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
struct NamesReq {
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

async fn meters_names(
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

async fn meters_csv(
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

// ------------------------------------------------------------ Simulator

async fn sim_get(State(st): State<AppState>, headers: HeaderMap) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let active = st.db.lock().unwrap().active_line()?;
    Ok(Json(json!({
        "scenarios": crate::sim::all(),
        "media": crate::sim::MEDIA.iter().map(|(k, _)| k).collect::<Vec<_>>(),
        "faults": crate::sim::FAULTS,
        "line": { "id": active.id, "name": active.name, "connection": active.connection, "baud": active.baud },
    })))
}

#[derive(Deserialize)]
struct SimCreateReq {
    name: String,
    /// Kopie dieser Simulation; leer = ohne Zähler
    #[serde(default)]
    from: Option<String>,
}

async fn sim_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<SimCreateReq>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    let key = st
        .db
        .lock()
        .unwrap()
        .sim_create(&r.name, r.from.as_deref())
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "key": key })))
}

#[derive(Deserialize)]
struct SimSaveReq {
    name: String,
    meters: Vec<crate::sim::SimMeter>,
}

/// Simulation speichern; Linien, die sie gerade verwenden, verbinden neu (neue Zähler gelten sofort)
async fn sim_save(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(r): Json<SimSaveReq>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    st.db
        .lock()
        .unwrap()
        .sim_save(&key, &r.name, &r.meters)
        .map_err(bad_request)?;
    let s = st.clone();
    let restarted = tokio::task::spawn_blocking(move || {
        s.restart_connection(&format!("SIMULATOR:{key}"), lang)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "ok": true, "restarted": restarted })))
}

async fn sim_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> ApiResult<serde_json::Value> {
    i18n::set(lang_of(&headers));
    st.db
        .lock()
        .unwrap()
        .sim_delete(&key)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Simulation für die aktive Linie verwenden und verbinden
async fn sim_use(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> ApiResult<serde_json::Value> {
    let lang = lang_of(&headers);
    i18n::set(lang);
    ensure_idle(&st)?;
    let connection = if key == "praxis" {
        "SIMULATOR".to_string()
    } else {
        format!("SIMULATOR:{key}")
    };
    if !crate::sim::all().iter().any(|s| s.key == key) {
        return Err(bad_request(format!(
            "{} „{key}“ {}",
            tr("Simulation", "Simulation"),
            tr("gibt es nicht", "does not exist")
        )));
    }
    let line = st.db.lock().unwrap().active_line()?;
    let conf = LineConf {
        connection: connection.clone(),
        baud: line.baud,
        timeout_ms: line.timeout_ms,
    };
    let s = st.clone();
    let id = line.id;
    tokio::task::spawn_blocking(move || s.connect_line(id, conf, lang))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(bad_request)?;
    st.db
        .lock()
        .unwrap()
        .set_line_connection(id, &connection, line.baud, line.timeout_ms)?;
    Ok(Json(json!({ "ok": true, "connection": connection })))
}

// ------------------------------------------------------------ MQTT

fn mqtt_status(st: &AppState, db: &Db) -> crate::mqtt::MqttStatus {
    let mut s = st.mqtt.status.lock().unwrap().clone();
    s.queued = db.mqtt_queued().unwrap_or(0);
    s
}

/// Einstellungen ohne Passwort, dazu ob eines gespeichert ist, und der Zustand
async fn mqtt_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    let mut c = db.mqtt_config();
    let has_password = !c.password.is_empty();
    c.password.clear();
    Ok(Json(
        json!({ "config": c, "has_password": has_password, "status": mqtt_status(&st, &db) }),
    ))
}

#[derive(Deserialize)]
struct MqttReq {
    config: crate::mqtt::MqttConfig,
    /// None = gespeichertes Passwort behalten, "" = löschen
    #[serde(default)]
    password: Option<String>,
}

async fn mqtt_put(
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

#[derive(Deserialize)]
struct ThinReq {
    /// nach so vielen Tagen nur noch ein Wert pro Tag; None = alles behalten
    days: Option<i64>,
}

async fn project_thin(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<ThinReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_thin_days(id, r.days)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TimeReq {
    /// Uhrzeit des Browsers, Millisekunden seit 1970 (UTC)
    epoch_ms: i64,
}

/// „Uhrzeit vom Gerät übernehmen“
async fn set_time(
    State(st): State<AppState>,
    Json(r): Json<TimeReq>,
) -> ApiResult<serde_json::Value> {
    let offset = clock::set_from_browser(r.epoch_ms).ok_or_else(|| bad_request("epoch_ms"))?;
    st.log.push(
        "INFO",
        if offset == 0.0 {
            tr("Uhrzeit vom Gerät bestätigt", "Time confirmed by device")
        } else {
            format!(
                "{} {offset:+.0} s",
                tr(
                    "Uhrzeit vom Gerät übernommen, Korrektur",
                    "Time taken from device, correction"
                )
            )
        },
    );
    Ok(Json(
        json!({ "ok": true, "offset_s": offset, "time": clock::state() }),
    ))
}

/// Projekt anlegen und gleich seine erste Linie aktivieren
async fn project_create(
    State(st): State<AppState>,
    Json(p): Json<Project>,
) -> ApiResult<serde_json::Value> {
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

async fn project_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(p): Json<Project>,
) -> ApiResult<Project> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_project(id, p)
            .map_err(bad_request)?,
    ))
}

async fn project_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db
        .lock()
        .unwrap()
        .delete_project(id)
        .map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn gateway_create(
    State(st): State<AppState>,
    Path(project): Path<i64>,
    Json(g): Json<Gateway>,
) -> ApiResult<Gateway> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .create_gateway(project, g)
            .map_err(bad_request)?,
    ))
}

async fn gateway_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(g): Json<Gateway>,
) -> ApiResult<Gateway> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_gateway(id, g)
            .map_err(bad_request)?,
    ))
}

async fn gateway_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let before = st.line_id()?;
    st.db
        .lock()
        .unwrap()
        .delete_gateway(id)
        .map_err(bad_request)?;
    if st.line_id()? != before {
        disconnect_bus(&st);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn line_create(
    State(st): State<AppState>,
    Path(gateway): Path<i64>,
    Json(l): Json<Line>,
) -> ApiResult<Line> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .create_line(gateway, l)
            .map_err(bad_request)?,
    ))
}

async fn line_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(l): Json<Line>,
) -> ApiResult<Line> {
    Ok(Json(
        st.db
            .lock()
            .unwrap()
            .update_line(id, l)
            .map_err(bad_request)?,
    ))
}

async fn line_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
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

/// Verbindungen trennen, z.B. weil eine andere Linie aktiv wird; Linien mit eingeschaltetem
/// Ausleseplan bleiben verbunden.
fn disconnect_bus(st: &AppState) {
    st.stop_lines();
    *st.job.lock().unwrap() = JobStatus::default();
}

#[derive(Deserialize)]
struct MoveReq {
    /// -1 nach oben, +1 nach unten
    dir: i64,
}

async fn gateway_move(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<MoveReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .move_gateway(id, r.dir)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

async fn line_move(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<MoveReq>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .move_line(id, r.dir)
        .map_err(bad_request)?;
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
    let (changed, kept) = st
        .db
        .lock()
        .unwrap()
        .set_addressing(id, &req.mode)
        .map_err(bad_request)?;
    Ok(Json(
        json!({ "ok": true, "changed": changed, "kept": kept }),
    ))
}

/// Auffälligkeiten der Linie merken (berechnet die Oberfläche aus dem letzten Scan)
async fn line_issues(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(v): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    st.db
        .lock()
        .unwrap()
        .set_issues(id, &v)
        .map_err(bad_request)?;
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
    let conf = LineConf {
        connection: line.connection.clone(),
        baud: line.baud,
        timeout_ms: line.timeout_ms,
    };
    let result = tokio::task::spawn_blocking(move || s.connect_line(id, conf, lang))
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
async fn report(
    State(st): State<AppState>,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<serde_json::Value> {
    let id = project_or_active(&st, &q)?;
    Ok(Json(st.db.lock().unwrap().report(id).map_err(bad_request)?))
}

async fn changes_clear(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let n = st.db.lock().unwrap().clear_changes(id)?;
    Ok(Json(json!({ "ok": true, "removed": n })))
}

async fn project_export(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let v = db.export_project(id).map_err(bad_request)?;
    let name: String = db
        .project(id)?
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(download(
        serde_json::to_string_pretty(&v).unwrap_or_default(),
        "application/json",
        &format!("mbusio-{name}.json"),
    ))
}

async fn project_import(
    State(st): State<AppState>,
    Json(v): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let id = st
        .db
        .lock()
        .unwrap()
        .import_project(&v)
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "project": id })))
}

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default)]
    format: String,
    #[serde(default)]
    lang: String,
}

async fn export_readings(
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
