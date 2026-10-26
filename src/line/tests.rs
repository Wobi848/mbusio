//! Tests des Linien-Dienstes (Aufträge, Verbindung, USB abgezogen)

use super::*;

fn start(id: i64, connection: &str, ports: &Arc<Ports>) -> LineService {
    LineService::start(
        id,
        format!("L{id}"),
        LineConf {
            connection: connection.into(),
            baud: 9600,
            timeout_ms: 50,
        },
        Arc::new(FrameLog::default()),
        ports.clone(),
    )
}

fn sim() -> LineService {
    start(1, "SIMULATOR", &Arc::default())
}

#[test]
fn missing_device_path_means_unplugged() {
    assert!(device_gone("/dev/ttyUSB-does-not-exist"));
    assert!(!device_gone("/dev/null"));
    assert!(!device_gone("COM3"), "Windows: nicht prüfbar");
    assert!(!device_gone("tcp://10.0.0.1:10001"));
    assert!(!device_gone("SIMULATOR"));
    // Meldung beim Öffnen sagt, was wahrscheinlich los ist
    let e = link::open("/dev/ttyUSB-does-not-exist", 2400)
        .err()
        .unwrap();
    assert!(format!("{e:#}").contains("abgezogen"), "{e:#}");
}

#[test]
fn one_port_one_line_also_for_planned_reads() {
    let gw = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("tcp://{}", gw.local_addr().unwrap());
    let ports = Arc::default();
    let a = start(1, &addr, &ports);
    a.connect().unwrap();
    // zweite Linie, gleiche Adresse anders geschrieben: abgelehnt, mit Name der Linie
    let b = start(2, &addr.to_uppercase(), &ports);
    let e = b.connect().unwrap_err().to_string();
    assert!(e.contains("L1"), "{e}");
    // geplante Auslesung: dieselbe Meldung als Grund für „gestört“
    let (tx, rx) = mpsc::channel();
    b.submit_planned(Box::new(move |m| {
        tx.send(m.err()).unwrap();
        true
    }));
    assert!(rx.recv().unwrap().unwrap().contains("L1"));
    assert!(b.status().error.unwrap().contains("L1"));
    // Simulationen teilen nichts
    start(3, "SIMULATOR", &ports).connect().unwrap();
    start(4, "SIMULATOR", &ports).connect().unwrap();
    // erste Linie getrennt: Anschluss wieder frei
    assert!(a.stop_wait(Duration::from_secs(5)));
    start(5, &addr, &ports).connect().unwrap();
}

#[test]
fn late_close_keeps_newer_claim_of_same_line() {
    let ports: Arc<Ports> = Arc::default();
    let old = ports.claim("tcp://10.0.0.9:10001", 1, "L1").unwrap();
    // neu verbunden, während die alte Verbindung noch nicht zu ist
    let _new = ports.claim("tcp://10.0.0.9:10001", 1, "L1").unwrap();
    drop(old);
    let e = ports.claim("10.0.0.9:10001", 2, "L2").err().unwrap();
    assert!(e.to_string().contains("L1"), "{e}");
}

#[test]
fn runs_jobs_in_order_with_manual_first() {
    let s = sim();
    // ersten Auftrag blockieren, bis die anderen in der Schlange stehen
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let order = Arc::new(Mutex::new(Vec::new()));
    s.submit(Box::new(move |_| {
        go_rx.recv().unwrap();
        true
    }));
    for (planned, tag) in [(true, "p1"), (true, "p2"), (false, "m1")] {
        let o = order.clone();
        let w: Work = Box::new(move |_| {
            o.lock().unwrap().push(tag);
            true
        });
        if planned {
            s.submit_planned(w);
        } else {
            s.submit(w);
        }
    }
    // ein geplanter Auftrag ganz am Schluss meldet, wenn alles durch ist
    let (done_tx, done_rx) = mpsc::channel::<()>();
    s.submit_planned(Box::new(move |_| {
        done_tx.send(()).unwrap();
        true
    }));
    go_tx.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(*order.lock().unwrap(), vec!["m1", "p1", "p2"]);
    s.stop();
}

#[test]
fn reads_and_reports_status() {
    let s = sim();
    let r = s
        .run(|m| m.read_meter(&crate::mbus::master::Target::Primary(1)))
        .unwrap();
    assert_eq!(r.header.unwrap().id, "12345678");
    // "arbeitet" geht erst kurz nach dem Ergebnis aus
    let until = Instant::now() + Duration::from_secs(5);
    while s.status().busy && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let st = s.status();
    assert!(st.connected && !st.busy);
    s.stop();
}

#[test]
fn unreachable_port_reports_error_and_retries_later() {
    let s = start(2, "/dev/does-not-exist", &Arc::default());
    assert!(s.connect().is_err());
    let st = s.status();
    assert!(!st.connected && st.error.is_some());
    // Bedienung versucht es sofort wieder, nicht erst nach der Wartezeit
    assert!(s.connect().is_err());
    s.stop();
}
