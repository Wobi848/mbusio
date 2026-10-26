//! E-Mail-Server (einmal je Gerät): Einstellungen ohne Passwort, speichern, Testmail

use super::*;
use crate::mail::{self, MailConfig, Message};

/// Einstellungen ohne Passwort, dazu ob eines gespeichert ist
pub(super) async fn mail_get(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let mut c = st.db.lock().unwrap().mail_config();
    let has_password = !c.password.is_empty();
    c.password.clear();
    Ok(Json(json!({ "config": c, "has_password": has_password })))
}

#[derive(Deserialize)]
pub(super) struct MailReq {
    config: MailConfig,
    /// None = gespeichertes Passwort behalten, "" = löschen
    #[serde(default)]
    password: Option<String>,
}

pub(super) async fn mail_put(
    State(st): State<AppState>,
    Json(r): Json<MailReq>,
) -> ApiResult<serde_json::Value> {
    let mut db = st.db.lock().unwrap();
    let mut c = r.config;
    c.password = r.password.unwrap_or_else(|| db.mail_config().password);
    c.host = c.host.trim().to_string();
    c.from = c.from.trim().to_string();
    db.set_mail_config(&c).map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(super) struct TestReq {
    to: String,
}

/// Kurze Nachricht mit den gespeicherten Einstellungen; der Fehler kommt sofort zurück
pub(super) async fn mail_test(
    State(st): State<AppState>,
    Json(r): Json<TestReq>,
) -> ApiResult<serde_json::Value> {
    let to = mail::recipients(&r.to).map_err(bad_request)?;
    if to.is_empty() {
        return Err(bad_request(tr(
            "Keine Empfänger eingetragen",
            "No recipients entered",
        )));
    }
    let config = st.db.lock().unwrap().mail_config();
    let message = Message {
        from: config.sender().map_err(bad_request)?,
        to,
        subject: format!("mbusio: {}", tr("Testmail", "test e-mail")),
        text: tr(
            "So kommen die Abrechnungen an.",
            "This is how the billing exports will arrive.",
        ),
        attachments: Vec::new(),
    };
    blocking(move || mail::send(&config, &message))
        .await?
        .map_err(bad_request)?;
    Ok(Json(json!({ "ok": true })))
}
