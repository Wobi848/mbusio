//! Sicherung des Geräts (Konzept Sicherung): herunterladen, hochladen und prüfen, einspielen
//! (mbusio startet dafür neu)

use super::*;
use crate::backup;

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

/// Ganze Datenbank als Datei `mbusio-sicherung-<gerät>-<datum>.db`; ist ein Sicherungs-Passwort
/// eingestellt (Konzept Sicherung §5, automatische Sicherung), auch von Hand verschlüsselt (`.mbusio`)
pub(super) async fn backup_download(State(st): State<AppState>) -> Result<Response, ApiError> {
    let dir = data_dir(&st)?;
    let tmp = dir.join("mbusio.db.sicherung");
    let (device, secret) = {
        let db = st.db.lock().unwrap();
        if tmp.exists() {
            std::fs::remove_file(&tmp).map_err(anyhow::Error::from)?;
        }
        db.backup_to(&tmp)?;
        (db.mail_config().device_name(), db.backup_config().secret)
    };
    let plain = std::fs::read(&tmp).map_err(anyhow::Error::from)?;
    std::fs::remove_file(&tmp).map_err(anyhow::Error::from)?;
    let device = backup::safe_device_name(&device);
    let (body, ext, mime) = if secret.is_empty() {
        (plain, "db", "application/vnd.sqlite3")
    } else {
        (
            backup::crypt::encrypt(&secret, &plain)?,
            "mbusio",
            "application/octet-stream",
        )
    };
    let name = format!(
        "mbusio-sicherung-{device}-{}.{ext}",
        chrono::Local::now().format("%Y-%m-%d")
    );
    Ok(download(body, mime, &name))
}

/// Sicherung hochladen und prüfen: zeigt, was drin ist; eingespielt wird erst mit `restore/apply`.
/// Verschlüsselt (erkannt an `MBUSIOV1`): Passwort im Kopf `X-Backup-Password`, sonst
/// `needs_password` in der Antwort, damit die Oberfläche danach fragen kann.
pub(super) async fn restore_upload(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, ApiError> {
    let dir = data_dir(&st)?;
    let needs_password = || {
        Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": tr("Sicherung ist verschlüsselt: Passwort nötig", "Backup is encrypted: password required"),
                "needs_password": true,
            })),
        )
            .into_response())
    };
    let plain = if backup::crypt::is_encrypted(&body) {
        let password = headers
            .get("x-backup-password")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if password.is_empty() {
            return needs_password();
        }
        match backup::crypt::decrypt(password, &body) {
            Ok(d) => d,
            Err(_) => return needs_password(),
        }
    } else {
        body.to_vec()
    };
    let summary = backup::upload(&dir, &plain).map_err(bad_request)?;
    Ok(Json(summary).into_response())
}

/// Einstellungen der automatischen Sicherung ohne Passwort/Sicherungs-Passwort, dazu ob je eines
/// gespeichert ist, und das letzte Ergebnis
pub(super) async fn backup_auto_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut c = st.db.lock().unwrap().backup_config();
    let has_password = !c.password.is_empty();
    let has_secret = !c.secret.is_empty();
    c.password.clear();
    c.secret.clear();
    let state = st.db.lock().unwrap().backup_state();
    Ok(Json(json!({
        "config": c, "has_password": has_password, "has_secret": has_secret, "state": state,
    })))
}

#[derive(Deserialize)]
pub(super) struct BackupAutoReq {
    config: crate::backup::auto::BackupConfig,
    /// None = gespeichertes Passwort/Sicherungs-Passwort behalten, "" = löschen
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    secret: Option<String>,
}

pub(super) async fn backup_auto_put(
    State(st): State<AppState>,
    Json(r): Json<BackupAutoReq>,
) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let mut c = r.config;
    let saved = db.backup_config();
    c.password = r.password.unwrap_or(saved.password);
    c.secret = r.secret.unwrap_or(saved.secret);
    c.folder = c.folder.trim().to_string();
    c.host = c.host.trim().to_string();
    c.dir = c.dir.trim().to_string();
    c.user = c.user.trim().to_string();
    db.set_backup_config(&c).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

