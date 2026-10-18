//! MQTT-Ausgang (Konzept Dauerbetrieb, Phase 3): gibt Datenpunkte an einen Broker weiter.
//!
//! - Ein Topic je Datenpunkt unter einem Basis-Topic, Nutzlast JSON
//!   `{"value", "unit", "time", "quality", "name", "no"}`, retained, QoS 1.
//! - `<basis>/status` = `online`, als „Last Will“ `offline`: der Empfänger erkennt, wenn mbusio weg ist.
//! - **Zwischenspeicher:** alles geht zuerst in eine Warteschlange in der Datenbank und wird erst
//!   gelöscht, wenn der Broker den Empfang bestätigt hat (PUBACK). Ist der Broker weg, wird
//!   gesammelt (höchstens 7 Tage) und danach in richtiger Reihenfolge nachgeliefert, auch über
//!   einen Neustart hinweg.
//! - Optional die Uhrzeit vom Leitsystem: Nachricht auf `<basis>/time` (Sekunden oder ms seit 1970,
//!   oder RFC 3339).
//! - Ein Broker, der nicht geht, beeinflusst das Auslesen nicht (eigener Thread).
//!
//! Eigener kleiner Client für MQTT 3.1.1 (nur was gebraucht wird), damit die Zustellung genau
//! kontrolliert ist: veröffentlichen, auf die Bestätigung warten, dann erst aus der Warteschlange.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::i18n::{self, tr, Lang};

/// Warteschlange: älter wird verworfen, mehr auch (älteste zuerst)
const QUEUE_MAX_AGE_S: i64 = 7 * 86400;
const QUEUE_MAX_ROWS: i64 = 200_000;
const KEEPALIVE: u16 = 60;

// ---------------------------------------------------------------- Einstellungen

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqttConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
    /// Zertifikat des Brokers nicht prüfen (selbst signiert, nur im geschützten Netz)
    #[serde(default)]
    pub insecure: bool,
    /// eigenes CA-Zertifikat (PEM); leer = übliche öffentliche Zertifizierungsstellen
    #[serde(default)]
    pub ca: String,
    #[serde(default)]
    pub username: String,
    /// wird nie an die Oberfläche, ins Protokoll oder in einen Export gegeben
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_base")]
    pub base: String,
    /// Uhrzeit vom Leitsystem über `<basis>/time` übernehmen
    #[serde(default)]
    pub time_sync: bool,
}

fn default_port() -> u16 {
    1883
}

fn default_base() -> String {
    "mbusio".into()
}

impl Default for MqttConfig {
    fn default() -> Self {
        MqttConfig {
            enabled: false,
            host: String::new(),
            port: 1883,
            tls: false,
            insecure: false,
            ca: String::new(),
            username: String::new(),
            password: String::new(),
            client_id: String::new(),
            base: default_base(),
            time_sync: false,
        }
    }
}

impl MqttConfig {
    pub fn check(&self) -> Result<()> {
        if self.enabled && self.host.trim().is_empty() {
            bail!(tr("Broker-Adresse fehlt", "Broker address missing"));
        }
        if self.base.trim().is_empty()
            || self.base.contains(['+', '#'])
            || self.base.starts_with('/')
        {
            bail!(tr(
                "Basis-Topic ungültig (nicht leer, ohne + und #, nicht mit / beginnen)",
                "Invalid base topic (not empty, no + or #, must not start with /)"
            ));
        }
        Ok(())
    }

    fn client_id(&self) -> String {
        if !self.client_id.trim().is_empty() {
            return self.client_id.trim().to_string();
        }
        let host = std::fs::read_to_string("/etc/hostname")
            .ok()
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .unwrap_or_default();
        format!("mbusio-{}", slug(host.trim()))
            .trim_end_matches('-')
            .to_string()
    }

    fn topic(&self, rel: &str) -> String {
        format!("{}/{rel}", self.base.trim().trim_end_matches('/'))
    }
}

