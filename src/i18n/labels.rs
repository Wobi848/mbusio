//! Namen der Messdaten: Schlüssel → Text in Deutsch und Englisch, weitere Sprachen in `locales/`

use super::{catalog, tr_in, Lang};

/// Alle Schlüssel für Messdaten: (Schlüssel, Deutsch, Englisch).
pub(super) const LABELS: &[(&str, &str, &str)] = &[
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

/// Eintrag der Tabelle in einer Sprache; fehlt die Übersetzung, gilt Englisch
fn text_of(&(key, de, en): &(&str, &str, &str), lang: Lang) -> String {
    match lang {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
        other => catalog(other)
            .labels
            .get(key)
            .map_or(en, String::as_str)
            .to_string(),
    }
}

/// Übersetzt einen Schlüssel. `note_error_code:05` → "Fehlercode 05".
pub fn label(key: &str, lang: Lang) -> String {
    let (base, arg) = match key.split_once(':') {
        Some((b, a)) => (b, Some(a)),
        None => (key, None),
    };
    let found = LABELS
        .iter()
        .find(|(k, _, _)| *k == base)
        .map(|entry| text_of(entry, lang));
    // Grösse mit „nur negative Beiträge“ (decode::qualified), z.B. volume_negative
    let text = match (found, base.strip_suffix("_negative")) {
        (Some(t), _) => t,
        (None, Some(q)) => format!(
            "{} ({})",
            label(q, lang),
            tr_in(lang, "nur negativ", "negative only")
        ),
        (None, None) => base.to_string(),
    };
    match arg {
        Some(a) => format!("{text} {a}"),
        None => text,
    }
}

/// Alle Übersetzungen einer Sprache, für die Weboberfläche.
pub fn labels(lang: Lang) -> serde_json::Map<String, serde_json::Value> {
    LABELS
        .iter()
        .map(|entry| {
            (
                entry.0.to_string(),
                serde_json::Value::from(text_of(entry, lang)),
            )
        })
        .collect()
}
