//! Mehrsprachigkeit (Deutsch, Englisch).
//!
//! Messdaten tragen sprachneutrale Schlüssel (`energy`, `medium_water` …), übersetzt
//! wird erst bei der Anzeige oder beim Export. Meldungen aus dem Backend verwenden
//! die Sprache des aktuellen Requests (Task) bzw. Threads.

use std::cell::Cell;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    De,
    En,
}

impl Lang {
    /// "en", "en-US", "de-CH" … ; alles Unbekannte wird Deutsch.
    pub fn parse(s: &str) -> Self {
        if s.trim().to_ascii_lowercase().starts_with("en") {
            Lang::En
        } else {
            Lang::De
        }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::De) };
}

tokio::task_local! {
    static TASK: Lang;
}

/// Sprache des Threads setzen (Bus-Aufträge, Hintergrund-Threads)
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(lang));
}

/// `f` mit dieser Sprache ausführen, auch über `await` hinweg (ein Request)
pub async fn scope<F: Future>(lang: Lang, f: F) -> F::Output {
    TASK.scope(lang, f).await
}

/// Sprache des Requests, sonst des Threads
pub fn current() -> Lang {
    TASK.try_with(|l| *l)
        .unwrap_or_else(|_| CURRENT.with(|c| c.get()))
}

/// Meldung in der Sprache des aktuellen Threads.
pub fn tr(de: &str, en: &str) -> String {
    match current() {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
    }
}

/// Alle Schlüssel für Messdaten: (Schlüssel, Deutsch, Englisch).
const LABELS: &[(&str, &str, &str)] = &[
    // Grössen
    ("energy", "Energie", "Energy"),
    ("energy_cooling", "Kälteenergie", "Cooling energy"),
    ("volume", "Volumen", "Volume"),
    ("mass", "Masse", "Mass"),
    ("on_time", "Einschaltdauer", "On time"),
    ("operating_time", "Betriebsdauer", "Operating time"),
    ("power", "Leistung", "Power"),
    ("volume_flow", "Durchfluss", "Volume flow"),
    ("mass_flow", "Massenstrom", "Mass flow"),
    ("flow_temperature", "Vorlauftemperatur", "Flow temperature"),
    (
        "return_temperature",
        "Rücklauftemperatur",
        "Return temperature",
    ),
    (
        "temperature_difference",
        "Temperaturdifferenz",
        "Temperature difference",
    ),
    (
        "external_temperature",
        "Aussentemperatur",
        "External temperature",
    ),
    ("pressure", "Druck", "Pressure"),
    ("date", "Datum", "Date"),
    ("date_time", "Datum/Zeit", "Date/time"),
    ("hca_units", "HKV-Einheiten", "HCA units"),
    (
        "averaging_duration",
        "Mittelungsdauer",
        "Averaging duration",
    ),
    (
        "actuality_duration",
        "Aktualitätsdauer",
        "Actuality duration",
    ),
    ("fabrication_no", "Fabrikationsnummer", "Fabrication number"),
    (
        "enhanced_id",
        "Erweiterte Identifikation",
        "Enhanced identification",
    ),
    ("bus_address", "Busadresse", "Bus address"),
    ("value", "Wert", "Value"),
    ("any_vif", "Beliebiger VIF", "Any VIF"),
    (
        "manufacturer_specific",
        "Herstellerspezifisch",
        "Manufacturer specific",
    ),
    ("unknown_vif", "Unbekannter VIF", "Unknown VIF"),
    ("access_no", "Zugriffsnummer", "Access number"),
    ("medium", "Medium", "Medium"),
    ("manufacturer", "Hersteller", "Manufacturer"),
    ("parameter_set_id", "Parametersatz-ID", "Parameter set ID"),
    ("model_version", "Modell/Version", "Model/version"),
    ("hardware_version", "Hardwareversion", "Hardware version"),
    ("firmware_version", "Firmwareversion", "Firmware version"),
    ("software_version", "Softwareversion", "Software version"),
    ("customer_location", "Kundenstandort", "Customer location"),
    ("customer", "Kunde", "Customer"),
    ("password", "Passwort", "Password"),
    ("error_flags", "Fehlerflags", "Error flags"),
    ("digital_output", "Digitalausgang", "Digital output"),
    ("digital_input", "Digitaleingang", "Digital input"),
    ("baud_rate", "Baudrate", "Baud rate"),
    ("dimensionless", "Dimensionslos", "Dimensionless"),
    ("voltage", "Spannung", "Voltage"),
    ("current", "Strom", "Current"),
    ("reset_counter", "Reset-Zähler", "Reset counter"),
    (
        "cumulation_counter",
        "Kumulationszähler",
        "Cumulation counter",
    ),
    (
        "manufacturer_data",
        "Herstellerspezifische Daten",
        "Manufacturer specific data",
    ),
    ("special_function", "Spezialfunktion", "Special function"),
    // Funktion
    ("instantaneous", "Momentanwert", "Instantaneous"),
    ("maximum", "Maximum", "Maximum"),
    ("minimum", "Minimum", "Minimum"),
    ("error_value", "Fehlerwert", "Value during error"),
    // Hinweise aus VIF-Erweiterungen
    ("note_error_code", "Fehlercode", "Error code"),
    (
        "note_only_positive",
        "nur positive Anteile",
        "positive contributions only",
    ),
    (
        "note_only_negative",
        "nur negative Anteile",
        "negative contributions only",
    ),
    (
        "note_manufacturer_specific",
        "herstellerspezifisch",
        "manufacturer specific",
    ),
    ("note_vife", "VIFE", "VIFE"),
    ("note_counter", "Zähler", "Counter"),
    // Medien
    ("medium_other", "Andere", "Other"),
    ("medium_oil", "Öl", "Oil"),
    ("medium_electricity", "Elektrizität", "Electricity"),
    ("medium_gas", "Gas", "Gas"),
    ("medium_heat_outlet", "Wärme (Rücklauf)", "Heat (outlet)"),
    ("medium_steam", "Dampf", "Steam"),
    ("medium_warm_water", "Warmwasser", "Warm water"),
    ("medium_water", "Wasser", "Water"),
    ("medium_hca", "Heizkostenverteiler", "Heat cost allocator"),
    ("medium_compressed_air", "Druckluft", "Compressed air"),
    (
        "medium_cooling_outlet",
        "Kälte (Rücklauf)",
        "Cooling (outlet)",
    ),
    ("medium_cooling_inlet", "Kälte (Vorlauf)", "Cooling (inlet)"),
    ("medium_heat_inlet", "Wärme (Vorlauf)", "Heat (inlet)"),
    ("medium_heat_cooling", "Wärme/Kälte", "Heat/cooling"),
    ("medium_bus", "Bus/System", "Bus/system"),
    ("medium_unknown", "Unbekannt", "Unknown"),
    ("medium_hot_water", "Heisswasser", "Hot water"),
    ("medium_cold_water", "Kaltwasser", "Cold water"),
    ("medium_dual_water", "Dual-Wasser", "Dual water"),
    ("medium_pressure", "Druck", "Pressure"),
    ("medium_ad_converter", "A/D-Wandler", "A/D converter"),
    ("medium_reserved", "Reserviert", "Reserved"),
];

