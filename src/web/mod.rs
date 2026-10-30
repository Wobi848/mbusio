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
use crate::i18n::{self, both, tr, Lang};
use crate::line::{is_link_error, ConnInfo, LineConf, LineService, Ports};
use crate::link;
use crate::mbus::decode::decode_rsp_ud;
use crate::mbus::frame::{hex, Frame};
use crate::mbus::master::{
    mask_matches, Bauds, FrameLog, Master, MeterReading, Progress, Reply, ScanHit, Target,
};
use crate::scheduler::Planner;

mod api;
mod backup;
mod billing;
mod bus;
mod datapoints;
mod mail;
mod meters;
mod nodered;
mod outputs;
mod projects;
mod scan;
mod schedule;
mod simulator;
mod state;
mod tools;
mod warnings;

use self::api::*;
pub use self::state::{AppState, JobStatus, Shared};
pub use self::tools::parse_hex;
use self::{
    backup::*, billing::*, bus::*, datapoints::*, mail::*, meters::*, nodered::*, outputs::*,
    projects::*, scan::*, schedule::*, simulator::*, tools::*, warnings::*,
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
        .route("/api/exchanges/{id}/confirm", post(swap_confirm))
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
        .route(
            "/api/projects/{id}/billing",
            get(billing_get).put(billing_put),
        )
        .route("/api/projects/{id}/billing/csv", get(billing_csv))
        .route("/api/projects/{id}/billing/files/{name}", get(billing_file))
        .route(
            "/api/projects/{id}/billing/files/{name}/send",
            post(billing_file_send),
        )
        .route("/api/projects/{id}/billing/mail", post(billing_mail))
        .route("/api/mail", get(mail_get).put(mail_put))
        .route("/api/mail/test", post(mail_test))
        .route("/api/mail/report", post(mail_report))
        .route("/api/warnings", get(warnings_get))
        .route("/api/warnings/seen", post(warnings_seen))
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
        .route("/api/storage", get(storage_get).put(storage_put))
        .route("/api/backup", get(backup_download))
        .route(
            "/api/backup/auto",
            get(backup_auto_get).put(backup_auto_put),
        )
        .route("/api/backup/auto/test", post(backup_auto_test))
        // eine Sicherung mit Jahren an Messwerten ist gross (ohne Grenze lehnt axum ab 2 MB ab)
        .route(
            "/api/restore",
            post(restore_upload)
                .delete(restore_discard)
                .layer(axum::extract::DefaultBodyLimit::max(4 << 30)),
        )
        .route("/api/restore/apply", post(restore_apply))
        .route("/api/mqtt", get(mqtt_get).put(mqtt_put))
        .route("/api/modbus", get(modbus_get).put(modbus_put))
        .route("/api/out-log", get(out_log))
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            notify_changes,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .fallback(static_file)
        .with_state(state)
}

/// Nach jeder erfolgreichen ändernden Anfrage (POST, PUT, DELETE): Empfänger (Node-RED) gleichen
/// ab. So kommen auch neue Namen von Projekt, Linie oder Zähler an, nicht nur neue Werte.
async fn notify_changes(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let changes = req.method() != axum::http::Method::GET;
    let res = next.run(req).await;
    if changes && res.status().is_success() {
        st.db.lock().unwrap().notify();
    }
    res
}

/// Ändernde Anfrage einer fremden Webseite (CSRF)? Der Browser schickt sie auch ohne Rückfrage an
/// mbusio, z.B. ein POST „Projekt leeren“ an 127.0.0.1, aber immer mit dem `Origin` jener Seite.
/// Programme ohne Browser (Node-RED, curl) schicken kein `Origin` und bleiben erlaubt.
fn foreign_origin(req: &Request) -> bool {
    use axum::http::Method;
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    let Some(origin) = req.headers().get(header::ORIGIN) else {
        return false;
    };
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|o| o.split_once("://"))
        .map(|(_, h)| h);
    origin_host.is_none() || origin_host != host
}

