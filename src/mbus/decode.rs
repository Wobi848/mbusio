//! Application-Layer (EN 13757-3): RSP_UD mit variabler oder fester Datenstruktur decodieren.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::frame::hex;
use super::tables::{self, Kind, VifInfo, Vife};
use crate::i18n::tr;

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

/// Feste Datenstruktur (CI 73), EN 1434-3: ID, Zugriffsnummer, Status,
/// Medium/Einheiten (2 Byte), zwei Zählerstände à 4 Byte.
fn fixed(d: &[u8]) -> Result<Telegram, String> {
    if d.len() < 16 {
        return Err(tr("Datenkopf zu kurz", "Data header too short"));
    }
    let status = d[5];
    let medium = (d[6] >> 6) | ((d[7] >> 6) << 2);
    let header = Header {
        id: format!("{:02X}{:02X}{:02X}{:02X}", d[3], d[2], d[1], d[0]),
        // keine Herstellerkennung und Version in der festen Struktur
        manufacturer: String::new(),
        version: 0,
        medium_code: medium,
        medium: tables::medium_key(medium).into(),
        access_no: d[4],
        status,
        secondary_address: format!(
            "{:02X}{:02X}{:02X}{:02X}FFFFFF{medium:02X}",
            d[3], d[2], d[1], d[0]
        ),
    };
    // Status Bit 7: Zähler binär statt BCD; Bit 6: Werte vom Stichtag statt aktuell
    let binary = status & 0x80 != 0;
    let storage = u64::from(status & 0x40 != 0);
    let unit1 = d[6] & 0x3F;
    let mut records = Vec::new();
    for (k, raw) in [&d[8..12], &d[12..16]].into_iter().enumerate() {
        let mut code = d[6 + k] & 0x3F;
        let mut st = storage;
        // 3E beim zweiten Zähler: gleiche Einheit wie Zähler 1, aber historisch
        if k == 1 && code == 0x3E {
            code = unit1;
            st = 1;
        }
        let (quantity, unit, exp) = tables::fixed_unit(code);
        let v = if binary {
            Some(le_unsigned(raw) as i64)
        } else {
            bcd(raw)
        };
        let (value, value_text) = match v {
            Some(v) => scale(v, exp),
            None => (
                Value::String(hex(raw)),
                format!("{} (BCD {})", hex(raw), tr("ungültig", "invalid")),
            ),
        };
        records.push(Record {
            function: "instantaneous".into(),
            storage: st,
            tariff: 0,
            subunit: 0,
            quantity: quantity.into(),
            notes: vec![format!("note_counter:{}", k + 1)],
            unit: unit.into(),
            value,
            value_text,
            dif: String::new(),
            vif: format!("{code:02X}"),
            raw: hex(raw),
        });
    }
    Ok(Telegram {
        header: Some(header),
        records,
        more_follow: false,
    })
}

/// Fehlermeldung des Zählers (CI 70), EN 13757-3.
fn application_error(d: &[u8]) -> String {
    let code = d.first().copied().unwrap_or(0);
    let text = match code {
        0 => tr("nicht näher bezeichnet", "unspecified"),
        1 => tr("CI-Feld nicht unterstützt", "unimplemented CI field"),
        2 => tr(
            "Puffer zu lang, abgeschnitten",
            "buffer too long, truncated",
        ),
        3 => tr("zu viele Datensätze", "too many records"),
        4 => tr("Datensatz vorzeitig beendet", "premature end of record"),
        5 => tr("mehr als 10 DIFE", "more than 10 DIFEs"),
        6 => tr("mehr als 10 VIFE", "more than 10 VIFEs"),
        8 => tr("Zähler beschäftigt", "application too busy"),
        9 => tr("zu viele Auslesungen", "too many readouts"),
        _ => tr("reserviert", "reserved"),
    };
    format!(
        "{} {code:02X}: {text}",
        tr(
            "Zähler meldet Anwendungsfehler",
            "Meter reports application error"
        )
    )
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
            quantity: vi.quantity.into(),
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

fn le_signed(raw: &[u8]) -> i64 {
    let mut v: i64 = 0;
    for (k, b) in raw.iter().enumerate() {
        v |= (*b as i64) << (8 * k);
    }
    let bits = raw.len() * 8;
    if bits > 0 && bits < 64 && v & (1 << (bits - 1)) != 0 {
        v -= 1 << bits;
    }
    v
}

fn le_unsigned(raw: &[u8]) -> u64 {
    raw.iter()
        .enumerate()
        .fold(0u64, |acc, (k, b)| acc | ((*b as u64) << (8 * k)))
}

/// BCD, niederwertiges Byte zuerst. Oberstes Nibble 0xF = negativ.
fn bcd(raw: &[u8]) -> Option<i64> {
    let mut v: i64 = 0;
    let mut negative = false;
    for (k, b) in raw.iter().rev().enumerate() {
        for (j, nib) in [b >> 4, b & 0x0F].into_iter().enumerate() {
            if k == 0 && j == 0 && nib == 0xF {
                negative = true;
                continue;
            }
            if nib > 9 {
                return None;
            }
            v = v * 10 + nib as i64;
        }
    }
    Some(if negative { -v } else { v })
}

/// Ganzzahl mit Zehnerexponent exakt als Text und als Zahl.
pub fn scale(raw: i64, exp: i32) -> (Value, String) {
    let text = if exp >= 0 {
        (raw as i128 * 10i128.pow(exp as u32)).to_string()
    } else {
        let d = (-exp) as usize;
        let digits = format!("{:0>width$}", raw.unsigned_abs(), width = d + 1);
        let (int, frac) = digits.split_at(digits.len() - d);
        format!("{}{int}.{frac}", if raw < 0 { "-" } else { "" })
    };
    let num = text
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64);
    (num.map(Value::Number).unwrap_or(Value::Null), text)
}

