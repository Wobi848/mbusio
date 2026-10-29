//! Sicherung des Geräts (Konzept Sicherung): herunterladen, hochladen und prüfen, einspielen
//! (mbusio startet dafür neu)

use super::*;
use crate::backup::{self, Summary};

/// Datenordner (dort liegen Datenbank und bereitgelegte Sicherung)
fn data_dir(st: &AppState) -> Result<std::path::PathBuf, ApiError> {
    st.db
        .lock()
        .unwrap()
        .path()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| bad_request(tr("Keine Datenbankdatei", "No database file")))
}

/// Ganze Datenbank als Datei `mbusio-sicherung-<gerät>-<datum>.db`
pub(super) async fn backup_download(State(st): State<AppState>) -> Result<Response, ApiError> {
    let dir = data_dir(&st)?;
    let tmp = dir.join("mbusio.db.sicherung");
    let device = {
        let db = st.db.lock().unwrap();
        if tmp.exists() {
            std::fs::remove_file(&tmp).map_err(anyhow::Error::from)?;
        }
        db.backup_to(&tmp)?;
        db.mail_config().device_name()
    };
    let body = std::fs::read(&tmp).map_err(anyhow::Error::from);
    std::fs::remove_file(&tmp).map_err(anyhow::Error::from)?;
    let device = backup::safe_device_name(&device);
    let name = format!(
        "mbusio-sicherung-{device}-{}.db",
        chrono::Local::now().format("%Y-%m-%d")
    );
    Ok(download(body?, "application/vnd.sqlite3", &name))
}

/// Sicherung hochladen und prüfen: zeigt, was drin ist; eingespielt wird erst mit `restore/apply`
pub(super) async fn restore_upload(
    State(st): State<AppState>,
    body: axum::body::Bytes,
) -> ApiResult<Summary> {
    let dir = data_dir(&st)?;
    Ok(Json(backup::upload(&dir, &body).map_err(bad_request)?))
}

/// Hochgeladene Sicherung verwerfen
pub(super) async fn restore_discard(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    backup::discard(&data_dir(&st)?)?;
    Ok(Json(json!({ "ok": true })))
}

/// Geprüfte Sicherung einspielen: bereitlegen und neu starten (nach der Antwort)
pub(super) async fn restore_apply(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    backup::apply(&data_dir(&st)?).map_err(bad_request)?;
    st.log.push(
        "INFO",
        both(|| tr("Sicherung wird eingespielt, mbusio startet neu", "Restoring backup, mbusio restarts")),
    );
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        crate::service::restart();
    });
    Ok(Json(json!({ "ok": true })))
}