/// Prüft den Zugriffsschlüssel (Header `X-Token` oder `?token=`), falls einer gesetzt ist.
async fn auth(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let lang = lang_of(&req);
    // die Oberfläche schickt ihre Sprache immer mit; andere Programme (Node-RED) ändern sie nicht
    // gespeichert, damit Meldungen gleich nach einem Neustart schon in dieser Sprache entstehen
    if req.headers().contains_key("x-lang") && i18n::remember_ui(lang) {
        if let Err(e) = st.db.lock().unwrap().set_meta("ui_lang", lang.code()) {
            st.log.push("INFO", format!("DB: {e:#}"));
        }
    }
    i18n::scope(lang, check_token(st, req, next)).await
}

async fn check_token(st: AppState, req: Request, next: Next) -> Response {
    if foreign_origin(&req) {
        return ApiError(
            StatusCode::FORBIDDEN,
            tr(
                "Anfrage einer fremden Webseite abgelehnt",
                "Request from another website refused",
            ),
        )
        .into_response();
    }
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

/// Dateien der Oberfläche. Ein Verzeichnis liefert seine `index.html` (`/`). Mit ETag aus
/// dem Inhalt: der Browser fragt nur nach, unveränderte Dateien kommen als 304 (Pi über WLAN).
async fn static_file(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let dir = path.trim_end_matches('/');
    let index = if dir.is_empty() {
        "index.html".to_string()
    } else {
        format!("{dir}/index.html")
    };
    let (path, file) = match Assets::get(path).filter(|_| !path.is_empty()) {
        Some(f) => (path.to_string(), f),
        None => match Assets::get(&index) {
            Some(f) => (index, f),
            None => return (StatusCode::NOT_FOUND, "Not found").into_response(),
        },
    };
    let etag = format!(
        "\"{}\"",
        file.metadata.sha256_hash()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let cache = [
        (header::ETAG, etag.clone()),
        (header::CACHE_CONTROL, "no-cache".to_string()),
    ];
    if headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|v| v.as_bytes() == etag.as_bytes())
    {
        return (StatusCode::NOT_MODIFIED, cache).into_response();
    }
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    (
        cache,
        [(header::CONTENT_TYPE, mime.as_ref().to_string())],
        file.data.into_owned(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(path: &str, etag: Option<&str>) -> Response {
        let mut headers = HeaderMap::new();
        if let Some(e) = etag {
            headers.insert(header::IF_NONE_MATCH, e.parse().unwrap());
        }
        static_file(path.parse().unwrap(), headers).await
    }

    #[test]
    fn refuses_changes_from_other_websites_only() {
        let req = |method: &str, origin: Option<&str>| {
            let mut b = Request::builder()
                .method(method)
                .uri("/api/projects/1/clear")
                .header(header::HOST, "127.0.0.1:8080");
            if let Some(o) = origin {
                b = b.header(header::ORIGIN, o);
            }
            b.body(axum::body::Body::empty()).unwrap()
        };
        // fremde Seite, auch „null“ (Datei, Sandbox): abgelehnt
        assert!(foreign_origin(&req("POST", Some("https://boese.example"))));
        assert!(foreign_origin(&req("POST", Some("null"))));
        assert!(foreign_origin(&req(
            "DELETE",
            Some("http://127.0.0.1:9999")
        )));
        // eigene Oberfläche, Programme ohne Origin, Lesen: erlaubt
        assert!(!foreign_origin(&req("POST", Some("http://127.0.0.1:8080"))));
        assert!(!foreign_origin(&req("POST", None)));
        assert!(!foreign_origin(&req("GET", Some("https://boese.example"))));
    }

    #[tokio::test]
    async fn static_files_with_index_and_etag() {
        for path in ["/", "/js/base.js"] {
            assert_eq!(get(path, None).await.status(), StatusCode::OK, "{path}");
        }
        assert_eq!(
            get("/gibt-es-nicht.js", None).await.status(),
            StatusCode::NOT_FOUND
        );
        // unverändert: 304 ohne Inhalt; anderes ETag: wieder 200
        let r = get("/js/base.js", None).await;
        let etag = r.headers()[header::ETAG].to_str().unwrap().to_string();
        assert_eq!(r.headers()[header::CACHE_CONTROL], "no-cache");
        assert_eq!(
            get("/js/base.js", Some(&etag)).await.status(),
            StatusCode::NOT_MODIFIED
        );
        assert_eq!(
            get("/js/base.js", Some("\"alt\"")).await.status(),
            StatusCode::OK
        );
    }
}
