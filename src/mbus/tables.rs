//! Tabellen aus EN 13757-3: Medium, Hersteller, VIF-Codes.
//!
//! Grössen, Medien und Funktionen werden als sprachneutrale Schlüssel geliefert,
//! die Übersetzung steht in `crate::i18n`.

pub fn medium_key(code: u8) -> &'static str {
    match code {
        0x00 => "medium_other",
        0x01 => "medium_oil",
        0x02 => "medium_electricity",
        0x03 => "medium_gas",
        0x04 => "medium_heat_outlet",
        0x05 => "medium_steam",
        0x06 => "medium_warm_water",
        0x07 => "medium_water",
        0x08 => "medium_hca",
        0x09 => "medium_compressed_air",
        0x0A => "medium_cooling_outlet",
        0x0B => "medium_cooling_inlet",
        0x0C => "medium_heat_inlet",
        0x0D => "medium_heat_cooling",
        0x0E => "medium_bus",
        0x0F => "medium_unknown",
        0x15 => "medium_hot_water",
        0x16 => "medium_cold_water",
        0x17 => "medium_dual_water",
        0x18 => "medium_pressure",
        0x19 => "medium_ad_converter",
        _ => "medium_reserved",
    }
}

/// Herstellerkennung (3 Buchstaben, FLAG-Register).
pub fn manufacturer(code: u16) -> String {
    [(code >> 10) & 0x1F, (code >> 5) & 0x1F, code & 0x1F]
        .iter()
        .map(|c| char::from(*c as u8 + 64))
        .collect()
}

pub fn manufacturer_code(name: &str) -> u16 {
    name.bytes().take(3).fold(0u16, |acc, c| {
        (acc << 5) | (c.to_ascii_uppercase().saturating_sub(64) as u16 & 0x1F)
    })
}

/// Einheiten der festen Datenstruktur (6 Bit): (Grösse, Einheit, Zehnerexponent).
/// Aufbau: je Einheit drei Stufen ×1, ×10, ×100.
pub fn fixed_unit(code: u8) -> (&'static str, &'static str, i32) {
    const STEPS: &[(u8, &str, &str)] = &[
        (0x02, "energy", "Wh"),
        (0x05, "energy", "kWh"),
        (0x08, "energy", "MWh"),
        (0x0B, "energy", "kJ"),
        (0x0E, "energy", "MJ"),
        (0x11, "energy", "GJ"),
        (0x14, "power", "W"),
        (0x17, "power", "kW"),
        (0x1A, "power", "MW"),
        (0x1D, "power", "kJ/h"),
        (0x20, "power", "MJ/h"),
        (0x23, "power", "GJ/h"),
        (0x26, "volume", "ml"),
        (0x29, "volume", "l"),
        (0x2C, "volume", "m³"),
        (0x2F, "volume_flow", "ml/h"),
        (0x32, "volume_flow", "l/h"),
        (0x35, "volume_flow", "m³/h"),
    ];
    for &(base, quantity, unit) in STEPS {
        if (base..base + 3).contains(&code) {
            return (quantity, unit, (code - base) as i32);
        }
    }
    match code {
        0x00 => ("value", "h,m,s", 0),
        0x01 => ("value", "D,M,Y", 0),
        0x38 => ("external_temperature", "°C", -3),
        0x39 => ("hca_units", "", 0),
        _ => ("value", "", 0),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Messwert mit Zehnerexponent
    Numeric,
    /// Zahl ohne Skalierung (Nummern, Zähler, Flags)
    Plain,
    /// Datum, Typ G
    Date,
    /// Datum und Uhrzeit, Typ F (4 Byte) oder I (6 Byte)
    DateTime,
}

#[derive(Debug, Clone)]
pub struct VifInfo {
    pub quantity: &'static str,
    pub unit: String,
    pub exponent: i32,
    pub kind: Kind,
    pub notes: Vec<String>,
}

fn info(quantity: &'static str, unit: &str, exponent: i32, kind: Kind) -> VifInfo {
    VifInfo {
        quantity,
        unit: unit.into(),
        exponent,
        kind,
        notes: Vec::new(),
    }
}

fn time_unit(n: u8) -> &'static str {
    match n & 0x03 {
        0 => "s",
        1 => "min",
        2 => "h",
        _ => "d",
    }
}

