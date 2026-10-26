//! Tests des Simulators: Auslesen, Telegramme, Rohdaten

use super::*;

#[test]
fn read_multi_telegram() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    assert_eq!(r.raw_frames.len(), 2);
    assert!(r.records.iter().any(|x| x.storage == 2 && x.unit == "kWh"));
    assert!(r
        .records
        .iter()
        .any(|x| x.quantity == "flow_temperature" && x.value_text == "65.23"));
}

#[test]
fn raw_telegrams_round_trip() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    let back = MeterReading::from_raw(
        &r.raw_bytes(),
        r.timestamp.clone(),
        r.utc,
        r.target.clone(),
        false,
    )
    .unwrap();
    assert_eq!(back.raw_frames, r.raw_frames);
    assert_eq!(back.primary_address, r.primary_address);
    assert_eq!(
        serde_json::to_value(&back.records).unwrap(),
        serde_json::to_value(&r.records).unwrap()
    );
    assert_eq!(
        back.header.unwrap().secondary_address,
        r.header.unwrap().secondary_address
    );
}

#[test]
fn read_electricity_values() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(2)).unwrap();
    let find = |d: &str| {
        r.records
            .iter()
            .find(|x| x.quantity == d)
            .unwrap()
            .value_text
            .clone()
    };
    assert_eq!(find("voltage"), "230.1");
    assert_eq!(find("current"), "5.123");
    assert_eq!(find("firmware_version"), "1.02");
    assert!(r.records.iter().any(|x| x.tariff == 2));
}

#[test]
fn corrupt_telegrams_are_rejected() {
    let mut m = master();
    let e = m.read_meter(&Target::Primary(4)).unwrap_err().to_string();
    assert!(e.contains("Kollision"), "{e}");
    assert!(m.read_meter(&Target::Primary(5)).is_err());
}

#[test]
fn read_fixed_structure() {
    let mut m = master();
    let r = m.read_meter(&Target::Primary(7)).unwrap();
    let h = r.header.unwrap();
    assert_eq!(
        (h.id.as_str(), h.medium.as_str()),
        ("70000007", "medium_heat_outlet")
    );
    assert_eq!(r.records.len(), 2);
    assert_eq!(
        (r.records[0].quantity.as_str(), r.records[0].unit.as_str()),
        ("energy", "kWh")
    );
    assert!(r.records[0].value_text.starts_with("123"));
    assert_eq!(
        (r.records[1].unit.as_str(), r.records[1].value_text.as_str()),
        ("m³", "678")
    );
}

#[test]
fn raw_send_returns_reply() {
    let mut m = master();
    let ack = m.raw_send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
    assert!(matches!(
        ack,
        crate::mbus::master::Reply::Frame(crate::mbus::frame::Frame::Ack)
    ));
    let none = m.raw_send(&[0x10, 0x40, 0x63, 0xA3, 0x16]).unwrap();
    assert!(matches!(none, crate::mbus::master::Reply::None));
}

#[test]
fn counters_keep_rising_after_a_restart() {
    // Energie des Wärmezählers an Adresse 1, gelesen von einem frischen Simulator (= Neustart)
    let energy = || {
        let mut m = Master::new(
            Box::new(Simulator::new(9600).with_counter_origin()),
            9600,
            50,
            Arc::new(FrameLog::default()),
        );
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        r.records
            .iter()
            .find(|x| x.quantity == "energy" && x.storage == 0)
            .and_then(|x| x.value.as_f64())
            .unwrap()
    };
    let before = energy();
    // ab einem festen Datum: deutlich über dem Anfangswert, und nach dem „Neustart“ nicht kleiner
    assert!(before > 123_456.0, "{before}");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(energy() >= before);
}
