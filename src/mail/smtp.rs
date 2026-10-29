//! Eigener kleiner SMTP-Client: EHLO, STARTTLS bzw. TLS, AUTH PLAIN oder LOGIN, eine Nachricht

use std::io::Write;
use std::sync::Arc;

use anyhow::{bail, Result};

use super::{base64, MailConfig, Message};
use crate::i18n::tr;
use crate::textproto::{connect, tls, Conn, Session};

/// Nachricht senden; Fehler nennen den Schritt und die Antwort des Servers, nie das Passwort
pub fn send(c: &MailConfig, m: &Message) -> Result<()> {
    let host = c.host.trim();
    let tls_config = Arc::new(crate::tls::config(c.insecure, "")?);
    let tcp = connect(host, c.port)?;
    let conn = if c.security == "tls" {
        tls(tls_config.clone(), host, tcp)?
    } else {
        Conn::Plain(tcp)
    };
    let mut s = Session::new(conn);
    s.expect(tr("Begrüssung", "greeting").as_str(), &[220])?;
    let mut caps = s.cmd("EHLO mbusio", "EHLO", &[250])?;
    if c.security == "starttls" {
        if !caps.iter().any(|l| l.eq_ignore_ascii_case("STARTTLS")) {
            bail!(tr(
                "Server kann kein STARTTLS (Verschlüsselung „TLS“ oder „keine“ wählen)",
                "Server does not offer STARTTLS (choose encryption “TLS” or “none”)"
            ));
        }
        s.cmd("STARTTLS", "STARTTLS", &[220])?;
        s = s.upgrade(tls_config, host)?;
        caps = s.cmd("EHLO mbusio", "EHLO", &[250])?;
    }
    if !c.username.is_empty() {
        login(&mut s, c, &caps)?;
    }
    let from = c.sender()?;
    s.cmd(&format!("MAIL FROM:<{from}>"), &from, &[250])?;
    for to in &m.to {
        s.cmd(&format!("RCPT TO:<{to}>"), to, &[250, 251])?;
    }
    s.cmd("DATA", "DATA", &[354])?;
    let id = format!(
        "{}.{}",
        chrono::Local::now().format("%Y%m%d%H%M%S%f"),
        std::process::id()
    );
    let body = m.render(&chrono::Local::now().to_rfc2822(), &id);
    s.write_all(body.as_bytes())?;
    s.cmd(".", tr("Nachricht", "message").as_str(), &[250])?;
    // Antwort auf QUIT ist egal: die Nachricht ist angenommen
    let _ = s.cmd("QUIT", "QUIT", &[221]);
    Ok(())
}

fn login(s: &mut Session, c: &MailConfig, caps: &[String]) -> Result<()> {
    let auth = caps
        .iter()
        .find(|l| l.to_ascii_uppercase().starts_with("AUTH"))
        .map(|l| l.to_ascii_uppercase())
        .unwrap_or_default();
    let shown = tr("Anmeldung", "login");
    if auth.contains("PLAIN") || !auth.contains("LOGIN") {
        let token = base64(format!("\0{}\0{}", c.username, c.password).as_bytes());
        s.cmd(&format!("AUTH PLAIN {token}"), &shown, &[235])?;
    } else {
        s.cmd("AUTH LOGIN", &shown, &[334])?;
        s.cmd(&base64(c.username.as_bytes()), &shown, &[334])?;
        s.cmd(&base64(c.password.as_bytes()), &shown, &[235])?;
    }
    Ok(())
}
