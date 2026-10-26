//! Tests der E-Mail: Adressen, Nachricht, SMTP mit kleinem Server im Test (ohne TLS)

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

use super::*;

/// Kleiner SMTP-Server für eine Verbindung: bietet `auth` an (z.B. "PLAIN LOGIN"), nimmt nur
/// Passwort „geheim“; liefert alles, was der Client geschickt hat
pub(crate) fn fake_server(auth: &'static str) -> (u16, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut w = stream.try_clone().unwrap();
        let mut r = BufReader::new(stream);
        let mut seen = String::new();
        let mut say = |s: &str| w.write_all(format!("{s}\r\n").as_bytes()).unwrap();
        say("220 test ESMTP");
        let mut data = false;
        let mut login_step = 0;
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap() == 0 {
                break;
            }
            seen.push_str(&line);
            let l = line.trim_end();
            if data {
                if l == "." {
                    data = false;
                    say("250 queued");
                }
                continue;
            }
            match login_step {
                1 => {
                    login_step = 2;
                    say("334 UGFzc3dvcmQ6");
                    continue;
                }
                2 => {
                    login_step = 0;
                    say(if l == base64(b"geheim") {
                        "235 ok"
                    } else {
                        "535 bad"
                    });
                    continue;
                }
                _ => {}
            }
            let upper = l.to_ascii_uppercase();
            if upper.starts_with("EHLO") {
                say("250-test");
                say(&format!("250 AUTH {auth}"));
            } else if let Some(token) = l.strip_prefix("AUTH PLAIN ") {
                say(if token == base64(b"\0anna\0geheim") {
                    "235 ok"
                } else {
                    "535 bad"
                });
            } else if upper == "AUTH LOGIN" {
                login_step = 1;
                say("334 VXNlcm5hbWU6");
            } else if upper.starts_with("MAIL FROM") || upper.starts_with("RCPT TO") {
                say("250 ok");
            } else if upper == "DATA" {
                data = true;
                say("354 go");
            } else if upper == "QUIT" {
                say("221 bye");
                break;
            } else {
                say("500 what");
            }
        }
        seen
    });
    (port, handle)
}

pub(crate) fn config(port: u16, password: &str) -> MailConfig {
    MailConfig {
        host: "127.0.0.1".into(),
        port,
        security: "none".into(),
        username: "anna".into(),
        password: password.into(),
        from: "mbusio@example.org".into(),
        ..Default::default()
    }
}

fn message() -> Message {
    Message {
        from: "mbusio@example.org".into(),
        to: vec!["a@example.org".into(), "b@example.org".into()],
        subject: "Abrechnung Müllerstrasse".into(),
        text: "Hallo".into(),
        attachments: vec![Attachment {
            name: "Müller-abrechnung.csv".into(),
            mime: "text/csv; charset=utf-8".into(),
            data: "Projekt;Stand\r\nA;1,5\r\n".into(),
        }],
    }
}

#[test]
fn base64_like_rfc_4648() {
    let cases = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foobar", "Zm9vYmFy"),
    ];
    for (a, b) in cases {
        assert_eq!(base64(a.as_bytes()), b);
    }
}

#[test]
fn addresses_and_recipients() {
    assert!(is_address("a.b@example.org"));
    for bad in [
        "a@b",
        "@x.de",
        "a b@x.de",
        "a@x.de>\r\nBcc: c@y.de",
        "a@.de",
    ] {
        assert!(!is_address(bad), "{bad}");
    }
    assert_eq!(
        recipients(" a@x.de; b@y.de ,").unwrap(),
        ["a@x.de", "b@y.de"]
    );
    assert!(recipients("a@x.de, kaputt").is_err());
    assert!(MailConfig::default().sender().is_err(), "ohne Server");
}

#[test]
fn message_has_encoded_subject_and_attachment() {
    let m = message().render("Mon, 26 Oct 2026 08:00:00 +0100", "1");
    assert!(m.contains("To: a@example.org, b@example.org\r\n"));
    assert!(m.contains(&format!(
        "Subject: =?UTF-8?B?{}?=\r\n",
        base64("Abrechnung Müllerstrasse".as_bytes())
    )));
    assert!(m.contains(
        "filename=\"M_ller-abrechnung.csv\"; filename*=UTF-8''M%C3%BCller-abrechnung.csv"
    ));
    assert!(m.contains(&base64(b"Projekt;Stand\r\nA;1,5\r\n")));
    assert!(m.ends_with("--mbusio-1--\r\n"));
}

#[test]
fn smtp_sends_with_plain_and_login() {
    for auth in ["PLAIN LOGIN", "LOGIN"] {
        let (port, server) = fake_server(auth);
        send(&config(port, "geheim"), &message()).unwrap();
        let seen = server.join().unwrap();
        assert!(seen.contains("MAIL FROM:<mbusio@example.org>"), "{seen}");
        assert!(seen.contains("RCPT TO:<b@example.org>"));
        assert!(seen.contains("Content-Disposition: attachment"));
        assert!(!seen.contains("geheim"), "Passwort nie im Klartext");
    }
}

#[test]
fn smtp_wrong_password_says_so_without_password() {
    let (port, server) = fake_server("PLAIN");
    let e = format!(
        "{:#}",
        send(&config(port, "falsch"), &message()).unwrap_err()
    );
    assert!(
        e.contains("Passwort falsch") || e.contains("bad user name"),
        "{e}"
    );
    assert!(
        !e.contains("falsch\0") && !e.contains(&base64(b"\0anna\0falsch")),
        "{e}"
    );
    drop(server);
}

/// Jede Mail steht im Protokoll der Weitergabe (Ausgang email) und als letzter Versand
#[test]
fn record_goes_to_log_and_card() {
    let mut db = Db::memory().unwrap();
    let m = message();
    db.mail_record(&m, &Ok(())).unwrap();
    assert_eq!(db.mail_last().unwrap().error, None);
    db.mail_record(&m, &Err(anyhow::anyhow!("Server lehnt ab")))
        .unwrap();
    assert_eq!(
        db.mail_last().unwrap().error.as_deref(),
        Some("Server lehnt ab")
    );
    let f = crate::datapoint::LogFilter {
        output: "email".into(),
        ..Default::default()
    };
    let page = db.out_log_page(&f, Vec::new()).unwrap();
    assert_eq!((page.counts["sent"], page.counts["failed"]), (1, 1));
    let r = &page.rows[0];
    assert_eq!(
        (r.name.as_str(), r.address.as_str(), r.value.as_str()),
        (
            "Abrechnung Müllerstrasse",
            "a@example.org, b@example.org",
            "Müller-abrechnung.csv"
        )
    );
    assert_eq!(
        (r.result.as_str(), r.reason.as_str()),
        ("failed", "Server lehnt ab")
    );
    // ausgeschaltet: nicht bereit, mit Hinweis wo
    let off = MailConfig {
        enabled: false,
        ..config(25, "x")
    };
    assert!(format!("{:#}", off.ready().unwrap_err()).contains("ausgeschaltet"));
    assert_eq!(config(25, "x").ready().unwrap(), "mbusio@example.org");
}
