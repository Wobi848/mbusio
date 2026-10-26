//! Tests des MQTT-Clients (Pakete, Sitzung mit Testbroker)

use super::*;
use std::net::TcpListener;
use std::sync::Mutex;

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

/// Von Hand gegen einen echten Broker (ignoriert)
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
