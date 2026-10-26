//! Telegramm-Analyse: erklärt ein Telegramm Byte für Byte, auch wenn es gestört ist.
//!
//! Anders als der Parser bricht die Analyse nicht beim ersten Fehler ab, sondern markiert
//! falsche Prüfsummen, fehlende und überzählige Bytes und erklärt den Rest so weit wie möglich.

use serde::Serialize;

use super::decode::{decode_records, Record};
use super::frame::{checksum, hex};
use super::tables;
use crate::i18n::{self, label, tr};

#[derive(Debug, Serialize)]
pub struct Field {
    /// Bytes als Hex
    pub bytes: String,
    pub name: String,
    pub value: String,
    /// true: hier stimmt etwas nicht
    pub bad: bool,
}

#[derive(Debug, Serialize)]
pub struct Analysis {
    /// Kurzbeschreibung, z.B. "RSP_UD: Antwort mit Daten von Adresse 1"
    pub summary: String,
    pub fields: Vec<Field>,
    pub problems: Vec<String>,
}

struct Out {
    fields: Vec<Field>,
    problems: Vec<String>,
}

impl Out {
    fn add(&mut self, bytes: &[u8], name: String, value: String) {
        self.fields.push(Field {
            bytes: hex(bytes),
            name,
            value,
            bad: false,
        });
    }

    fn bad(&mut self, bytes: &[u8], name: String, value: String) {
        self.problems.push(format!("{name}: {value}"));
        self.fields.push(Field {
            bytes: hex(bytes),
            name,
            value,
            bad: true,
        });
    }
}

pub fn analyze(bytes: &[u8]) -> Analysis {
    let mut out = Out {
        fields: Vec::new(),
        problems: Vec::new(),
    };
    let summary = match bytes.first() {
        None => {
            out.problems.push(tr("Keine Bytes", "No bytes"));
            tr("Leer", "Empty")
        }
        Some(0xE5) => {
            out.add(
                &bytes[..1],
                "ACK".into(),
                tr(
                    "Quittung: Telegramm angenommen",
                    "Acknowledge: telegram accepted",
                ),
            );
            extra(&mut out, &bytes[1..]);
            tr("Quittung (E5)", "Acknowledge (E5)")
        }
        Some(0x10) => short(&mut out, bytes),
        Some(0x68) => long(&mut out, bytes),
        Some(&b) => {
            out.bad(
                &bytes[..1],
                tr("Start", "Start"),
                format!(
                    "{} ({b:02X})",
                    tr(
                        "unbekanntes Startbyte, erwartet 10, 68 oder E5",
                        "unknown start byte, expected 10, 68 or E5"
                    )
                ),
            );
            tr("Kein M-Bus-Telegramm", "Not an M-Bus telegram")
        }
    };
    Analysis {
        summary,
        fields: out.fields,
        problems: out.problems,
    }
}

/// Überzählige Bytes nach dem Telegramm: meist ein zweiter Sender
fn extra(out: &mut Out, rest: &[u8]) {
    if !rest.is_empty() {
        out.bad(
            rest,
            tr("Zusätzliche Bytes", "Extra bytes"),
            tr(
                "nach dem Telegramm: zweiter Zähler hat gleichzeitig geantwortet (Kollision)?",
                "after the telegram: a second meter answered at the same time (collision)?",
            ),
        );
    }
}

fn missing(out: &mut Out, name: String) {
    out.bad(
        &[],
        name,
        tr(
            "fehlt (Telegramm abgeschnitten)",
            "missing (telegram cut off)",
        ),
    );
}

fn control(c: u8) -> String {
    let fcb = if c & 0x20 != 0 { ", FCB=1" } else { ", FCB=0" };
    match c & 0xCF {
        0x40 => tr(
            "SND_NKE: Kommunikation zurücksetzen",
            "SND_NKE: reset communication",
        ),
        0x43 => format!(
            "{}{fcb}",
            tr("SND_UD: Daten an den Zähler", "SND_UD: data to the meter")
        ),
        0x4B => format!(
            "{}{fcb}",
            tr("REQ_UD2: Daten anfordern", "REQ_UD2: request data")
        ),
        0x4A => format!(
            "{}{fcb}",
            tr("REQ_UD1: Alarm anfordern", "REQ_UD1: request alarm")
        ),
        0x08 => tr("RSP_UD: Antwort mit Daten", "RSP_UD: reply with data"),
        _ => tr("unbekannt", "unknown"),
    }
}

fn address(a: u8) -> String {
    match a {
        0 => tr(
            "0 (Werkseinstellung, nicht konfiguriert)",
            "0 (factory default, not configured)",
        ),
        1..=250 => format!("{a}"),
        251 | 252 => format!("{a} ({})", tr("reserviert", "reserved")),
        253 => tr(
            "253: über Sekundäradresse selektierter Zähler",
            "253: meter selected by secondary address",
        ),
        254 => tr(
            "254: Broadcast, alle Zähler antworten",
            "254: broadcast, all meters reply",
        ),
        _ => tr(
            "255: Broadcast, kein Zähler antwortet",
            "255: broadcast, no meter replies",
        ),
    }
}