/// Übersetzt einen Schlüssel. `note_error_code:05` → "Fehlercode 05".
pub fn label(key: &str, lang: Lang) -> String {
    let (base, arg) = match key.split_once(':') {
        Some((b, a)) => (b, Some(a)),
        None => (key, None),
    };
    let found = LABELS
        .iter()
        .find(|(k, _, _)| *k == base)
        .map(|(_, de, en)| if lang == Lang::En { *en } else { *de });
    // Grösse mit „nur negative Beiträge“ (decode::qualified), z.B. volume_negative
    let text = match (found, base.strip_suffix("_negative")) {
        (Some(t), _) => t.to_string(),
        (None, Some(q)) => format!(
            "{} ({})",
            label(q, lang),
            if lang == Lang::En {
                "negative only"
            } else {
                "nur negativ"
            }
        ),
        (None, None) => base.to_string(),
    };
    match arg {
        Some(a) => format!("{text} {a}"),
        None => text.to_string(),
    }
}

/// Alle Übersetzungen einer Sprache, für die Weboberfläche.
pub fn labels(lang: Lang) -> serde_json::Map<String, serde_json::Value> {
    LABELS
        .iter()
        .map(|(k, de, en)| {
            (
                k.to_string(),
                serde_json::Value::from(if lang == Lang::En { *en } else { *de }),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_translate() {
        assert_eq!(label("energy", Lang::En), "Energy");
        assert_eq!(label("medium_water", Lang::De), "Wasser");
        assert_eq!(label("note_error_code:05", Lang::De), "Fehlercode 05");
        assert_eq!(label("no_such_key", Lang::En), "no_such_key");
        assert_eq!(Lang::parse("en-US"), Lang::En);
        assert_eq!(Lang::parse("de-CH"), Lang::De);
    }

    #[test]
    fn no_duplicate_keys() {
        let mut keys: Vec<_> = LABELS.iter().map(|l| l.0).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }
}
