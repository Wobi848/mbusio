//! Application-Layer (EN 13757-3): RSP_UD mit variabler oder fester Datenstruktur decodieren.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::frame::hex;
use super::tables::{self, Kind, VifInfo, Vife};
use crate::i18n::tr;

mod fixed;
mod values;

pub(super) use fixed::app_error;
use fixed::{application_error, fixed};
pub use values::scale;
use values::{bcd, interpret, le_unsigned};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub id: String,
    pub manufacturer: String,
    pub version: u8,
    pub medium_code: u8,
    /// Schlüssel, z.B. `medium_water`
    pub medium: String,
    pub access_no: u8,
    pub status: u8,
    /// Sekundäradresse: ID (8) + Hersteller (4) + Version (2) + Medium (2), hex
    pub secondary_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Schlüssel: instantaneous, maximum, minimum, error_value
    pub function: String,
    pub storage: u64,
    pub tariff: u32,
    pub subunit: u32,
    /// Schlüssel der Messgrösse, z.B. `energy`
    pub quantity: String,
    /// Zusatzhinweise als Schlüssel, z.B. `note_error_code:05`
    pub notes: Vec<String>,
    pub unit: String,
    /// Zahl, Text oder null
    pub value: Value,
    /// Exakte Textdarstellung des Werts (ohne Rundungsfehler)
    pub value_text: String,
    pub dif: String,
    pub vif: String,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct Telegram {
    pub header: Option<Header>,
    pub records: Vec<Record>,
    /// DIF 0x1F: weitere Telegramme folgen
    pub more_follow: bool,
}

pub fn decode_rsp_ud(ci: u8, data: &[u8]) -> Result<Telegram, String> {
    let (header, body) = match ci {
        0x72 => {
            if data.len() < 12 {
                return Err(tr("Datenkopf zu kurz", "Data header too short"));
            }
            (Some(long_header(&data[..12])), &data[12..])
        }
        0x7A => {
            if data.len() < 4 {
                return Err(tr("Datenkopf zu kurz", "Data header too short"));
            }
            (None, &data[4..])
        }
        0x78 => (None, data),
        0x73 => return fixed(data),
        0x70 => return Err(application_error(data)),
        _ => {
            return Err(format!(
                "{} {ci:02X}",
                tr("Nicht unterstütztes CI-Feld", "Unsupported CI field")
            ))
        }
    };
    let (records, more_follow) = decode_records(body)?;
    Ok(Telegram {
        header,
        records,
        more_follow,
    })
}

fn long_header(d: &[u8]) -> Header {
    let man = u16::from_le_bytes([d[4], d[5]]);
    Header {
        id: format!("{:02X}{:02X}{:02X}{:02X}", d[3], d[2], d[1], d[0]),
        manufacturer: tables::manufacturer(man),
        version: d[6],
        medium_code: d[7],
        medium: tables::medium_key(d[7]).into(),
        access_no: d[8],
        status: d[9],
        secondary_address: format!(
            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            d[3], d[2], d[1], d[0], d[5], d[4], d[6], d[7]
        ),
    }
}

fn take<'a>(data: &'a [u8], i: &mut usize, n: usize) -> Result<&'a [u8], String> {
    if *i + n > data.len() {
        return Err(format!(
            "{} {}",
            tr(
                "Datensatz abgeschnitten bei Byte",
                "Record truncated at byte"
            ),
            *i
        ));
    }
    let s = &data[*i..*i + n];
    *i += n;
    Ok(s)
}

