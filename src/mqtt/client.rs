//! Eigener kleiner Client für MQTT 3.1.1: Pakete, TLS, Sitzung mit Bestätigung (PUBACK)

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

use super::{MqttConfig, KEEPALIVE};
use crate::i18n::tr;

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

#[cfg(test)]
mod tests;
