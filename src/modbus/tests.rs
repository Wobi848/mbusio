use super::image::{block, build};
use super::protocol::{answer, next_request, Next};
use super::*;
use serde_json::json;

/// Anfrage Modbus TCP: Transaktion 7, Unit 1, Funktion, Start, Anzahl
fn request(function: u8, start: u16, count: u16) -> Vec<u8> {
    let mut r = vec![0, 7, 0, 0, 0, 6, 1, function];
    r.extend_from_slice(&start.to_be_bytes());
    r.extend_from_slice(&count.to_be_bytes());
    r
}

/// Register aus einer Antwort auf 03/04
fn registers(resp: &[u8]) -> Vec<u16> {
    resp[9..]
        .chunks(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect()
}

fn f64_of(w: &[u16]) -> f64 {
    let b: Vec<u8> = w.iter().flat_map(|x| x.to_be_bytes()).collect();
    f64::from_be_bytes(b.try_into().unwrap())
}

#[test]
fn block_holds_value_in_three_types_quality_and_age() {
    let b = block(123456.78, 1, 15, false);
    assert_eq!(f64_of(&b[0..4]), 123456.78);
    let f32b: Vec<u8> = b[4..6].iter().flat_map(|x| x.to_be_bytes()).collect();
    assert_eq!(f32::from_be_bytes(f32b.try_into().unwrap()), 123456.78f32);
    let i32b: Vec<u8> = b[6..8].iter().flat_map(|x| x.to_be_bytes()).collect();
    assert_eq!(i32::from_be_bytes(i32b.try_into().unwrap()), 12345678);
    assert_eq!((b[8], b[9]), (1, 15));
    // Worte getauscht: niederwertiges zuerst
    let s = block(123456.78, 0, 0, true);
    let mut w = s[0..4].to_vec();
    w.reverse();
    assert_eq!(f64_of(&w), 123456.78);
    assert_eq!(s[6..8], [b[7], b[6]]);
    // int32 begrenzt statt übergelaufen
    let big = block(1e12, 0, 0, false);
    assert_eq!(big[6..8], [0x7FFF, 0xFFFF]);
}

#[test]
fn image_from_datapoints() {
    let dp = |no: i64, enabled: bool, operation: bool, quality: &str| {
        json!({
            "no": no, "active": true, "operation": operation, "quality": quality,
            "last_value": no as f64 * 10.0, "last_utc": 1_000_000,
            "outputs": { "modbus": { "enabled": enabled, "address": block_start(no).unwrap().to_string() } }
        })
    };
    let rows = [
        dp(1, true, true, "good"),
        dp(2, true, true, "disturbed"),
        dp(3, false, true, "good"), // Häkchen weg: Lücke
        dp(4, true, false, "good"), // nicht im Dauerbetrieb: kein Wert
        dp(5, true, true, "stale"),
    ];
    let regs = build(&rows, false, 1_000_000 + 125, 42);
    assert_eq!(f64_of(&regs[0..4]), 10.0);
    assert_eq!((regs[8], regs[9]), (0, 2), "gut, 2 Minuten alt");
    assert_eq!(regs[18], 2, "gestört");
    assert!(regs[20..30].iter().all(|r| *r == 0), "Lücke");
    assert_eq!(regs[38], 4, "kein Wert ohne Dauerbetrieb");
    assert_eq!(f64_of(&regs[30..34]), 0.0);
    assert_eq!(regs[48], 1, "veraltet");
    assert_eq!(
        &regs[60000..60003],
        &[42, 1, 1],
        "Lebenszeichen, gestört, veraltet"
    );
}

#[test]
fn requests_and_error_answers() {
    let mut regs = vec![0u16; 65536];
    regs[10] = 0x1234;
    regs[11] = 0xABCD;
    // unvollständig, dann vollständig; zwei Anfragen hintereinander
    let r = request(0x04, 10, 2);
    assert_eq!(next_request(&r[..5]), Next::Need);
    let mut two = r.clone();
    two.extend(request(0x03, 10, 1));
    assert_eq!(next_request(&two), Next::Request(12));
    let resp = answer(&r, &regs);
    assert_eq!(&resp[0..2], &[0, 7], "gleiche Transaktion");
    assert_eq!(
        &resp[4..9],
        &[0, 7, 1, 0x04, 4],
        "Länge, Unit, Funktion, Bytes"
    );
    assert_eq!(registers(&resp), vec![0x1234, 0xABCD]);
    // 03 liefert dieselben Register
    assert_eq!(
        registers(&answer(&request(0x03, 10, 2), &regs)),
        vec![0x1234, 0xABCD]
    );
    // Schreiben: Funktion unbekannt; zu viele Register; über das Ende
    assert_eq!(&answer(&request(0x06, 10, 1), &regs)[7..], &[0x86, 0x01]);
    assert_eq!(&answer(&request(0x04, 0, 126), &regs)[7..], &[0x84, 0x03]);
    assert_eq!(&answer(&request(0x04, 65535, 2), &regs)[7..], &[0x84, 0x02]);
    // kein Modbus TCP
    assert_eq!(next_request(&[0, 1, 0, 9, 0, 6, 1, 4]), Next::Garbage);
}

#[test]
fn addresses_follow_the_number_and_are_unique_per_project() {
    let o = crate::datapoint::output("modbus").unwrap();
    assert!(o.check_address("60").is_ok());
    assert!(o.check_address("65").is_err(), "kein Vielfaches von 10");
    assert!(o.check_address("60000").is_err(), "Zustandsbereich");
    assert!(o.check_address("abc").is_err());
    assert_eq!(block_start(1), Some(0));
    assert_eq!(block_start(7), Some(60));
    assert_eq!(block_start(6000), Some(59990), "letzter Block unter 60000");
    assert_eq!(block_start(6001), None, "passt nicht mehr");
}

/// Wie `testutil::db_with_energy`, die Energie zusätzlich mit Modbus
fn db_with_energy() -> Db {
    let (mut db, _, mid, _) = crate::testutil::db_with_energy();
    let id = db.dp_by_meter(mid).unwrap()[0].id;
    db.dp_bulk(&[id], "out:modbus:on").unwrap();
    db
}

#[test]
fn datapoint_gets_register_from_its_number() {
    let db = db_with_energy();
    let d = &db.dp_list(None, Some(1)).unwrap()[0];
    assert_eq!(d["outputs"]["modbus"]["address"], "0");
    let regs = build(
        &db.dp_list(None, Some(1)).unwrap(),
        false,
        crate::clock::utc_secs(),
        0,
    );
    assert_eq!(f64_of(&regs[0..4]), 123456.0);
    assert_eq!(regs[8], 0, "gut");
}

/// Anfrage schicken, Antwort lesen
async fn ask(s: &mut TcpStream, req: Vec<u8>) -> Vec<u8> {
    s.write_all(&req).await.unwrap();
    let mut buf = vec![0u8; 512];
    let n = s.read(&mut buf).await.unwrap_or(0);
    buf.truncate(n);
    buf
}

#[tokio::test]
async fn server_answers_over_tcp_and_follows_settings() {
    let mut db = db_with_energy();
    let port = 15000 + (std::process::id() % 1000) as u16;
    db.set_modbus_config(&ModbusConfig {
        enabled: true,
        bind: "127.0.0.1".into(),
        port,
        ..Default::default()
    })
    .unwrap();
    let st: crate::web::AppState = std::sync::Arc::new(crate::web::Shared::new(None, db));
    tokio::spawn(run(st.clone()));
    let mut s = None;
    for _ in 0..50 {
        if let Ok(c) = TcpStream::connect(("127.0.0.1", port)).await {
            s = Some(c);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut s = s.expect("Server hört");
    let resp = ask(&mut s, request(0x04, 0, 10)).await;
    assert_eq!(f64_of(&registers(&resp)[0..4]), 123456.0);
    assert_eq!(registers(&resp)[8], 0);
    let status = st.modbus.status.lock().unwrap().clone();
    assert_eq!(status.state, "listening");
    assert_eq!(status.clients, 1);
    // Wortreihenfolge umgestellt: Server startet neu, liefert getauscht
    {
        let mut db = st.db.lock().unwrap();
        let mut c = db.modbus_config();
        c.swap_words = true;
        db.set_modbus_config(&c).unwrap();
    }
    st.modbus.version.fetch_add(1, Ordering::SeqCst);
    let mut s2 = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(mut c) = TcpStream::connect(("127.0.0.1", port)).await {
            let r = ask(&mut c, request(0x04, 0, 4)).await;
            if !r.is_empty() {
                s2 = Some(r);
                break;
            }
        }
    }
    let mut w = registers(&s2.expect("wieder erreichbar"));
    w.reverse();
    assert_eq!(f64_of(&w), 123456.0);
}

#[test]
fn register_list_for_the_integrator() {
    let mut db = db_with_energy();
    let mut c = db.modbus_config();
    c.swap_words = true;
    c.port = 1502;
    db.set_modbus_config(&c).unwrap();
    let rows = db.output_list(1, "modbus", crate::i18n::Lang::De).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["address"], "0", "Startregister aus der Nummer");
    let info = crate::datapoint::output("modbus")
        .unwrap()
        .list_info(&db)
        .unwrap();
    assert_eq!(info["port"], 1502);
    assert_eq!(info["word_order"], "CDAB");
    assert_eq!(info["block"]["registers"], 10);
    assert!(info["block"]["+8"].as_str().unwrap().contains("Qualität"));
    assert!(info["status"]["60001"].is_string());
    // Übersicht für den Reiter Weitergabe: Zähler mit Datenpunkten, Ausgänge mit Zustand
    let overview = db.dp_overview(None, Some(1)).unwrap();
    assert_eq!(
        overview[0]["datapoints"][0]["outputs"]["modbus"]["enabled"],
        true
    );
    let outputs = db.outputs_info();
    let keys: Vec<&str> = outputs.iter().map(|o| o["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["mqtt", "modbus"]);
    assert_eq!(outputs[1]["address_label"], "Register");
    assert_eq!(outputs[1]["on"], false, "Modbus als Ganzes noch aus");
}
