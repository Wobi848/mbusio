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
/// eingestellt (Konzept Sicherung §5, automatische Sicherung), auch von Hand verschlüsselt
/// (`.mbusio`). Erzeugt und gesendet Stück für Stück, nie ganz im Arbeitsspeicher.
pub(super) async fn backup_download(State(st): State<AppState>) -> Result<Response, ApiError> {
    let (db_file, device) = st.backup_source()?;
    let secret = st.db.lock().unwrap().backup_config().secret;
    let (file, encrypted) = blocking(move || backup::make(&db_file, &secret)).await??;
    let (ext, mime) = if encrypted {
        ("mbusio", "application/octet-stream")
    } else {
        ("db", "application/vnd.sqlite3")
    };
    let name = format!(
        "mbusio-sicherung-{device}-{}.{ext}",
        chrono::Local::now().format("%Y-%m-%d")
    );
    let size = std::fs::metadata(&file).map_err(anyhow::Error::from)?.len();
    let mut response = download(file_body(file, size), mime, &name);
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, size.into());
    Ok(response)
}

/// Inhalt einer Zwischendatei (`size` Bytes) als Strom in Stücken; gelöscht, sobald das letzte
/// Stück gelesen ist (nach dem letzten Byte fragt der Server nicht mehr nach) oder ein Fehler kam.
/// Bricht der Browser ab, räumt der nächste Start auf (`backup::clean_temp`).
fn file_body(path: std::path::PathBuf, size: u64) -> axum::body::Body {
    use tokio::io::AsyncReadExt;
    struct Open {
        file: Option<tokio::fs::File>,
        path: std::path::PathBuf,
        left: u64,
    }
    let start = Open {
        file: None,
        path,
        left: size,
    };
    let stream = futures_util::stream::unfold(Some(start), |state| async move {
        let mut s = state?;
        let read = async {
            if s.file.is_none() {
                s.file = Some(tokio::fs::File::open(&s.path).await?);
            }
            let mut buf = vec![0u8; 64 * 1024];
            let n = s.file.as_mut().unwrap().read(&mut buf).await?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            buf.truncate(n);
            Ok(buf)
        };
        let chunk = read.await;
        s.left = s.left.saturating_sub(chunk.as_ref().map_or(0, |c| c.len() as u64));
        if chunk.is_err() || s.left == 0 {
            s.file = None;
            let removed = tokio::fs::remove_file(&s.path).await;
            return Some((chunk.and_then(|c| removed.map(|()| c.into())), None));
        }
        Some((chunk.map(axum::body::Bytes::from), Some(s)))
    });
    axum::body::Body::from_stream(stream)
}

/// Sicherung hochladen und prüfen: zeigt, was drin ist; eingespielt wird erst mit `restore/apply`.
/// Die Datei kommt Stück für Stück auf die Platte. Verschlüsselt: Passwort im Kopf
/// `X-Backup-Password` (prozent-kodiert, damit Umlaute erlaubt sind), sonst `needs_password` in der
/// Antwort, damit die Oberfläche danach fragen kann.
pub(super) async fn restore_upload(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Result<Response, ApiError> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let dir = data_dir(&st)?;
    let mut file = tokio::fs::File::create(backup::upload_file(&dir))
        .await
        .map_err(anyhow::Error::from)?;
    let mut chunks = body.into_data_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| bad_request(e.to_string()))?;
        file.write_all(&chunk).await.map_err(anyhow::Error::from)?;
    }
    file.flush().await.map_err(anyhow::Error::from)?;
    drop(file);
    let password = headers
        .get("x-backup-password")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| percent_encoding::percent_decode_str(v).decode_utf8().ok())
        .unwrap_or_default()
        .into_owned();
    match blocking(move || backup::accept(&dir, &password)).await? {
        Ok(summary) => Ok(Json(summary).into_response()),
        // die Oberfläche fragt (wieder) nach dem Passwort
        Err(backup::Refused::Password(error)) => Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error, "needs_password": true })),
        )
            .into_response()),
        Err(backup::Refused::Other(e)) => Err(bad_request(e)),
    }
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
    let (db_file, device) = st.backup_source()?;
    let result = blocking(move || backup::auto::run(&db_file, &device, &config)).await?;
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
        both(|| {
            tr(
                "Sicherung wird eingespielt, mbusio startet neu",
                "Restoring backup, mbusio restarts",
            )
        }),
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
        let d =
            std::env::temp_dir().join(format!("mbusio-webbackup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let db = crate::db::Db::open(&d).unwrap();
        (d, std::sync::Arc::new(Shared::new(None, db)))
    }

    async fn body_json(r: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap();
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
            // Umlaute: kommen prozent-kodiert im Kopf (HTTP-Köpfe sind nur ASCII)
            c.secret = "Grüße".into();
            c.folder = "/tmp".into();
            db.set_backup_config(&c).unwrap();
        }
        let dl = backup_download(State(st.clone()))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        let bytes = axum::body::to_bytes(dl.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(backup::crypt::is_encrypted(&bytes));
        // gelesen: die Zwischendatei neben der Datenbank ist weg
        let left = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("sicherung"))
            .count();
        assert_eq!(left, 0);

        let r = restore_upload(State(st.clone()), HeaderMap::new(), bytes.clone().into())
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(r).await["needs_password"], true);

        let mut wrong = HeaderMap::new();
        wrong.insert("x-backup-password", "falsch".parse().unwrap());
        let r = restore_upload(State(st.clone()), wrong, bytes.clone().into())
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        let answer = body_json(r).await;
        assert_eq!(answer["needs_password"], true);
        assert!(
            answer["error"].as_str().unwrap().contains("falsch"),
            "{answer}"
        );

        let mut right = HeaderMap::new();
        right.insert("x-backup-password", "Gr%C3%BC%C3%9Fe".parse().unwrap());
        let r = restore_upload(State(st.clone()), right, bytes.into())
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(r.status(), StatusCode::OK);
        let _ = restore_discard(State(st))
            .await
            .unwrap_or_else(|e| panic!("{}", e.1));
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
