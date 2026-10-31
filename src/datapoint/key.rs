//! Kennung eines Werts (Funktion|Speicher|Tarif|Untereinheit|Grösse|Einheit): zerlegen,
//! lesbare Bezeichnung, Einheit, ob es ein Zählerstand ist.

use super::*;

/// Zählerstände: dürfen nicht sinken, werden beim Zählertausch fortgeführt
pub(super) const COUNTERS: [&str; 5] = ["energy", "energy_cooling", "volume", "mass", "hca_units"];

/// Kennung eines Werts: Funktion|Speicher|Tarif|Untereinheit|Grösse|Einheit
pub fn key_of(r: &Record) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        r.function, r.storage, r.tariff, r.subunit, r.quantity, r.unit
    )
}

pub(super) struct KeyParts<'a> {
    pub(super) function: &'a str,
    pub(super) storage: &'a str,
    pub(super) tariff: &'a str,
    pub(super) subunit: &'a str,
    pub(super) quantity: &'a str,
    pub(super) unit: &'a str,
}

pub(super) fn parts(key: &str) -> Option<KeyParts<'_>> {
    let mut it = key.splitn(6, '|');
    Some(KeyParts {
        function: it.next()?,
        storage: it.next()?,
        tariff: it.next()?,
        subunit: it.next()?,
        quantity: it.next()?,
        unit: it.next()?,
    })
}

/// Zählerstand (aktuell, Speicher 0)? Nur dort gilt „darf nicht sinken“.
pub fn is_counter(key: &str) -> bool {
    parts(key).is_some_and(|p| {
        p.function == "instantaneous" && p.storage == "0" && COUNTERS.contains(&p.quantity)
    })
}

/// Lesbare Bezeichnung des Werts, z.B. „Energie · Tarif 1 · Speicher 2“
pub fn key_label(key: &str, lang: Lang) -> String {
    let Some(p) = parts(key) else {
        return key.to_string();
    };
    let mut s = i18n::label(p.quantity, lang);
    if p.function != "instantaneous" {
        s += &format!(" · {}", i18n::label(p.function, lang));
    }
    if p.tariff != "0" {
        s += &format!(" · {} {}", i18n::tr_in(lang, "Tarif", "tariff"), p.tariff);
    }
    if p.storage != "0" {
        s += &format!(
            " · {} {}",
            i18n::tr_in(lang, "Speicher", "storage"),
            p.storage
        );
    }
    if p.subunit != "0" {
        s += &format!(
            " · {} {}",
            i18n::tr_in(lang, "Untereinheit", "subunit"),
            p.subunit
        );
    }
    s
}

pub fn key_unit(key: &str) -> String {
    parts(key).map(|p| p.unit.to_string()).unwrap_or_default()
}