/// Teil eines Topics aus einem Namen: klein, ä → ae, nur a-z 0-9 und Bindestrich
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            'é' | 'è' | 'ê' => out.push('e'),
            'à' | 'â' => out.push('a'),
            c if c.is_ascii_alphanumeric() || c == '_' => out.push(c),
            _ => {
                if !out.ends_with('-') {
                    out.push('-')
                }
            }
        }
    }
    out.trim_matches('-').to_string()
}

/// Topic (relativ zum Basis-Topic) prüfen: nicht leer, keine Platzhalter, keine leeren Teile
pub fn check_topic(rel: &str) -> Result<()> {
    if rel.is_empty()
        || rel.contains(['+', '#'])
        || rel.starts_with('/')
        || rel.ends_with('/')
        || rel.contains("//")
    {
        bail!("{}: „{rel}“", tr("Topic ungültig", "Invalid topic"));
    }
    Ok(())
}

// ---------------------------------------------------------------- Datenbank

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mqtt_queue (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    payload TEXT NOT NULL,
    created INTEGER NOT NULL
);
";

impl Db {
    pub fn mqtt_config(&self) -> MqttConfig {
        self.meta("mqtt")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    pub fn set_mqtt_config(&mut self, c: &MqttConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mqtt", &serde_json::to_string(c)?)?;
        self.mqtt_enabled = c.enabled;
        Ok(())
    }

    /// Nachricht für den Broker vormerken (Topic relativ zum Basis-Topic)
    pub(crate) fn mqtt_enqueue(&mut self, topic: &str, payload: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mqtt_queue (topic, payload, created) VALUES (?1, ?2, ?3)",
            params![topic, payload, crate::clock::utc_secs()],
        )?;
        Ok(())
    }

