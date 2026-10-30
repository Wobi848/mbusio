//! Test des FTP-Clients mit einem kleinen eigenen Server (ohne TLS, wie bei SMTP)

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::JoinHandle;

use super::{Ftp, FtpConfig};

fn fake_server() -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        run(stream);
    });
    (port, handle)
}

/// Nimmt nur Benutzer „anna“ mit Passwort „geheim“; „sicherung“ muss erst mit MKD angelegt
/// werden, danach ist CWD dorthin erlaubt; merkt sich hochgeladene Dateinamen für NLST/DELE
fn run(stream: TcpStream) {
    let mut w = stream.try_clone().unwrap();
    let mut r = BufReader::new(stream);
    let mut say = |s: &str| w.write_all(format!("{s}\r\n").as_bytes()).unwrap();
    say("220 test FTP");
    let mut known_dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut data: Option<TcpListener> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let l = line.trim_end();
        let upper = l.to_ascii_uppercase();
        if upper.starts_with("USER") {
            say("331 password please");
        } else if let Some(pw) = l.strip_prefix("PASS ") {
            say(if pw == "geheim" { "230 ok" } else { "530 bad" });
        } else if upper == "TYPE I" {
            say("200 ok");
        } else if let Some(name) = l.strip_prefix("CWD ") {
            if name == "/" || known_dirs.iter().any(|d| d == name) {
                say("250 ok");
            } else {
                say("550 no such dir");
            }
        } else if let Some(name) = l.strip_prefix("MKD ") {
            known_dirs.push(name.to_string());
            say("257 created");
        } else if upper == "PASV" {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            data = Some(listener);
            // absichtlich eine fremde IP wie hinter NAT: der Client muss die Adresse der
            // Steuerverbindung nehmen
            say(&format!(
                "227 Entering Passive Mode (10,99,99,99,{},{}).",
                port / 256,
                port % 256
            ));
        } else if let Some(name) = l.strip_prefix("STOR ") {
            say("150 go");
            let (mut d, _) = data.take().unwrap().accept().unwrap();
            let mut buf = Vec::new();
            d.read_to_end(&mut buf).unwrap();
            files.push(name.to_string());
            say("226 done");
        } else if upper == "NLST" {
            if files.is_empty() {
                say("450 no files");
                continue;
            }
            say("150 go");
            let (mut d, _) = data.take().unwrap().accept().unwrap();
            d.write_all(files.join("\r\n").as_bytes()).unwrap();
            drop(d);
            say("226 done");
        } else if let Some(name) = l.strip_prefix("DELE ") {
            files.retain(|f| f != name);
            say("250 deleted");
        } else if upper == "QUIT" {
            say("221 bye");
            break;
        } else {
            say("500 what");
        }
    }
}

fn config(port: u16) -> FtpConfig {
    FtpConfig {
        // Name statt IP: löst zu ::1 und 127.0.0.1 auf, der Server hört nur auf IPv4
        host: "localhost".into(),
        port,
        user: "anna".into(),
        password: "geheim".into(),
        tls: false,
        insecure: false,
    }
}

#[test]
fn upload_list_delete_and_missing_dir_gets_created() {
    let (port, handle) = fake_server();
    let mut ftp = Ftp::connect(&config(port)).unwrap();
    ftp.cd("sicherung").unwrap();
    ftp.store("eins.mbusio", b"geheime Daten").unwrap();
    assert_eq!(ftp.list().unwrap(), vec!["eins.mbusio"]);
    ftp.delete("eins.mbusio").unwrap();
    assert!(ftp.list().unwrap().is_empty());
    ftp.quit();
    handle.join().unwrap();
}

#[test]
fn wrong_password_is_refused() {
    let (port, handle) = fake_server();
    let mut c = config(port);
    c.password = "falsch".into();
    assert!(Ftp::connect(&c).is_err());
    handle.join().unwrap();
}

// ------------------------------------------------------------ FTPS

/// Nur für diesen Test: selbst signiert für „localhost“, 100 Jahre gültig, kein Geheimnis
const CERT: &[u8] = include_bytes!("test-cert.pem");
const KEY: &[u8] = include_bytes!("test-key.pem");

type Tls = rustls::StreamOwned<rustls::ServerConnection, TcpStream>;

fn tls_config() -> std::sync::Arc<rustls::ServerConfig> {
    use rustls::pki_types::pem::PemObject;
    let certs = rustls::pki_types::CertificateDer::pem_slice_iter(CERT)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = rustls::pki_types::PrivateKeyDer::from_pem_slice(KEY).unwrap();
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    // wie echte Server schickt rustls nach dem Handschlag Sitzungs-Tickets; liest der Client sie
    // auf der Datenverbindung nicht, kam die Datei früher abgeschnitten an
    std::sync::Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap(),
    )
}

fn accept_tls(config: &std::sync::Arc<rustls::ServerConfig>, tcp: TcpStream) -> Tls {
    rustls::StreamOwned::new(rustls::ServerConnection::new(config.clone()).unwrap(), tcp)
}

/// FTPS-Server (explizit, AUTH TLS) für eine Sitzung; liefert die Grössen der hochgeladenen Dateien
fn ftps_server() -> (u16, JoinHandle<Vec<usize>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let config = tls_config();
        let (mut plain, _) = listener.accept().unwrap();
        plain.write_all(b"220 test FTPS\r\n").unwrap();
        let mut line = String::new();
        BufReader::new(plain.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(line.trim_end(), "AUTH TLS");
        plain.write_all(b"234 go\r\n").unwrap();
        let mut ctrl = BufReader::new(accept_tls(&config, plain));
        let mut sizes = Vec::new();
        let mut data: Option<TcpListener> = None;
        loop {
            let mut line = String::new();
            if ctrl.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let l = line.trim_end().to_string();
            let mut say = |s: &str| {
                let w = ctrl.get_mut();
                w.write_all(format!("{s}\r\n").as_bytes()).unwrap();
                w.flush().unwrap();
            };
            if l.starts_with("PBSZ") || l.starts_with("PROT") || l == "TYPE I" {
                say("200 ok");
            } else if l.starts_with("USER") {
                say("331 password please");
            } else if l.starts_with("PASS") {
                say("230 ok");
            } else if l == "PASV" {
                let d = TcpListener::bind("127.0.0.1:0").unwrap();
                let p = d.local_addr().unwrap().port();
                data = Some(d);
                say(&format!("227 Passive (127,0,0,1,{},{}).", p / 256, p % 256));
            } else if l.starts_with("STOR") {
                say("150 go");
                let (tcp, _) = data.take().unwrap().accept().unwrap();
                let mut d = accept_tls(&config, tcp);
                let mut buf = Vec::new();
                // abgebrochen (RST) zählt nur, was bis dahin ankam
                let _ = d.read_to_end(&mut buf);
                sizes.push(buf.len());
                say("226 done");
            } else if l == "QUIT" {
                say("221 bye");
                break;
            } else {
                say("500 what");
            }
        }
        sizes
    });
    (port, handle)
}

#[test]
fn ftps_upload_arrives_complete() {
    let (port, server) = ftps_server();
    let mut ftp = Ftp::connect(&FtpConfig {
        tls: true,
        insecure: true,
        ..config(port)
    })
    .unwrap();
    // gross genug für viele TLS-Datensätze (die abgeschnittene Datei war 96 von 132 KB)
    let file: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    ftp.store("sicherung.mbusio", &file).unwrap();
    ftp.quit();
    assert_eq!(server.join().unwrap(), [file.len()]);
}