/// Primäre VIF-Tabelle. `vif` ohne Erweiterungsbit.
pub fn primary_vif(vif: u8) -> VifInfo {
    use Kind::*;
    let n = (vif & 0x07) as i32;
    let n2 = (vif & 0x03) as i32;
    match vif {
        0x00..=0x07 => info("energy", "kWh", n - 6, Numeric),
        0x08..=0x0F => info("energy", "kJ", n - 3, Numeric),
        0x10..=0x17 => info("volume", "m³", n - 6, Numeric),
        0x18..=0x1F => info("mass", "kg", n - 3, Numeric),
        0x20..=0x23 => info("on_time", time_unit(vif), 0, Numeric),
        0x24..=0x27 => info("operating_time", time_unit(vif), 0, Numeric),
        0x28..=0x2F => info("power", "W", n - 3, Numeric),
        0x30..=0x37 => info("power", "kJ/h", n - 3, Numeric),
        0x38..=0x3F => info("volume_flow", "m³/h", n - 6, Numeric),
        0x40..=0x47 => info("volume_flow", "m³/min", n - 7, Numeric),
        0x48..=0x4F => info("volume_flow", "m³/s", n - 9, Numeric),
        0x50..=0x57 => info("mass_flow", "kg/h", n - 3, Numeric),
        0x58..=0x5B => info("flow_temperature", "°C", n2 - 3, Numeric),
        0x5C..=0x5F => info("return_temperature", "°C", n2 - 3, Numeric),
        0x60..=0x63 => info("temperature_difference", "K", n2 - 3, Numeric),
        0x64..=0x67 => info("external_temperature", "°C", n2 - 3, Numeric),
        0x68..=0x6B => info("pressure", "bar", n2 - 3, Numeric),
        0x6C => info("date", "", 0, Date),
        0x6D => info("date_time", "", 0, DateTime),
        0x6E => info("hca_units", "", 0, Numeric),
        0x70..=0x73 => info("averaging_duration", time_unit(vif), 0, Numeric),
        0x74..=0x77 => info("actuality_duration", time_unit(vif), 0, Numeric),
        0x78 => info("fabrication_no", "", 0, Plain),
        0x79 => info("enhanced_id", "", 0, Plain),
        0x7A => info("bus_address", "", 0, Plain),
        0x7C => info("value", "", 0, Numeric),
        0x7E => info("any_vif", "", 0, Plain),
        0x7F => info("manufacturer_specific", "", 0, Plain),
        _ => info("unknown_vif", "", 0, Plain),
    }
}

/// Erweiterte VIF-Tabelle (VIF = 0xFD), `code` ohne Erweiterungsbit.
pub fn vif_fd(code: u8) -> VifInfo {
    use Kind::*;
    let n = (code & 0x0F) as i32;
    match code {
        0x08 => info("access_no", "", 0, Plain),
        0x09 => info("medium", "", 0, Plain),
        0x0A => info("manufacturer", "", 0, Plain),
        0x0B => info("parameter_set_id", "", 0, Plain),
        0x0C => info("model_version", "", 0, Plain),
        0x0D => info("hardware_version", "", 0, Plain),
        0x0E => info("firmware_version", "", 0, Plain),
        0x0F => info("software_version", "", 0, Plain),
        0x10 => info("customer_location", "", 0, Plain),
        0x11 => info("customer", "", 0, Plain),
        0x16 => info("password", "", 0, Plain),
        0x17 => info("error_flags", "", 0, Plain),
        0x1A => info("digital_output", "", 0, Plain),
        0x1B => info("digital_input", "", 0, Plain),
        0x1C => info("baud_rate", "Bd", 0, Plain),
        0x3A => info("dimensionless", "", 0, Numeric),
        0x40..=0x4F => info("voltage", "V", n - 9, Numeric),
        0x50..=0x5F => info("current", "A", n - 12, Numeric),
        0x60 => info("reset_counter", "", 0, Plain),
        0x61 => info("cumulation_counter", "", 0, Plain),
        _ => info("unknown_vif", "", 0, Plain),
    }
}

/// Erweiterte VIF-Tabelle (VIF = 0xFB), `code` ohne Erweiterungsbit.
pub fn vif_fb(code: u8) -> VifInfo {
    use Kind::*;
    let n = (code & 0x01) as i32;
    match code {
        0x00..=0x01 => info("energy", "kWh", n + 2, Numeric),
        0x08..=0x09 => info("energy", "GJ", n - 1, Numeric),
        0x10..=0x11 => info("volume", "m³", n + 2, Numeric),
        0x18..=0x19 => info("mass", "t", n + 2, Numeric),
        0x28..=0x29 => info("power", "kW", n + 2, Numeric),
        0x30..=0x31 => info("power", "GJ/h", n - 1, Numeric),
        _ => info("unknown_vif", "", 0, Plain),
    }
}

/// Wirkung einer kombinierbaren VIF-Erweiterung (VIFE).
pub enum Vife {
    /// Hinweis als Schlüssel, optional mit Argument nach `:`
    Note(String),
    /// Einheit ergänzen, z.B. "/h"
    PerUnit(&'static str),
    /// Exponent anpassen
    Exponent(i32),
}

pub fn combinable_vife(code: u8) -> Vife {
    match code {
        0x00..=0x1F => Vife::Note(format!("note_error_code:{code:02X}")),
        0x20 => Vife::PerUnit("/s"),
        0x21 => Vife::PerUnit("/min"),
        0x22 => Vife::PerUnit("/h"),
        0x23 => Vife::PerUnit("/d"),
        0x24 => Vife::PerUnit("/wk"),
        0x25 => Vife::PerUnit("/mo"),
        0x26 => Vife::PerUnit("/a"),
        0x27 => Vife::PerUnit("/rev"),
        0x3B => Vife::Note("note_only_positive".into()),
        0x3C => Vife::Note("note_only_negative".into()),
        0x70..=0x77 => Vife::Exponent((code & 0x07) as i32 - 6),
        0x7D => Vife::Exponent(3),
        0x7F => Vife::Note("note_manufacturer_specific".into()),
        _ => Vife::Note(format!("note_vife:{code:02X}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manufacturer_roundtrip() {
        assert_eq!(manufacturer(0x2C2D), "KAM");
        assert_eq!(manufacturer_code("KAM"), 0x2C2D);
        assert_eq!(manufacturer(0x4024), "PAD");
    }
}