/// Jetzt sichern, mit den gespeicherten Einstellungen; der Fehler kommt sofort zurück
pub(super) async fn backup_auto_test(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let config = st.db.lock().unwrap().backup_config();
    config.ready().map_err(bad_request)?;
    let target = config.target_label();
    let device = backup::safe_device_name(&st.db.lock().unwrap().mail_config().device_name());
    let st2 = st.clone();
    let result = blocking(move || -> anyhow::Result<u64> {
        let bytes = backup::auto::build(&st2.db.lock().unwrap())?;
        backup::auto::store(&config, &device, &bytes)
    })
    .await?;
    let mut db = st.db.lock().unwrap();
    db.backup_record(&target, &result, crate::clock::now())?;
    let bytes = result.map_err(bad_request)?;
    Ok(Json(json!({ "ok": true, "bytes": bytes })))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str) -> (std::path::PathBuf, AppState) {
        let d = std::env::temp_dir().join(format!("mbusio-webbackup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let db = crate::db::Db::open(&d).unwrap();
        (d, std::sync::Arc::new(Shared::new(None, db)))
    }

    async fn body_json(r: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Mit gesetztem Sicherungs-Passwort ist „Herunterladen“ verschlüsselt; „Einspielen“ erkennt
    /// das und fragt erst mit, dann ohne Kopfzeile das Passwort ab
    #[tokio::test]
    async fn download_encrypted_upload_needs_password_then_works() {
        let (dir, st) = state("passwort");
        {
            let mut db = st.db.lock().unwrap();
            let mut c = db.backup_config();
            c.secret = "geheim".into();
            c.folder = "/tmp".into();
            db.set_backup_config(&c).unwrap();
        }
        let dl = backup_download(State(st.clone()))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        let bytes = axum::body::to_bytes(dl.into_body(), usize::MAX).await.unwrap();
        assert!(backup::crypt::is_encrypted(&bytes));

        let r = restore_upload(State(st.clone()), HeaderMap::new(), bytes.clone())
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(r).await["needs_password"], true);

        let mut wrong = HeaderMap::new();
        wrong.insert("x-backup-password", "falsch".parse().unwrap());
        let r = restore_upload(State(st.clone()), wrong, bytes.clone())
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(body_json(r).await["needs_password"], true);

        let mut right = HeaderMap::new();
        right.insert("x-backup-password", "geheim".parse().unwrap());
        let r = restore_upload(State(st.clone()), right, bytes)
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.status(), StatusCode::OK);
        let _ = restore_discard(State(st)).await.unwrap_or_else(|e| panic!("{}", e.1));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Einstellungen speichern (Sicherungs-Passwort separat), „Jetzt sichern“ legt die
    /// verschlüsselte Datei im Ordner ab
    #[tokio::test]
    async fn auto_config_roundtrip_and_test_now() {
        let (dir, st) = state("auto");
        let target = dir.join("ziel");
        std::fs::create_dir_all(&target).unwrap();

        let r = backup_auto_get(State(st.clone()))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.0["has_secret"], false);

        let req = BackupAutoReq {
            config: backup::auto::BackupConfig {
                enabled: true,
                target: "folder".into(),
                folder: target.to_string_lossy().into_owned(),
                keep: 3,
                ..Default::default()
            },
            password: None,
            secret: Some("geheim".into()),
        };
        let _ = backup_auto_put(State(st.clone()), Json(req))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));

        let r = backup_auto_get(State(st.clone()))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.0["has_secret"], true);
        assert_eq!(r.0["config"]["secret"], "");

        let r = backup_auto_test(State(st.clone()))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.0["ok"], true);
        let files: Vec<String> = std::fs::read_dir(&target)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".mbusio"));
        assert_eq!(st.db.lock().unwrap().backup_state().error, None);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
