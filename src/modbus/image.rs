//! Registerbild: aus den Datenpunkten eines Projekts die 65536 Register, die der Server liefert.

use serde_json::Value;

/// Register je Datenpunkt
pub const BLOCK: u16 = 10;
/// Ab hier der Zustand von mbusio; Blöcke liegen darunter
pub const STATUS_START: u16 = 60000;

/// Startregister des Blocks eines Datenpunkts mit dieser Nummer, falls er unter 60000 passt
pub fn block_start(no: i64) -> Option<u16> {
    let start = (no - 1).checked_mul(BLOCK as i64)?;
    (0..=(STATUS_START - BLOCK) as i64)
        .contains(&start)
        .then_some(start as u16)
}

/// Qualität als Zahl: 0 gut, 1 veraltet, 2 gestört, 3 ungültig, 4 kein Wert
fn quality_code(quality: &str) -> u16 {
    match quality {
        "good" => 0,
        "stale" => 1,
        "disturbed" => 2,
        "invalid" => 3,
        _ => 4,
    }
}

/// Worte eines Werts, höherwertiges zuerst; `swap`: niederwertiges zuerst
fn words(bytes: &[u8], swap: bool) -> Vec<u16> {
    let mut w: Vec<u16> = bytes
        .chunks(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    if swap {
        w.reverse();
    }
    w
}

/// Die 10 Register eines Datenpunkts
pub(super) fn block(value: f64, quality: u16, age_min: u16, swap: bool) -> [u16; BLOCK as usize] {
    let scaled = (value * 100.0)
        .round()
        .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    let mut b = [0u16; BLOCK as usize];
    b[0..4].copy_from_slice(&words(&value.to_be_bytes(), swap));
    b[4..6].copy_from_slice(&words(&(value as f32).to_be_bytes(), swap));
    b[6..8].copy_from_slice(&words(&scaled.to_be_bytes(), swap));
    b[8] = quality;
    b[9] = age_min;
    b
}

/// Registerbild aus `dp_list` eines Projekts: Datenpunkte mit Modbus-Häkchen und Adresse; Werte
/// nur aus Linien im Dauerbetrieb, sonst Qualität „kein Wert“. Dazu ab 60000 der Zustand.
pub(super) fn build(rows: &[Value], swap: bool, now_utc: i64, uptime_min: u16) -> Vec<u16> {
    let mut regs = vec![0u16; 65536];
    let (mut bad, mut stale) = (0u16, 0u16);
    for d in rows {
        let o = &d["outputs"]["modbus"];
        let Some(start) = o["address"]
            .as_str()
            .and_then(|a| a.parse::<u16>().ok())
            .filter(|_| o["enabled"] == true && d["active"] == true)
            .filter(|s| *s <= STATUS_START - BLOCK)
        else {
            continue;
        };
        let live = d["operation"] == true;
        let quality = if live {
            quality_code(d["quality"].as_str().unwrap_or(""))
        } else {
            4
        };
        let value = d["last_value"].as_f64().filter(|_| quality != 4);
        let age = d["last_utc"]
            .as_i64()
            .filter(|_| value.is_some())
            .map_or(0, |t| {
                ((now_utc - t).max(0) / 60).min(u16::MAX as i64) as u16
            });
        match quality {
            2 | 3 => bad += 1,
            1 => stale += 1,
            _ => {}
        }
        let s = start as usize;
        regs[s..s + BLOCK as usize].copy_from_slice(&block(
            value.unwrap_or(0.0),
            if value.is_some() { quality } else { 4 },
            age,
            swap,
        ));
    }
    let st = STATUS_START as usize;
    regs[st] = uptime_min;
    regs[st + 1] = bad;
    regs[st + 2] = stale;
    regs
}