pub fn decode_records(data: &[u8]) -> Result<(Vec<Record>, bool), String> {
    let mut records = Vec::new();
    let mut more_follow = false;
    let mut i = 0;

    while i < data.len() {
        let dif = data[i];
        i += 1;
        match dif {
            0x2F => continue, // Füllbyte
            0x0F | 0x1F => {
                more_follow = dif == 0x1F;
                let rest = &data[i..];
                if !rest.is_empty() {
                    records.push(Record {
                        function: "instantaneous".into(),
                        storage: 0,
                        tariff: 0,
                        subunit: 0,
                        quantity: "manufacturer_data".into(),
                        notes: Vec::new(),
                        unit: String::new(),
                        value: Value::String(hex(rest)),
                        value_text: hex(rest),
                        dif: format!("{dif:02X}"),
                        vif: String::new(),
                        raw: hex(rest),
                    });
                }
                break;
            }
            _ => {}
        }

        let mut dif_bytes = vec![dif];
        let mut storage = ((dif >> 6) & 1) as u64;
        let mut tariff = 0u32;
        let mut subunit = 0u32;
        let mut ext = dif & 0x80 != 0;
        let mut n = 0;
        while ext {
            if n >= 10 {
                return Err(tr("Zu viele DIF-Erweiterungen", "Too many DIF extensions"));
            }
            let dife = take(data, &mut i, 1)?[0];
            dif_bytes.push(dife);
            storage |= ((dife & 0x0F) as u64) << (1 + 4 * n);
            tariff |= (((dife >> 4) & 0x03) as u32) << (2 * n);
            subunit |= (((dife >> 6) & 0x01) as u32) << n;
            n += 1;
            ext = dife & 0x80 != 0;
        }

        let vif = take(data, &mut i, 1)?[0];
        let mut vif_bytes = vec![vif];
        let mut plain_unit = None;
        if vif & 0x7F == 0x7C {
            let len = take(data, &mut i, 1)?[0] as usize;
            let text = take(data, &mut i, len)?;
            plain_unit = Some(text.iter().rev().map(|b| *b as char).collect::<String>());
        }
        let mut vifes = Vec::new();
        let mut ext = vif & 0x80 != 0;
        while ext {
            if vifes.len() >= 10 {
                return Err(tr("Zu viele VIF-Erweiterungen", "Too many VIF extensions"));
            }
            let b = take(data, &mut i, 1)?[0];
            vif_bytes.push(b);
            vifes.push(b);
            ext = b & 0x80 != 0;
        }

        let coding = dif & 0x0F;
        let mut lvar = None;
        let len = match coding {
            0x0 | 0x8 => 0,
            0x1 | 0x9 => 1,
            0x2 | 0xA => 2,
            0x3 | 0xB => 3,
            0x4 | 0x5 | 0xC => 4,
            0x6 | 0xE => 6,
            0x7 => 8,
            0xD => {
                let l = take(data, &mut i, 1)?[0];
                lvar = Some(l);
                match l {
                    0x00..=0xBF => l as usize,
                    0xC0..=0xEF => (l & 0x0F) as usize,
                    0xF0..=0xF4 => 4 * (l as usize - 0xEC),
                    0xF5 => 48,
                    0xF6 => 64,
                    _ => return Err(format!("LVAR {l:02X} {}", tr("ungültig", "invalid"))),
                }
            }
            _ => {
                // 0xF mit Speicher/Funktion: Rest ist Spezialfunktion
                let rest = &data[i..];
                i = data.len();
                records.push(Record {
                    function: "instantaneous".into(),
                    storage,
                    tariff,
                    subunit,
                    quantity: "special_function".into(),
                    notes: Vec::new(),
                    unit: String::new(),
                    value: Value::String(hex(rest)),
                    value_text: hex(rest),
                    dif: hex(&dif_bytes),
                    vif: hex(&vif_bytes),
                    raw: hex(rest),
                });
                continue;
            }
        };
        let raw = take(data, &mut i, len)?;

        let mut vi = vif_info(vif, &vifes);
        if let Some(u) = plain_unit {
            vi.quantity = "value";
            vi.unit = u;
        }
        let (value, value_text) = interpret(coding, lvar, raw, &vi);

        records.push(Record {
            function: match (dif >> 4) & 0x03 {
                0 => "instantaneous",
                1 => "maximum",
                2 => "minimum",
                _ => "error_value",
            }
            .into(),
            storage,
            tariff,
            subunit,
            quantity: qualified(vi.quantity, &vi.notes),
            notes: vi.notes,
            unit: vi.unit,
            value,
            value_text,
            dif: hex(&dif_bytes),
            vif: hex(&vif_bytes),
            raw: hex(raw),
        });
    }
    Ok((records, more_follow))
}

/// Grösse mit Zusatz, der einen eigenen Wert ergibt: „nur negative Beiträge“ (VIFE 0x3C) ist bei
/// Wärme-/Kältezählern die Kälteenergie neben der Wärmeenergie. Ohne eigene Bezeichnung hätten
/// beide denselben Schlüssel, und die Kälteenergie liesse sich nicht wählen.
fn qualified(quantity: &str, notes: &[String]) -> String {
    if !notes.iter().any(|n| n == "note_only_negative") {
        return quantity.into();
    }
    match quantity {
        "energy" => "energy_cooling".into(),
        q => format!("{q}_negative"),
    }
}

fn vif_info(vif: u8, vifes: &[u8]) -> VifInfo {
    let (mut vi, rest) = match vif & 0x7F {
        0x7D if !vifes.is_empty() => (tables::vif_fd(vifes[0] & 0x7F), &vifes[1..]),
        0x7B if !vifes.is_empty() => (tables::vif_fb(vifes[0] & 0x7F), &vifes[1..]),
        0x7F => return tables::primary_vif(0x7F),
        v => (tables::primary_vif(v), vifes),
    };
    for b in rest {
        match tables::combinable_vife(b & 0x7F) {
            Vife::Note(t) => vi.notes.push(t),
            Vife::PerUnit(u) => vi.unit.push_str(u),
            Vife::Exponent(e) => vi.exponent += e,
        }
    }
    vi
}

#[cfg(test)]
mod tests;
