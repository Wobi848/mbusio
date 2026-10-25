//! Scan: Primär- und Sekundärscan, freie Adresse, Baudraten am Bus, letzter Scan

use super::*;

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
