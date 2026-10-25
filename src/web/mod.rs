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

mod api;
mod bus;
mod datapoints;
mod meters;
mod nodered;
mod outputs;
mod projects;
mod scan;
mod simulator;
mod state;
mod tools;

use self::api::*;
pub use self::state::{AppState, JobStatus, Shared};
pub use self::tools::parse_hex;
use self::{
    bus::*, datapoints::*, meters::*, nodered::*, outputs::*, projects::*, scan::*, simulator::*,
    tools::*,
};

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

/// Höchstens so viele Auslesungen einer Linie an die Oberfläche bzw. in den Export
const MAX_READINGS: usize = 5000;

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
        .route("/api/modbus", get(modbus_get).put(modbus_put))
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
        .route("/api/lines/{id}/operation", put(line_operation))
        .route("/api/meters/{id}/plan", put(meter_plan))
        .route("/api/time", post(set_time))
        .route("/api/nodered/tree", get(nodered_tree))
        .route("/api/nodered/stream", get(nodered_stream))
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
