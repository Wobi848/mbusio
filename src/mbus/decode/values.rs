//! Rohbytes in Werte: Ganzzahlen, BCD, Datum und Uhrzeit, Skalierung nach Exponent

use super::*;

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

pub(super) fn le_unsigned(raw: &[u8]) -> u64 {
    raw.iter()
        .enumerate()
        .fold(0u64, |acc, (k, b)| acc | ((*b as u64) << (8 * k)))
}

/// BCD, niederwertiges Byte zuerst. Oberstes Nibble 0xF = negativ.
pub(super) fn bcd(raw: &[u8]) -> Option<i64> {
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

pub(super) fn date_g(raw: &[u8]) -> String {
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

pub(super) fn interpret(coding: u8, lvar: Option<u8>, raw: &[u8], vi: &VifInfo) -> (Value, String) {
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
