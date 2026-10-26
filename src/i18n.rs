//! Mehrsprachigkeit (Deutsch, Englisch).
//!
//! Messdaten tragen sprachneutrale Schlüssel (`energy`, `medium_water` …), übersetzt
//! wird erst bei der Anzeige oder beim Export. Meldungen aus dem Backend verwenden
//! die Sprache des aktuellen Requests (Task) bzw. Threads.

use std::cell::Cell;
use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};

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

    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }
}

thread_local! {
    /// Sprache des Threads; `None` = wie die Oberfläche
    static CURRENT: Cell<Option<Lang>> = const { Cell::new(None) };
    /// vorübergehend erzwungen (`both`), gilt vor Request und Thread
    static FORCED: Cell<Option<Lang>> = const { Cell::new(None) };
}

tokio::task_local! {
    static TASK: Lang;
}

/// Sprache der Oberfläche: die der letzten Anfrage mit `X-Lang` (0 = Deutsch, 1 = Englisch). Danach
/// richten sich Arbeiten ohne eigene Anfrage (Zeitplan, Ausgänge, Linien-Threads)
static UI: AtomicU8 = AtomicU8::new(0);

/// Sprache des Threads setzen (Aufträge aus einer Anfrage)
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(Some(lang)));
}

/// Thread folgt wieder der Sprache der Oberfläche (Arbeit im Hintergrund)
pub fn follow_ui() {
    CURRENT.with(|c| c.set(None));
}

/// Sprache der Oberfläche merken (Anfrage mit `X-Lang`, beim Start die gespeicherte). Liefert, ob
/// sie sich geändert hat.
pub fn remember_ui(lang: Lang) -> bool {
    UI.swap(lang as u8, Ordering::Relaxed) != lang as u8
}

fn ui() -> Lang {
    if UI.load(Ordering::Relaxed) == Lang::En as u8 {
        Lang::En
    } else {
        Lang::De
    }
}

/// `f` mit dieser Sprache ausführen, auch über `await` hinweg (ein Request)
pub async fn scope<F: Future>(lang: Lang, f: F) -> F::Output {
    TASK.scope(lang, f).await
}

/// Erzwungene Sprache, sonst die des Requests, sonst des Threads, sonst der Oberfläche
pub fn current() -> Lang {
    FORCED
        .with(|c| c.get())
        .or_else(|| TASK.try_with(|l| *l).ok())
        .or_else(|| CURRENT.with(|c| c.get()))
        .unwrap_or_else(ui)
}

/// Meldung in der Sprache des aktuellen Threads.
pub fn tr(de: &str, en: &str) -> String {
    match current() {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
    }
}

/// Text in beiden Sprachen, gezeigt in der Sprache, die gerade fragt (Protokoll: eine Meldung
/// bleibt, die Sprache der Oberfläche kann wechseln)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub de: String,
    pub en: String,
}

impl Text {
    pub fn get(&self) -> &str {
        match current() {
            Lang::De => &self.de,
            Lang::En => &self.en,
        }
    }
}

/// Ohne Übersetzung (Telegramm, Fehlertext vom System): in beiden Sprachen gleich
impl From<String> for Text {
    fn from(s: String) -> Self {
        Text {
            de: s.clone(),
            en: s,
        }
    }
}

/// `f` einmal deutsch und einmal englisch auswerten: alles, was darin `tr` benutzt, gibt es dann
/// in beiden Sprachen
pub fn both(f: impl Fn() -> String) -> Text {
    let with = |lang| {
        let before = FORCED.with(|c| c.replace(Some(lang)));
        let s = f();
        FORCED.with(|c| c.set(before));
        s
    };
    Text {
        de: with(Lang::De),
        en: with(Lang::En),
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
    fn both_languages_and_thread_language_kept() {
        set(Lang::En);
        let t = both(|| format!("{} 3", tr("Zähler", "Meter")));
        assert_eq!((t.de.as_str(), t.en.as_str()), ("Zähler 3", "Meter 3"));
        // danach gilt wieder die Sprache des Threads
        assert_eq!(tr("ja", "yes"), "yes");
        assert_eq!(t.get(), "Meter 3");
        // ohne Übersetzung in beiden gleich
        let raw = Text::from("68 03 03 68".to_string());
        assert_eq!(raw.de, raw.en);
        follow_ui();
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
