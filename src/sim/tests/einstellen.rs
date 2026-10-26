//! Tests des Simulators: Adresse, Baudrate, Reset

use super::*;

#[test]
fn set_address_via_secondary() {
    let mut m = master();
    m.set_primary(&Target::parse("12345699").unwrap(), 20)
        .unwrap();
    let r = m.read_meter(&Target::Primary(20)).unwrap();
    assert_eq!(r.header.unwrap().id, "12345699");
    // Adresse 0 ist jetzt eindeutig
    let r = m.read_meter(&Target::Primary(0)).unwrap();
    assert_eq!(r.header.unwrap().id, "55500011");
}

#[test]
fn set_address_refuses_collision() {
    let mut m = master();
    let e = m
        .set_primary(&Target::Primary(0), 9)
        .unwrap_err()
        .to_string();
    assert!(e.contains("Mehrere Zähler"), "{e}");
    // keiner der beiden wurde umgestellt
    assert!(m.read_meter(&Target::Primary(9)).is_err());
    let r = m.read_meter(&Target::parse("55500011").unwrap()).unwrap();
    assert_eq!(r.primary_address, 0);
}

#[test]
fn set_address_refuses_occupied() {
    let mut m = master();
    let e = m
        .set_primary(&Target::Primary(1), 2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("schon belegt"), "{e}");
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    assert_eq!(r.header.unwrap().id, "12345678");
    let r = m.read_meter(&Target::Primary(2)).unwrap();
    assert_eq!(r.header.unwrap().id, "87654321");
}

#[test]
fn set_address_to_own_address_is_ok() {
    let mut m = master();
    m.set_primary(&Target::Primary(1), 1).unwrap();
    m.set_primary(&Target::parse("87654321").unwrap(), 2)
        .unwrap();
}

#[test]
fn detect_and_switch_baud() {
    let mut m = master();
    // Adresse 6 läuft mit 2400 Bd, der Master mit 9600
    assert!(m.read_meter(&Target::Primary(6)).is_err());
    assert_eq!(m.detect_baud(&Target::Primary(6)).unwrap(), vec![2400]);
    assert_eq!(m.baud(), 9600);
    m.set_baud(2400).unwrap();
    m.switch_baud(&Target::parse("60000006").unwrap(), 9600)
        .unwrap();
    m.set_baud(9600).unwrap();
    let r = m.read_meter(&Target::Primary(6)).unwrap();
    assert_eq!(r.header.unwrap().id, "60000006");
    // Zähler 1 läuft weiter mit 9600
    assert_eq!(m.detect_baud(&Target::Primary(1)).unwrap(), vec![9600]);
}

#[test]
fn switch_baud_refuses_collision() {
    let mut m = master();
    assert!(m.switch_baud(&Target::Primary(0), 2400).is_err());
    assert_eq!(m.detect_baud(&Target::Primary(0)).unwrap(), vec![9600]);
}

#[test]
fn application_reset_is_acknowledged() {
    let mut m = master();
    m.application_reset(&Target::Primary(1), None).unwrap();
    m.application_reset(&Target::parse("87654321").unwrap(), Some(0x10))
        .unwrap();
    assert!(m.application_reset(&Target::Primary(99), None).is_err());
}

#[test]
fn free_primary_skips_meters_without_data() {
    let mut m = master();
    // 1..5 und 7 belegt (4 und 5 liefern keine Daten, antworten aber), 6 läuft mit anderer Baudrate
    assert_eq!(m.free_primary(1, &[]).unwrap(), 6);
    assert_eq!(m.free_primary(1, &[6]).unwrap(), 8);
}

#[test]
fn switch_meter_on_other_baud_to_bus_baud() {
    let mut m = master();
    // Zähler 6 läuft mit 2400, Bus mit 9600: Befehl mit 2400 senden, Kontrolle mit 9600
    m.set_baud(2400).unwrap();
    m.switch_baud(&Target::Primary(6), 9600).unwrap();
    m.set_baud(9600).unwrap();
    assert_eq!(m.bus_bauds(&[]).unwrap(), vec![300, 9600]);
}