fn date_g(raw: &[u8]) -> String {
    let day = raw[0] & 0x1F;
    let month = raw[1] & 0x0F;
    let year = ((raw[0] & 0xE0) >> 5) | ((raw[1] & 0xF0) >> 1);
    format!("{:04}-{month:02}-{day:02}", 2000 + year as u32)
}

fn datetime(raw: &[u8]) -> String {
    let (sec, r) = if raw.len() == 6 {
        (Some(raw[0] & 0x3F), &raw[1..])
    } else {
        (None, raw)
    };
    let min = r[0] & 0x3F;
    let hour = r[1] & 0x1F;
    let invalid = r[0] & 0x80 != 0;
    let date = date_g(&r[2..4]);
    let mut s = format!("{date} {hour:02}:{min:02}");
    if let Some(sec) = sec {
        s.push_str(&format!(":{sec:02}"));
    }
    if invalid {
        s.push_str(&format!(" ({})", tr("ungültig", "invalid")));
    }
    s
}

fn interpret(coding: u8, lvar: Option<u8>, raw: &[u8], vi: &VifInfo) -> (Value, String) {
    match vi.kind {
        Kind::Date if raw.len() == 2 => {
            let s = date_g(raw);
            return (Value::String(s.clone()), s);
        }
        Kind::DateTime if raw.len() == 4 || raw.len() == 6 => {
            let s = datetime(raw);
            return (Value::String(s.clone()), s);
        }
        _ => {}
    }
    let exp = if vi.kind == Kind::Numeric {
        vi.exponent
    } else {
        0
    };
    let int = |v: i64| scale(v, exp);
    match coding {
        0x0 | 0x8 => (Value::Null, String::new()),
        0x1..=0x4 | 0x6 => int(le_signed(raw)),
        0x7 => int(le_signed(raw)),
        0x5 => {
            let f = f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64 * 10f64.powi(exp);
            let text = format!("{f}");
            (
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                text,
            )
        }
        0x9..=0xC | 0xE => match bcd(raw) {
            Some(v) => int(v),
            None => (
                Value::String(hex(raw)),
                format!("{} (BCD {})", hex(raw), tr("ungültig", "invalid")),
            ),
        },
        0xD => match lvar.unwrap_or(0) {
            0x00..=0xBF => {
                let s: String = raw.iter().rev().map(|b| *b as char).collect();
                (Value::String(s.clone()), s)
            }
            0xC0..=0xCF => bcd(raw)
                .map(int)
                .unwrap_or_else(|| (Value::String(hex(raw)), hex(raw))),
            0xD0..=0xDF => bcd(raw)
                .map(|v| int(-v))
                .unwrap_or_else(|| (Value::String(hex(raw)), hex(raw))),
            0xE0..=0xEF if raw.len() <= 8 => int(le_unsigned(raw) as i64),
            _ => (Value::String(hex(raw)), hex(raw)),
        },
        _ => (Value::String(hex(raw)), hex(raw)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbus::frame::{parse, Frame, Parse};

    /// Beispiel aus der M-Bus-Dokumentation (m-bus.com, Kapitel 6.8)
    #[test]
    fn decode_reference_telegram() {
        let bytes = [
            0x68, 0x1F, 0x1F, 0x68, 0x08, 0x02, 0x72, 0x78, 0x56, 0x34, 0x12, 0x24, 0x40, 0x01,
            0x07, 0x55, 0x00, 0x00, 0x00, 0x03, 0x13, 0x15, 0x31, 0x00, 0xDA, 0x02, 0x3B, 0x13,
            0x01, 0x8B, 0x60, 0x04, 0x37, 0x18, 0x02, 0x18, 0x16,
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
}
