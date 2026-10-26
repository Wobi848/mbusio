//! Export für die Abrechnung: Einstellungen je Projekt, Werte zur Auswahl, Vorschau, CSV

use super::*;
use crate::billing::{self, Settings};

/// Zeilen der Vorschau
const PREVIEW: usize = 30;

/// Dateien des automatischen Exports (Datenbank im Speicher: keine)
fn files(db: &Db, id: i64) -> Result<Vec<billing::auto::ExportFile>, ApiError> {
    match db.path().and_then(|p| p.parent()) {
        Some(dir) => Ok(billing::auto::export_files(dir, id)?),
        None => Ok(Vec::new()),
    }
}

/// Einstellungen, Werte zur Auswahl, Vorschau und abgelegte Dateien; ein unvollständiger Zeitraum ist kein Fehler,
/// sondern ein Hinweis an der Vorschau (beim Tippen)
fn billing_view(db: &Db, id: i64) -> Result<serde_json::Value, ApiError> {
    let s = db.billing_settings(id);
    let (preview, error) = match db.billing_table(id, &s, clock::now().date(), Some(PREVIEW)) {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(format!("{e:#}"))),
    };
    Ok(json!({
        "settings": s,
        "kinds": db.billing_kinds(id)?,
        "preview": preview,
        "error": error,
        "files": files(db, id)?,
    }))
}

pub(super) async fn billing_get(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let db = st.db.lock().unwrap();
    db.project(id).map_err(bad_request)?;
    Ok(Json(billing_view(&db, id)?))
}

pub(super) async fn billing_put(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(s): Json<Settings>,
) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    db.set_billing_settings(id, &s).map_err(bad_request)?;
    if s.auto {
        st.planner.export_soon();
    }
    Ok(Json(billing_view(&db, id)?))
}

/// Ganze Tabelle nach den gespeicherten Einstellungen als CSV für Excel
pub(super) async fn billing_csv(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let s = db.billing_settings(id);
    let t = db
        .billing_table(id, &s, clock::now().date(), None)
        .map_err(bad_request)?;
    let what = if s.form == "series" {
        tr("verlauf", "series")
    } else {
        tr("abrechnung", "billing")
    };
    let name = format!("{}-{}-{}", db.project(id)?.name, t.from, t.to);
    Ok(csv_download(billing::csv(&t), &what, &name))
}

/// Datei des automatischen Exports; nur Namen aus der Liste, sonst nicht gefunden
pub(super) async fn billing_file(
    State(st): State<AppState>,
    Path((id, name)): Path<(i64, String)>,
) -> Result<Response, ApiError> {
    let db = st.db.lock().unwrap();
    let dir = db.path().and_then(|p| p.parent()).map(|p| p.to_path_buf());
    let known = files(&db, id)?.iter().any(|f| f.name == name);
    drop(db);
    let (Some(dir), true) = (dir, known) else {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            tr("Datei nicht gefunden", "File not found"),
        ));
    };
    let body = std::fs::read_to_string(billing::auto::export_dir(&dir, id).join(&name))
        .map_err(anyhow::Error::from)?;
    Ok(download(body, "text/csv; charset=utf-8", &name))
}
