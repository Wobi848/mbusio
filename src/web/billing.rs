//! Export für die Abrechnung: Einstellungen je Projekt, Werte zur Auswahl, Vorschau, CSV

use super::*;
use crate::billing::{self, Settings};

/// Zeilen der Vorschau
const PREVIEW: usize = 30;

/// Datenordner (Datenbank im Speicher: keiner, also auch keine Dateien)
fn data_dir(db: &Db) -> Option<std::path::PathBuf> {
    db.path().and_then(|p| p.parent()).map(|p| p.to_path_buf())
}

/// Dateien des automatischen Exports mit Zustand der E-Mail
fn files(db: &Db, id: i64) -> Result<Vec<billing::mail::FileView>, ApiError> {
    match data_dir(db) {
        Some(dir) => Ok(db.billing_files(&dir, id)?),
        None => Ok(Vec::new()),
    }
}

/// Datenordner, wenn `name` eine Datei aus der Liste ist; sonst nicht gefunden (kein „../“)
fn known_file(db: &Db, id: i64, name: &str) -> Result<std::path::PathBuf, ApiError> {
    match data_dir(db) {
        Some(dir) if files(db, id)?.iter().any(|f| f.file.name == name) => Ok(dir),
        _ => Err(ApiError(
            StatusCode::NOT_FOUND,
            tr("Datei nicht gefunden", "File not found"),
        )),
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
        "lines": db.billing_lines(id)?,
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

/// Datei des automatischen Exports herunterladen
pub(super) async fn billing_file(
    State(st): State<AppState>,
    Path((id, name)): Path<(i64, String)>,
) -> Result<Response, ApiError> {
    let dir = known_file(&st.db.lock().unwrap(), id, &name)?;
    let body = std::fs::read_to_string(billing::auto::export_dir(&dir, id).join(&name))
        .map_err(anyhow::Error::from)?;
    Ok(download(body, "text/csv; charset=utf-8", &name))
}

/// Datei jetzt an die Empfänger des Projekts senden; das Ergebnis steht danach an der Datei
pub(super) async fn billing_file_send(
    State(st): State<AppState>,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<serde_json::Value> {
    let outgoing = {
        let db = st.db.lock().unwrap();
        let dir = known_file(&db, id, &name)?;
        db.billing_outgoing(&dir, id, &name).map_err(bad_request)?
    };
    let result = blocking(move || crate::mail::send(&outgoing.config, &outgoing.message)).await?;
    let error = result.as_ref().err().map(|e| format!("{e:#}"));
    let db = st.db.lock().unwrap();
    if let Some(msg) = db.billing_mail_done(id, &name, result, clock::now())? {
        st.log.push("INFO", msg);
    }
    match error {
        Some(e) => Err(bad_request(e)),
        None => Ok(Json(billing_view(&db, id)?)),
    }
}
