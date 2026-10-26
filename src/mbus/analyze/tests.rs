//! Tests der Telegramm-Analyse

use super::*;

fn bytes(h: &str) -> Vec<u8> {
    h.split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).unwrap())
        .collect()
}

#[test]
fn explains_good_reply() {
    let a = analyze(&bytes(
        "68 1F 1F 68 08 02 72 78 56 34 12 24 40 01 07 55 00 00 00 03 13 15 31 00 DA 02 3B 13 01 8B 60 04 37 18 02 18 16",
    ));
    assert!(a.problems.is_empty(), "{:?}", a.problems);
    assert!(a
        .fields
        .iter()
        .any(|f| f.name == "ID" && f.value == "12345678"));
    assert!(a.fields.iter().any(|f| f.value.contains("PAD")));
}

#[test]
fn finds_checksum_truncation_and_extra_bytes() {
    let a = analyze(&bytes("10 7B 01 00 16"));
    assert!(
        a.fields.iter().any(|f| f.bad && f.value.contains("7C")),
        "{:?}",
        a.fields
    );
    let a = analyze(&bytes("68 1F 1F 68 08 02 72 78 56 34 12"));
    assert!(
        a.problems
            .iter()
            .any(|p| p.contains("abgeschnitten") || p.contains("cut off")),
        "{:?}",
        a.problems
    );
    let a = analyze(&bytes("E5 E5"));
    assert!(a.fields.iter().any(|f| f.bad));
    let a = analyze(&bytes("68 04 04 68 08 0B 70 08 8B 16"));
    assert!(a.fields.iter().any(|f| f.bad && f.value.contains("08")));
}
