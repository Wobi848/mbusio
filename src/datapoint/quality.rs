//! Qualität eines Datenpunkts und Plausibilität neuer Werte.

use super::*;

/// Qualität eines Datenpunkts: "none" (noch kein Wert), "good", "stale", "disturbed", "invalid".
/// `line_down`: beim letzten Termin war die Linie nicht erreichbar (Wandler weg, Gateway aus) oder
/// kein Zähler hat geantwortet; dann gilt der Wert sofort als gestört, nicht erst nach 3 Terminen.
pub fn quality(
    dp: &Datapoint,
    meter: &Meter,
    plan: Plan,
    line_down: bool,
    now: chrono::NaiveDateTime,
) -> &'static str {
    let Some(time) = dp.last_time.as_deref().and_then(plan::parse_time) else {
        return "none";
    };
    if dp.invalid.is_some() {
        return "invalid";
    }
    let disturbed = if plan == Plan::Off {
        meter.last_error.is_some()
    } else {
        line_down || meter.plan_fails >= DISTURBED_AFTER
    };
    if disturbed {
        return "disturbed";
    }
    if let Some(iv) = plan.interval_s() {
        if (now - time).num_seconds() as f64 > 3.0 * iv {
            return "stale";
        }
    }
    "good"
}

/// Prüft einen neuen Wert: None = plausibel, sonst Grund
pub fn check(key: &str, value: f64, last_good: Option<f64>, status: u8) -> Option<&'static str> {
    // Statusbyte: 0x08 dauerhafter, 0x10 vorübergehender Fehler des Zählers. Gilt für Messwerte
    // (Temperatur, Leistung, Durchfluss); Zählerstände führt der Zähler weiter, die bleiben gültig
    // (z.B. Luft im Rohr: Durchfluss gestört, Energiestand stimmt).
    if status & 0x18 != 0 && !is_counter(key) {
        return Some("status");
    }
    if is_counter(key) {
        if let Some(g) = last_good {
            if value < g - g.abs() * 1e-9 {
                return Some("decreased");
            }
        }
    }
    None
}
