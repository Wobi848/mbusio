//! Tests des Decoders (Referenztelegramme, feste und variable Struktur)

use super::values::date_g;
use super::*;
use crate::mbus::frame::{parse, Frame, Parse};

fn long(hex: &str) -> Telegram {
    let bytes: Vec<u8> = hex
        .split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect();
    let Parse::Complete(Frame::Long { ci, data, .. }, _) = parse(&bytes) else {
        panic!("Frame ungültig");
    };
    decode_rsp_ud(ci, &data).unwrap()
}

/// Echter Itron-Wärme-/Kältezähler (20.10.2026, auf dem Tisch ohne Wasser): Wärme- und
/// Kälteenergie getrennt, Status „vorübergehender Fehler“, Leistung und Durchfluss als Fehlerwert
#[test]
fn itron_heat_cooling_meter() {
    let t = long(
        "68 64 64 68 08 03 72 66 50 39 20 92 26 18 0D 90 10 00 00 0C 78 66 50 39 20 04 06 00 00 00 00 \
         0C 14 00 00 00 00 3B 2D 99 99 99 3B 3B 99 99 99 0A 5A 24 02 0A 5E 22 02 0B 61 17 00 00 \
         04 6D 3B 14 54 3A 02 27 F1 08 09 FD 0E 07 09 FD 0F 12 8C C0 00 14 18 00 00 00 \
         8C 80 40 14 00 00 00 00 04 86 3C 00 00 00 00 0F 20 00 52 16",
    );
    let h = t.header.unwrap();
    assert_eq!(
        (h.id.as_str(), h.manufacturer.as_str(), h.status),
        ("20395066", "ITR", 0x10)
    );
    let q: Vec<(&str, &str)> = t
        .records
        .iter()
        .map(|r| (r.function.as_str(), r.quantity.as_str()))
        .collect();
    assert!(q.contains(&("instantaneous", "energy")));
    assert!(q.contains(&("instantaneous", "energy_cooling")), "{q:?}");
    assert!(q.contains(&("error_value", "power")));
    assert!(q.contains(&("error_value", "volume_flow")));
    let vorlauf = t
        .records
        .iter()
        .find(|r| r.quantity == "flow_temperature")
        .unwrap();
    assert_eq!(vorlauf.value_text, "22.4");
}

/// Beispiel aus der M-Bus-Dokumentation (m-bus.com, Kapitel 6.8)
#[test]
fn decode_reference_telegram() {
    let bytes = [
        0x68, 0x1F, 0x1F, 0x68, 0x08, 0x02, 0x72, 0x78, 0x56, 0x34, 0x12, 0x24, 0x40, 0x01, 0x07,
        0x55, 0x00, 0x00, 0x00, 0x03, 0x13, 0x15, 0x31, 0x00, 0xDA, 0x02, 0x3B, 0x13, 0x01, 0x8B,
        0x60, 0x04, 0x37, 0x18, 0x02, 0x18, 0x16,
    ];
    let Parse::Complete(
        Frame::Long {
            ci, data, address, ..
        },
        _,
    ) = parse(&bytes)
    else {
        panic!("Frame ungültig");
    };
    assert_eq!(address, 2);
    let t = decode_rsp_ud(ci, &data).unwrap();
    let h = t.header.unwrap();
    assert_eq!(h.id, "12345678");
    assert_eq!(h.manufacturer, "PAD");
    assert_eq!(h.medium, "medium_water");
    assert_eq!(h.secondary_address, "1234567840240107");
    assert_eq!(t.records.len(), 3);

    let r = &t.records[0];
    assert_eq!(
        (r.quantity.as_str(), r.unit.as_str(), r.value_text.as_str()),
        ("volume", "m³", "12.565")
    );

    let r = &t.records[1];
    assert_eq!(r.function, "maximum");
    assert_eq!(r.storage, 5);
    assert_eq!((r.unit.as_str(), r.value_text.as_str()), ("m³/h", "0.113"));

    let r = &t.records[2];
    assert_eq!((r.tariff, r.subunit), (2, 1));
    assert_eq!((r.unit.as_str(), r.value_text.as_str()), ("kWh", "218.37"));
}

#[test]
fn scaling() {
    assert_eq!(scale(12345, -2).1, "123.45");
    assert_eq!(scale(-5, -3).1, "-0.005");
    assert_eq!(scale(7, 2).1, "700");
}

#[test]
fn bcd_negative() {
    assert_eq!(bcd(&[0x34, 0x12]), Some(1234));
    assert_eq!(bcd(&[0x34, 0xF2]), Some(-234));
    assert_eq!(bcd(&[0xAB]), None);
}

#[test]
fn dates() {
    // 2026-09-30: y=26 -> low 3 bits 2, high 4 bits 3
    assert_eq!(date_g(&[30 | (2 << 5), 9 | (3 << 4)]), "2026-09-30");
}

#[test]
fn fixed_binary_and_historic() {
    // Status 0xC0: binär, Stichtagswerte; Medium Wasser (7); Einheiten 100 l (2B), 3E
    let d = [
        0x01,
        0x00,
        0x00,
        0x10,
        0x05,
        0xC0,
        0xC0 | 0x2B,
        0x40 | 0x3E,
        0x10,
        0x00,
        0x00,
        0x00,
        0x20,
        0x00,
        0x00,
        0x00,
    ];
    let t = decode_rsp_ud(0x73, &d).unwrap();
    let h = t.header.unwrap();
    assert_eq!((h.id.as_str(), h.medium_code), ("10000001", 7));
    assert_eq!(
        (t.records[0].value_text.as_str(), t.records[0].unit.as_str()),
        ("1600", "l")
    );
    assert_eq!(t.records[0].storage, 1);
    // Zähler 2: gleiche Einheit wie Zähler 1
    assert_eq!(
        (t.records[1].value_text.as_str(), t.records[1].unit.as_str()),
        ("3200", "l")
    );
}

#[test]
fn application_error_is_reported() {
    let e = decode_rsp_ud(0x70, &[0x08]).unwrap_err();
    assert!(e.contains("08"), "{e}");
}