    pub(crate) fn mqtt_peek(&self, n: usize) -> Result<Vec<(i64, String, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, topic, payload FROM mqtt_queue ORDER BY id LIMIT ?1")?;
        let rows = st.query_map([n as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn mqtt_done(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM mqtt_queue WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn mqtt_queued(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM mqtt_queue", [], |r| r.get(0))?)
    }

    /// Warteschlange begrenzen; liefert die Anzahl verworfener Nachrichten
    pub fn mqtt_trim(&mut self) -> Result<usize> {
        let old = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE created < ?1",
            [crate::clock::utc_secs() - QUEUE_MAX_AGE_S],
        )?;
        let over = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE id <= (SELECT MAX(id) FROM mqtt_queue) - ?1",
            [QUEUE_MAX_ROWS],
        )?;
        Ok(old + over)
    }
}

// ---------------------------------------------------------------- Protokoll (MQTT 3.1.1)

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn packet(header: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![header];
    let mut len = body.len();
    loop {
        let mut b = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            b |= 0x80;
        }
        out.push(b);
        if len == 0 {
            break;
        }
    }
    out.extend_from_slice(body);
    out
}

pub fn connect_packet(client_id: &str, user: &str, pass: &str, will_topic: &str) -> Vec<u8> {
    let mut b = Vec::new();
    put_str(&mut b, "MQTT");
    b.push(4); // 3.1.1
               // clean session, Will (QoS 1, retained), Benutzer/Passwort falls angegeben
    let mut flags = 0x02 | 0x04 | 0x08 | 0x20;
    if !user.is_empty() {
        flags |= 0x80;
        if !pass.is_empty() {
            flags |= 0x40;
        }
    }
    b.push(flags);
    b.extend_from_slice(&KEEPALIVE.to_be_bytes());
    put_str(&mut b, client_id);
    put_str(&mut b, will_topic);
    put_str(&mut b, "offline");
    if !user.is_empty() {
        put_str(&mut b, user);
        if !pass.is_empty() {
            put_str(&mut b, pass);
        }
    }
    packet(0x10, &b)
}

pub fn publish_packet(topic: &str, payload: &[u8], pid: u16) -> Vec<u8> {
    let mut b = Vec::new();
    put_str(&mut b, topic);
    b.extend_from_slice(&pid.to_be_bytes());
    b.extend_from_slice(payload);
    // QoS 1, retained
    packet(0x30 | 0x02 | 0x01, &b)
}

fn subscribe_packet(topic: &str, pid: u16) -> Vec<u8> {
    let mut b = pid.to_be_bytes().to_vec();
    put_str(&mut b, topic);
    b.push(0); // QoS 0
    packet(0x82, &b)
}

/// Vollständiges Paket am Anfang des Puffers: (Kopfbyte, Inhalt, Länge insgesamt)
pub fn parse_packet(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    let header = *buf.first()?;
    let mut len = 0usize;
    let mut mult = 1usize;
    let mut i = 1;
    loop {
        let b = *buf.get(i)?;
        len += (b & 0x7F) as usize * mult;
        mult *= 128;
        i += 1;
        if b & 0x80 == 0 {
            break;
        }
        if i > 4 {
            return None;
        }
    }
    if buf.len() < i + len {
        return None;
    }
    Some((header, buf[i..i + len].to_vec(), i + len))
}

fn connack_error(rc: u8) -> String {
    match rc {
        1 => tr("Protokollversion abgelehnt", "protocol version refused"),
        2 => tr("Client-ID abgelehnt", "client ID rejected"),
        3 => tr("Broker nicht verfügbar", "broker unavailable"),
        4 => tr("Benutzer oder Passwort falsch", "bad user name or password"),
        5 => tr("nicht berechtigt", "not authorized"),
        _ => format!("{} {rc}", tr("Fehler", "error")),
    }
}

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

#[derive(Debug)]
struct NoVerify(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn tls_config(c: &MqttConfig) -> Result<rustls::ClientConfig> {
    use rustls::pki_types::pem::PemObject;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;
    if c.insecure {
        return Ok(builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify(provider)))
            .with_no_client_auth());
    }
    let mut roots = rustls::RootCertStore::empty();
    if c.ca.trim().is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    } else {
        for cert in rustls::pki_types::CertificateDer::pem_slice_iter(c.ca.as_bytes()) {
            roots.add(cert.context(tr("CA-Zertifikat unlesbar", "CA certificate unreadable"))?)?;
        }
        if roots.is_empty() {
            bail!(tr(
                "Kein Zertifikat im CA-Feld (PEM erwartet)",
                "No certificate in the CA field (PEM expected)"
            ));
        }
    }
    Ok(builder.with_root_certificates(roots).with_no_client_auth())
}

/// Eine Verbindung zum Broker
pub struct Session {
    stream: Box<dyn Stream>,
    buf: Vec<u8>,
    next_pid: u16,
    last_sent: Instant,
    /// eingehende Nachrichten (Topic, Inhalt), z.B. die Uhrzeit
    pub inbox: Vec<(String, Vec<u8>)>,
}

impl Session {
    pub fn connect(c: &MqttConfig) -> Result<Session> {
        let addr = (c.host.trim(), c.port)
            .to_socket_addrs()
            .with_context(|| format!("{}:{}", c.host, c.port))?
            .next()
            .ok_or_else(|| anyhow!("{}: {}", c.host, tr("Adresse unbekannt", "unknown address")))?;
        let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .with_context(|| format!("{}:{}", c.host, c.port))?;
        tcp.set_nodelay(true)?;
        tcp.set_read_timeout(Some(Duration::from_millis(500)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(10)))?;
        let stream: Box<dyn Stream> = if c.tls {
            let name = rustls::pki_types::ServerName::try_from(c.host.trim().to_string())
                .map_err(|e| anyhow!("{}: {e}", c.host))?;
            let conn = rustls::ClientConnection::new(Arc::new(tls_config(c)?), name)?;
            Box::new(rustls::StreamOwned::new(conn, tcp))
        } else {
            Box::new(tcp)
        };
        let mut s = Session {
            stream,
            buf: Vec::new(),
            next_pid: 0,
            last_sent: Instant::now(),
            inbox: Vec::new(),
        };
        s.send(&connect_packet(
            &c.client_id(),
            &c.username,
            &c.password,
            &c.topic("status"),
        ))?;
        let (h, body) = s
            .wait(|h, _| h >> 4 == 2, Duration::from_secs(10))?
            .ok_or_else(|| anyhow!(tr("Keine Antwort vom Broker", "No answer from broker")))?;
        let rc = body.get(1).copied().unwrap_or(0xFF);
        if h >> 4 != 2 || rc != 0 {
            bail!(
                "{}: {}",
                tr("Broker lehnt ab", "Broker refuses"),
                connack_error(rc)
            );
        }
        Ok(s)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        self.last_sent = Instant::now();
        Ok(())
    }

