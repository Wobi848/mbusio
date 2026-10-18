//! Export der Messwerte als CSV (Semikolon, für Excel) oder JSON.

use crate::i18n::{label, Lang};
use crate::mbus::master::MeterReading;

const COLUMNS_DE: &[&str] = &[
    "Zeitpunkt",
    "Adresse",
    "Primäradresse",
    "ID",
    "Hersteller",
    "Medium",
    "Nr",
    "Beschreibung",
    "Wert",
    "Einheit",
    "Funktion",
    "Speicher",
    "Tarif",
    "Untereinheit",
    "DIF",
    "VIF",
    "Rohdaten",
];
const COLUMNS_EN: &[&str] = &[
    "Timestamp",
    "Address",
    "Primary address",
    "ID",
    "Manufacturer",
    "Medium",
    "No",
    "Description",
    "Value",
    "Unit",
    "Function",
    "Storage",
    "Tariff",
    "Subunit",
    "DIF",
    "VIF",
    "Raw data",
];

/// Beschreibung inkl. Hinweisen, z.B. "Energie (nur positive Anteile)".
pub fn description(quantity: &str, notes: &[String], lang: Lang) -> String {
    let mut s = label(quantity, lang);
    if !notes.is_empty() {
        let n: Vec<String> = notes.iter().map(|n| label(n, lang)).collect();
        s.push_str(&format!(" ({})", n.join(", ")));
    }
    s
}

/// Ein CSV-Feld (Semikolon, Excel): Anführungszeichen, wo nötig
pub fn field(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Eine Zeile aus Feldern, mit Zeilenende für Excel
pub fn row<S: AsRef<str>>(fields: &[S]) -> String {
    let mut out = fields
        .iter()
        .map(|s| field(s.as_ref()))
        .collect::<Vec<_>>()
        .join(";");
    out.push_str("\r\n");
    out
}

pub fn csv(readings: &[MeterReading], lang: Lang) -> String {
    // BOM, damit Excel UTF-8 (Umlaute, m³) korrekt erkennt
    let mut out = String::from("\u{FEFF}");
    out.push_str(
        &if lang == Lang::En {
            COLUMNS_EN
        } else {
            COLUMNS_DE
        }
        .join(";"),
    );
    out.push_str("\r\n");
    for r in readings {
        let (id, man, medium) = match &r.header {
            Some(h) => (
                h.id.as_str(),
                h.manufacturer.as_str(),
                label(&h.medium, lang),
            ),
            None => ("", "", String::new()),
        };
        for (k, rec) in r.records.iter().enumerate() {
            let row = [
                r.timestamp.clone(),
                r.target.clone(),
                r.primary_address.to_string(),
                id.into(),
                man.into(),
                medium.clone(),
                (k + 1).to_string(),
                description(&rec.quantity, &rec.notes, lang),
                rec.value_text.clone(),
                rec.unit.clone(),
                label(&rec.function, lang),
                rec.storage.to_string(),
                rec.tariff.to_string(),
                rec.subunit.to_string(),
                rec.dif.clone(),
                rec.vif.clone(),
                rec.raw.clone(),
            ];
            out.push_str(&self::row(&row));
        }
    }
    out
}

pub fn json(readings: &[MeterReading]) -> String {
    serde_json::to_string_pretty(readings).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::field;

    #[test]
    fn quoting() {
        assert_eq!(field("a;b"), "\"a;b\"");
        assert_eq!(field("x\"y"), "\"x\"\"y\"");
        assert_eq!(field("m³"), "m³");
    }
}
