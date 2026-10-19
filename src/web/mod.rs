//! HTTP-API und eingebettete Weboberfläche.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode, Uri};
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
use crate::line::{is_link_error, ConnInfo, LineConf, LineService, Ports};
use crate::link;
use crate::mbus::decode::decode_rsp_ud;
use crate::mbus::frame::{hex, Frame};
use crate::mbus::master::{
    mask_matches, Bauds, FrameLog, Master, MeterReading, Progress, Reply, ScanHit, Target,
};
use crate::scheduler::Planner;

mod bus;
mod datapoints;
mod meters;
mod outputs;
mod projects;
mod simulator;

pub use self::bus::parse_hex;
use self::{bus::*, datapoints::*, meters::*, outputs::*, projects::*, simulator::*};

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
    /// Bereich des Scans (Adressen, Baudraten); verglichen wird nur mit gleichem Bereich
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scope: String,
    /// Letzter vollständiger Scan gleicher Art und gleichen Bereichs, zum Vergleich: {time, hits}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<serde_json::Value>,
    /// Abgebrochener Sekundärscan: hier geht „Weitersuchen“ weiter {mask, baud}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<serde_json::Value>,
}

pub struct Shared {
    /// Linien-Dienste, je verbundener Linie einer; nur er spricht mit dem Bus dieser Linie
    lines: Mutex<HashMap<i64, LineService>>,
    /// Offene Anschlüsse aller Linien (zwei Linien, ein Port: abgelehnt)
    ports: Arc<Ports>,
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
            ports: Arc::default(),
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

    /// Linien-Dienst arbeitet gerade oder hat Aufträge in der Warteschlange
    fn line_busy(&self, line_id: i64) -> bool {
        let svc = self.lines.lock().unwrap().get(&line_id).cloned();
        svc.is_some_and(|s| {
            let st = s.status();
            st.busy || st.queued > 0
        })
    }

    /// Verbindung der aktiven Linie, falls offen
    fn conn_info(&self) -> Option<ConnInfo> {
        self.service().ok()?.status().connection
    }

    /// Linie mit diesem Anschluss verbinden; ein vorhandener Dienst der Linie wird ersetzt.
    /// Klappt die Verbindung nicht, bleibt die Linie getrennt und der Fehler kommt zurück.
    pub fn connect_line(&self, line_id: i64, conf: LineConf) -> anyhow::Result<()> {
        self.planner.release(line_id);
        // läuft schon mit diesem Anschluss (z.B. wegen des Ausleseplans): weiter benutzen
        let running = self.lines.lock().unwrap().get(&line_id).cloned();
        if let Some(svc) = running.filter(|s| *s.conf() == conf) {
            return svc.connect();
        }
        self.stop_line_wait(line_id);
        let svc = self.start_service(line_id, conf);
        match svc.connect() {
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

    /// Linien-Dienst starten; belegt seinen Anschluss erst beim Verbinden (`Ports`)
    fn start_service(&self, line_id: i64, conf: LineConf) -> LineService {
        let label = {
            let db = self.db.lock().unwrap();
            db.line(line_id)
                .and_then(|l| {
                    Ok((
                        l.name,
                        db.project(db.gateway(l.gateway_id)?.project_id)?.name,
                    ))
                })
                .map(|(l, p)| {
                    tr(
                        &format!("„{l}“ im Projekt „{p}“"),
                        &format!("“{l}” in project “{p}”"),
                    )
                })
                .unwrap_or_else(|_| line_id.to_string())
        };
        LineService::start(line_id, label, conf, self.log.clone(), self.ports.clone())
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
        let conf = LineConf::of(line);
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
        let svc = self.start_service(line.id, conf);
        self.lines.lock().unwrap().insert(line.id, svc.clone());
        Some(svc)
    }

    /// Alle Linien mit diesem Anschluss neu verbinden (z.B. weil die Simulation geändert wurde).
    /// Liefert die Anzahl.
    pub(crate) fn restart_connection(&self, connection: &str) -> usize {
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
                if let Err(e) = self.connect_line(*id, conf.clone()) {
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
/// Sprache eines Requests: `?lang=` (Downloads), sonst Header `X-Lang` bzw. `Accept-Language`
fn lang_of(req: &Request) -> Lang {
    if let Some(l) = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("lang=")))
    {
        return Lang::parse(l);
    }
    let headers = req.headers();
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
async fn with_bus<T, F>(state: &AppState, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Master) -> anyhow::Result<T> + Send + 'static,
{
    let svc = state.service()?;
    blocking(move || svc.run(f).map_err(ApiError::from)).await?
}

/// Blockierende Arbeit (Bus, Port öffnen) ausserhalb der Web-Threads ausführen
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    let lang = i18n::current();
    tokio::task::spawn_blocking(move || {
        i18n::set(lang);
        f()
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
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
        .route("/api/projects/{id}/clear", post(project_clear))
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
        .route(
            "/api/projects/{id}/outputs/{key}/readdress",
            post(output_readdress),
        )
        .route("/api/projects/{id}/outputs/{key}/list", get(output_list))
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
    i18n::scope(lang_of(&req), check_token(st, req, next)).await
}

async fn check_token(st: AppState, req: Request, next: Next) -> Response {
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

/// Liest einen Zähler der Liste, mit seiner eigenen Baudrate falls angegeben.
pub(crate) fn read_listed(m: &mut Master, meter: &Meter) -> anyhow::Result<(Target, MeterReading)> {
    let target = Target::parse(&meter.address)?;
    let r = m.at_baud(meter.baud, |m| m.read_meter(&target))?;
    Ok((target, r))
}

/// CSV für Excel (mit BOM für die Umlaute) als `mbusio-<was>-<name>.csv`
fn csv_download(csv: String, what: &str, name: &str) -> Response {
    let name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    download(
        format!("\u{feff}{csv}"),
        "text/csv; charset=utf-8",
        &format!("mbusio-{what}-{name}.csv"),
    )
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