    fn pid(&mut self) -> u16 {
        self.next_pid = self.next_pid.wrapping_add(1).max(1);
        self.next_pid
    }

    /// Ein Paket lesen, höchstens `max` warten; eingehende Nachrichten landen im Posteingang
    fn read_packet(&mut self, max: Duration) -> Result<Option<(u8, Vec<u8>)>> {
        let until = Instant::now() + max;
        loop {
            if let Some((h, body, n)) = parse_packet(&self.buf) {
                self.buf.drain(..n);
                if h >> 4 == 3 {
                    // PUBLISH vom Broker (abonniert mit QoS 0)
                    if body.len() >= 2 {
                        let tl = u16::from_be_bytes([body[0], body[1]]) as usize;
                        let mut start = 2 + tl;
                        if (h >> 1) & 0x03 > 0 {
                            // QoS > 0: Paketnummer bestätigen
                            let pid = [body[start], body[start + 1]];
                            start += 2;
                            self.send(&packet(0x40, &pid))?;
                        }
                        if let (Ok(topic), Some(payload)) = (
                            String::from_utf8(body.get(2..2 + tl).unwrap_or_default().to_vec()),
                            body.get(start..),
                        ) {
                            self.inbox.push((topic, payload.to_vec()));
                        }
                    }
                    continue;
                }
                return Ok(Some((h, body)));
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            let mut tmp = [0u8; 4096];
            match self.stream.read(&mut tmp) {
                Ok(0) => bail!(tr(
                    "Broker hat die Verbindung getrennt",
                    "Broker closed the connection"
                )),
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Warten, bis ein passendes Paket kommt (andere werden übergangen)
    fn wait(
        &mut self,
        want: impl Fn(u8, &[u8]) -> bool,
        max: Duration,
    ) -> Result<Option<(u8, Vec<u8>)>> {
        let until = Instant::now() + max;
        while Instant::now() < until {
            if let Some((h, body)) = self.read_packet(until - Instant::now())? {
                if want(h, &body) {
                    return Ok(Some((h, body)));
                }
            }
        }
        Ok(None)
    }

    /// Veröffentlichen (QoS 1, retained) und auf die Bestätigung des Brokers warten
    pub fn publish(&mut self, topic: &str, payload: &[u8]) -> Result<()> {
        let pid = self.pid();
        self.send(&publish_packet(topic, payload, pid))?;
        let ack = self.wait(
            |h, b| h >> 4 == 4 && b.len() >= 2 && u16::from_be_bytes([b[0], b[1]]) == pid,
            Duration::from_secs(15),
        )?;
        if ack.is_none() {
            bail!(tr(
                "Broker bestätigt nicht (Zeitüberschreitung)",
                "Broker does not acknowledge (timeout)"
            ));
        }
        Ok(())
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<()> {
        let pid = self.pid();
        self.send(&subscribe_packet(topic, pid))?;
        self.wait(|h, _| h >> 4 == 9, Duration::from_secs(10))?
            .ok_or_else(|| {
                anyhow!(tr(
                    "Abonnieren nicht bestätigt",
                    "Subscription not acknowledged"
                ))
            })?;
        Ok(())
    }

    /// Kurz auf Eingehendes warten; ab und zu ein Lebenszeichen senden
    pub fn idle(&mut self, max: Duration) -> Result<()> {
        if self.last_sent.elapsed() > Duration::from_secs(KEEPALIVE as u64 / 2) {
            self.send(&packet(0xC0, &[]))?;
        }
        self.read_packet(max)?;
        Ok(())
    }

    pub fn disconnect(mut self) {
        let _ = self.send(&packet(0xE0, &[]));
    }
}

// ---------------------------------------------------------------- Dienst

#[derive(Debug, Clone, Default, Serialize)]
pub struct MqttStatus {
    /// "off", "connecting", "connected", "error"
    pub state: String,
    pub broker: String,
    pub error: Option<String>,
    pub since: Option<String>,
    /// seit dem Start gesendet
    pub sent: u64,
    pub queued: i64,
}

#[derive(Default)]
pub struct MqttState {
    pub status: Mutex<MqttStatus>,
    /// erhöht bei jeder Änderung der Einstellungen: neu verbinden
    pub version: AtomicU64,
}

/// Uhrzeit aus einer Nachricht: Sekunden oder Millisekunden seit 1970, oder RFC 3339
pub fn parse_time(payload: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(payload).ok()?.trim().trim_matches('"');
    if let Ok(n) = s.parse::<f64>() {
        return Some(if n > 1e11 {
            n as i64
        } else {
            (n * 1000.0) as i64
        });
    }
    Some(
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()?
            .timestamp_millis(),
    )
}

fn set_status(st: &crate::web::AppState, f: impl FnOnce(&mut MqttStatus)) {
    let mut s = st.mqtt.status.lock().unwrap();
    f(&mut s);
}

/// Läuft im eigenen Thread, solange mbusio läuft
pub fn run(st: crate::web::AppState) {
    i18n::set(Lang::De);
    let mut backoff = Duration::from_secs(2);
    let mut last_error = String::new();
    loop {
        let version = st.mqtt.version.load(Ordering::SeqCst);
        let cfg = st.db.lock().unwrap().mqtt_config();
        if !cfg.enabled || cfg.host.trim().is_empty() {
            set_status(&st, |s| {
                *s = MqttStatus {
                    state: "off".into(),
                    ..Default::default()
                }
            });
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let broker = format!("{}:{}", cfg.host.trim(), cfg.port);
        set_status(&st, |s| {
            s.state = "connecting".into();
            s.broker = broker.clone();
        });
        let result = Session::connect(&cfg).and_then(|s| session(&st, &cfg, s, version));
        if let Err(e) = result {
            let msg = format!("{e:#}");
            if msg != last_error {
                st.log.push("INFO", format!("MQTT {broker}: {msg}"));
                last_error = msg.clone();
            }
            set_status(&st, |s| {
                s.state = "error".into();
                s.error = Some(msg);
                s.since = None;
            });
            // warten, aber bei geänderten Einstellungen sofort neu versuchen
            let until = Instant::now() + backoff;
            while Instant::now() < until && st.mqtt.version.load(Ordering::SeqCst) == version {
                std::thread::sleep(Duration::from_millis(200));
            }
            backoff = (backoff * 2).min(Duration::from_secs(60));
        } else {
            backoff = Duration::from_secs(2);
            last_error.clear();
        }
    }
}

/// Verbunden: Warteschlange abarbeiten, Uhrzeit annehmen, bis die Einstellungen sich ändern
fn session(
    st: &crate::web::AppState,
    cfg: &MqttConfig,
    mut s: Session,
    version: u64,
) -> Result<()> {
    s.publish(&cfg.topic("status"), b"online")?;
    if cfg.time_sync {
        s.subscribe(&cfg.topic("time"))?;
    }
    st.log.push(
        "INFO",
        format!(
            "MQTT {} {}:{}",
            tr("verbunden mit", "connected to"),
            cfg.host.trim(),
            cfg.port
        ),
    );
    set_status(st, |x| {
        x.state = "connected".into();
        x.error = None;
        x.since = Some(crate::clock::stamp());
    });
    loop {
        if st.mqtt.version.load(Ordering::SeqCst) != version {
            // Einstellungen geändert: sauber abmelden (dann gilt der Last Will nicht)
            let _ = s.publish(&cfg.topic("status"), b"offline");
            s.disconnect();
            return Ok(());
        }
        let batch = st.db.lock().unwrap().mqtt_peek(50)?;
        if batch.is_empty() {
            s.idle(Duration::from_millis(500))?;
        }
        for (id, topic, payload) in batch {
            s.publish(&cfg.topic(&topic), payload.as_bytes())?;
            st.db.lock().unwrap().mqtt_done(id)?;
            set_status(st, |x| x.sent += 1);
        }
        for (topic, payload) in std::mem::take(&mut s.inbox) {
            if topic == cfg.topic("time") {
                match parse_time(&payload) {
                    Some(ms) => {
                        let was = crate::clock::state();
                        if let Some(off) = crate::clock::set_from("mqtt", ms) {
                            if was.source.as_deref() != Some("mqtt")
                                || (off - was.offset_s).abs() >= 2.0
                            {
                                st.log.push(
                                    "INFO",
                                    format!(
                                        "{} ({:+.0} s)",
                                        tr(
                                            "Uhrzeit vom Leitsystem (MQTT) übernommen",
                                            "Time taken from control system (MQTT)"
                                        ),
                                        off
                                    ),
                                );
                            }
                        }
                    }
                    None => st.log.push(
                        "INFO",
                        format!(
                            "MQTT {}: {}",
                            topic,
                            tr("Uhrzeit unlesbar", "time unreadable")
                        ),
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn slugs_and_topics() {
        assert_eq!(slug("MFH Seestrasse 12"), "mfh-seestrasse-12");
        assert_eq!(slug("Wärme · Rücklauf (Whg 3)"), "waerme-ruecklauf-whg-3");
        assert!(check_topic("a/b/c").is_ok());
        assert!(check_topic("a/+/c").is_err());
        assert!(check_topic("/a").is_err());
        assert!(check_topic("a//b").is_err());
    }

    #[test]
    fn packets() {
        let p = publish_packet("a/b", b"x", 7);
        assert_eq!(p, vec![0x33, 8, 0, 3, b'a', b'/', b'b', 0, 7, b'x']);
        // lange Nachricht: Länge über mehrere Bytes
        let long = packet(0x30, &vec![0u8; 321]);
        assert_eq!(&long[..3], &[0x30, 0xC1, 0x02]);
        assert_eq!(parse_packet(&long).unwrap().2, 324);
        assert!(parse_packet(&long[..100]).is_none());
        let c = connect_packet("id", "u", "p", "m/status");
        assert_eq!(c[0], 0x10);
        assert_eq!(c[2..8], [0, 4, b'M', b'Q', b'T', b'T']);
        assert_eq!(c[9], 0x02 | 0x04 | 0x08 | 0x20 | 0x80 | 0x40);
    }

    #[test]
    fn time_payloads() {
        assert_eq!(parse_time(b"1760000000"), Some(1_760_000_000_000));
        assert_eq!(parse_time(b"1760000000123"), Some(1_760_000_000_123));
        assert_eq!(
            parse_time(b"\"2026-10-17T12:00:00Z\""),
            Some(1_792_238_400_000)
        );
        assert_eq!(parse_time(b"morgen"), None);
    }

    /// Kleiner Test-Broker: nimmt die Verbindung an, bestätigt alles, merkt sich Veröffentlichtes
    type Received = Arc<Mutex<Vec<(String, String)>>>;

    fn fake_broker(refuse: u8) -> (u16, Received) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let got = Arc::new(Mutex::new(Vec::new()));
        let g = got.clone();
        std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let Ok(n) = c.read(&mut tmp) else { return };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
                while let Some((h, body, n)) = parse_packet(&buf) {
                    buf.drain(..n);
                    match h >> 4 {
                        1 => c.write_all(&[0x20, 2, 0, refuse]).unwrap(),
                        3 => {
                            let tl = u16::from_be_bytes([body[0], body[1]]) as usize;
                            let topic = String::from_utf8(body[2..2 + tl].to_vec()).unwrap();
                            let pid = [body[2 + tl], body[3 + tl]];
                            g.lock()
                                .unwrap()
                                .push((topic, String::from_utf8(body[4 + tl..].to_vec()).unwrap()));
                            c.write_all(&[0x40, 2, pid[0], pid[1]]).unwrap();
                        }
                        8 => {
                            c.write_all(&[0x90, 3, body[0], body[1], 0]).unwrap();
                            // gleich eine Uhrzeit schicken
                            let mut b = vec![0, 6];
                            b.extend_from_slice(b"m/time");
                            b.extend_from_slice(b"1760000000");
                            c.write_all(&packet(0x30, &b)).ok();
                        }
                        12 => c.write_all(&[0xD0, 0]).unwrap(),
                        _ => {}
                    }
                }
            }
        });
        (port, got)
    }

    fn cfg(port: u16) -> MqttConfig {
        MqttConfig {
            enabled: true,
            host: "127.0.0.1".into(),
            port,
            base: "m".into(),
            ..Default::default()
        }
    }

    #[test]
    fn session_publishes_with_ack() {
        let (port, got) = fake_broker(0);
        let mut s = Session::connect(&cfg(port)).unwrap();
        s.publish("m/a", b"1").unwrap();
        s.publish("m/b", b"2").unwrap();
        s.disconnect();
        assert_eq!(
            *got.lock().unwrap(),
            vec![
                ("m/a".to_string(), "1".to_string()),
                ("m/b".to_string(), "2".to_string())
            ]
        );
    }

    #[test]
    fn refused_connection_explains_why() {
        let (port, _) = fake_broker(4);
        let e = Session::connect(&cfg(port)).err().unwrap();
        assert!(format!("{e:#}").contains("Passwort"), "{e:#}");
    }

    #[test]
    fn unreachable_broker_fails_fast() {
        let t0 = Instant::now();
        assert!(Session::connect(&cfg(1)).is_err());
        assert!(t0.elapsed() < Duration::from_secs(6));
    }
}

#[cfg(test)]
mod debug_broker {
    use super::*;

    /// Nur von Hand mit laufendem Test-Broker: MBUSIO_TEST_BROKER=127.0.0.1:18830
    #[test]
    #[ignore]
    fn receives_subscribed_message() {
        let Ok(addr) = std::env::var("MBUSIO_TEST_BROKER") else {
            return;
        };
        let (host, port) = addr.split_once(':').unwrap();
        let cfg = MqttConfig {
            enabled: true,
            host: host.into(),
            port: port.parse().unwrap(),
            base: "dbg".into(),
            ..Default::default()
        };
        let mut s = Session::connect(&cfg).unwrap();
        s.subscribe("dbg/time").unwrap();
        std::process::Command::new("mosquitto_pub")
            .args(["-h", host, "-p", port, "-t", "dbg/time", "-m", "1760000000"])
            .status()
            .unwrap();
        for _ in 0..10 {
            s.idle(Duration::from_millis(300)).unwrap();
            if !s.inbox.is_empty() {
                break;
            }
        }
        println!(
            "inbox: {:?}",
            s.inbox
                .iter()
                .map(|(t, p)| (t.clone(), String::from_utf8_lossy(p).to_string()))
                .collect::<Vec<_>>()
        );
        assert!(!s.inbox.is_empty());
    }
}
