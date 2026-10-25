//! Antworten der API: Fehler, Sprache, Bus belegt oder nicht verbunden, Arbeit am Bus,
//! Downloads.

use super::*;

pub(super) fn bad_request(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, format!("{e:#}"))
}

pub(super) fn parse_target(address: &str) -> Result<Target, ApiError> {
    Target::parse(address).map_err(bad_request)
}

/// Sprache eines Requests: `?lang=` (Downloads), sonst Header `X-Lang` bzw. `Accept-Language`
pub(super) fn lang_of(req: &Request) -> Lang {
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

pub struct ApiError(pub(super) StatusCode, pub(super) String);

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

pub(super) type ApiResult<T> = Result<Json<T>, ApiError>;

pub(super) fn busy() -> ApiError {
    ApiError(
        StatusCode::CONFLICT,
        tr(
            "Bus ist belegt (Scan läuft?)",
            "Bus is busy (scan running?)",
        ),
    )
}

pub(super) fn not_connected() -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        tr("Nicht verbunden", "Not connected"),
    )
}

/// Führt `f` auf dem Bus der aktiven Linie aus: als Auftrag an ihren Linien-Dienst, der ihn
/// nach den laufenden Aufträgen abarbeitet (kein "Bus belegt").
pub(super) async fn with_bus<T, F>(state: &AppState, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Master) -> anyhow::Result<T> + Send + 'static,
{
    let svc = state.service()?;
    blocking(move || svc.run(f).map_err(ApiError::from)).await?
}

/// Blockierende Arbeit (Bus, Port öffnen) ausserhalb der Web-Threads ausführen
pub(super) async fn blocking<T: Send + 'static>(
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

/// CSV für Excel (mit BOM für die Umlaute) als `mbusio-<was>-<name>.csv`
pub(super) fn csv_download(csv: String, what: &str, name: &str) -> Response {
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

pub(super) fn download(body: String, mime: &str, filename: &str) -> Response {
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

pub(super) fn ensure_idle(st: &AppState) -> Result<(), ApiError> {
    if st.job.lock().unwrap().running {
        return Err(busy());
    }
    Ok(())
}

/// Verbindungen trennen, z.B. weil eine andere Linie aktiv wird; Linien mit eingeschaltetem
/// Ausleseplan bleiben verbunden.
pub(super) fn disconnect_bus(st: &AppState) {
    st.stop_lines();
    *st.job.lock().unwrap() = JobStatus::default();
}