fn short(out: &mut Out, b: &[u8]) -> String {
    out.add(
        &b[..1],
        tr("Start", "Start"),
        tr("Kurztelegramm", "Short frame"),
    );
    let names = [
        tr("Steuerfeld (C)", "Control (C)"),
        tr("Adresse (A)", "Address (A)"),
        tr("Prüfsumme", "Checksum"),
        tr("Stopbyte", "Stop byte"),
    ];
    let get = |k: usize| b.get(k).copied();
    match get(1) {
        Some(c) => out.add(&b[1..2], names[0].clone(), control(c)),
        None => missing(out, names[0].clone()),
    }
    match get(2) {
        Some(a) => out.add(&b[2..3], names[1].clone(), address(a)),
        None => missing(out, names[1].clone()),
    }
    match (get(1), get(2), get(3)) {
        (Some(c), Some(a), Some(cs)) => {
            let want = c.wrapping_add(a);
            if cs == want {
                out.add(&b[3..4], names[2].clone(), tr("stimmt", "correct"));
            } else {
                out.bad(
                    &b[3..4],
                    names[2].clone(),
                    format!("{} {want:02X}", tr("falsch, erwartet", "wrong, expected")),
                );
            }
        }
        _ => missing(out, names[2].clone()),
    }
    stop(out, b, 4, names[3].clone());
    extra(out, b.get(5..).unwrap_or(&[]));
    match (get(1), get(2)) {
        (Some(c), Some(a)) => format!("{} → {}", control(c), address(a)),
        _ => tr("Kurztelegramm (abgeschnitten)", "Short frame (cut off)"),
    }
}

fn stop(out: &mut Out, b: &[u8], at: usize, name: String) {
    match b.get(at) {
        Some(0x16) => out.add(&b[at..at + 1], name, tr("stimmt", "correct")),
        Some(_) => out.bad(
            &b[at..at + 1],
            name,
            tr("falsch, erwartet 16", "wrong, expected 16"),
        ),
        None => missing(out, name),
    }
}

fn long(out: &mut Out, b: &[u8]) -> String {
    out.add(
        &b[..1],
        tr("Start", "Start"),
        tr("Langtelegramm", "Long frame"),
    );
    let Some(&l) = b.get(1) else {
        missing(out, tr("Länge (L)", "Length (L)"));
        return tr("Langtelegramm (abgeschnitten)", "Long frame (cut off)");
    };
    out.add(
        &b[1..2],
        tr("Länge (L)", "Length (L)"),
        format!(
            "{l} {}",
            tr("Bytes von C bis Daten", "bytes from C to data")
        ),
    );
    match b.get(2) {
        Some(&l2) if l2 == l => out.add(
            &b[2..3],
            tr("Länge (Wiederholung)", "Length (repeat)"),
            tr("stimmt", "correct"),
        ),
        Some(_) => out.bad(
            &b[2..3],
            tr("Länge (Wiederholung)", "Length (repeat)"),
            tr("weicht ab", "differs"),
        ),
        None => missing(out, tr("Länge (Wiederholung)", "Length (repeat)")),
    }
    match b.get(3) {
        Some(0x68) => out.add(
            &b[3..4],
            tr("Start (Wiederholung)", "Start (repeat)"),
            "68".into(),
        ),
        Some(_) => out.bad(
            &b[3..4],
            tr("Start (Wiederholung)", "Start (repeat)"),
            tr("falsch, erwartet 68", "wrong, expected 68"),
        ),
        None => missing(out, tr("Start (Wiederholung)", "Start (repeat)")),
    }
    let l = l as usize;
    let end = (4 + l).min(b.len());
    let body = b.get(4..end).unwrap_or(&[]);
    let (c, a, ci) = (body.first(), body.get(1), body.get(2));
    match c {
        Some(&c) => out.add(&b[4..5], tr("Steuerfeld (C)", "Control (C)"), control(c)),
        None => missing(out, tr("Steuerfeld (C)", "Control (C)")),
    }
    match a {
        Some(&a) => out.add(&b[5..6], tr("Adresse (A)", "Address (A)"), address(a)),
        None => missing(out, tr("Adresse (A)", "Address (A)")),
    }
    let summary = match (c, a) {
        (Some(&c), Some(&a)) => format!(
            "{} · {} {}",
            control(c),
            tr("Adresse", "address"),
            address(a)
        ),
        _ => tr("Langtelegramm (abgeschnitten)", "Long frame (cut off)"),
    };
    if let Some(&ci) = ci {
        application(out, ci, &body[3..], 7);
    } else {
        missing(out, tr("CI-Feld", "CI field"));
    }
    if body.len() < l {
        out.problems.push(format!(
            "{} {} {}",
            tr("Es fehlen", "Missing"),
            l - body.len() + 2,
            tr("Bytes: Telegramm abgeschnitten", "bytes: telegram cut off")
        ));
        missing(out, tr("Prüfsumme", "Checksum"));
        missing(out, tr("Stopbyte", "Stop byte"));
        return summary;
    }
    let cs_at = 4 + l;
    match b.get(cs_at) {
        Some(&cs) => {
            let want = checksum(body);
            if cs == want {
                out.add(
                    &b[cs_at..cs_at + 1],
                    tr("Prüfsumme", "Checksum"),
                    tr("stimmt", "correct"),
                );
            } else {
                out.bad(
                    &b[cs_at..cs_at + 1],
                    tr("Prüfsumme", "Checksum"),
                    format!(
                        "{} {want:02X}: {}",
                        tr("falsch, erwartet", "wrong, expected"),
                        tr("Bytes unterwegs verfälscht", "bytes corrupted on the way")
                    ),
                );
            }
        }
        None => missing(out, tr("Prüfsumme", "Checksum")),
    }
    stop(out, b, cs_at + 1, tr("Stopbyte", "Stop byte"));
    extra(out, b.get(cs_at + 2..).unwrap_or(&[]));
    summary
}

mod application;

use application::application;

#[cfg(test)]
mod tests;
