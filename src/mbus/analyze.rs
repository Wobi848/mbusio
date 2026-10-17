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
                tr("Quittung: Telegramm angenommen", "Acknowledge: telegram accepted"),
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
    out.bad(&[], name, tr("fehlt (Telegramm abgeschnitten)", "missing (telegram cut off)"));
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
        0 => tr("0 (Werkseinstellung, nicht konfiguriert)", "0 (factory default, not configured)"),
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
    out.add(&b[..1], tr("Start", "Start"), tr("Kurztelegramm", "Short frame"));
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
        Some(_) => out.bad(&b[at..at + 1], name, tr("falsch, erwartet 16", "wrong, expected 16")),
        None => missing(out, name),
    }
}

fn long(out: &mut Out, b: &[u8]) -> String {
    out.add(&b[..1], tr("Start", "Start"), tr("Langtelegramm", "Long frame"));
    let Some(&l) = b.get(1) else {
        missing(out, tr("Länge (L)", "Length (L)"));
        return tr("Langtelegramm (abgeschnitten)", "Long frame (cut off)");
    };
    out.add(
        &b[1..2],
        tr("Länge (L)", "Length (L)"),
        format!("{l} {}", tr("Bytes von C bis Daten", "bytes from C to data")),
    );
    match b.get(2) {
        Some(&l2) if l2 == l => out.add(&b[2..3], tr("Länge (Wiederholung)", "Length (repeat)"), tr("stimmt", "correct")),
        Some(_) => out.bad(&b[2..3], tr("Länge (Wiederholung)", "Length (repeat)"), tr("weicht ab", "differs")),
        None => missing(out, tr("Länge (Wiederholung)", "Length (repeat)")),
    }
    match b.get(3) {
        Some(0x68) => out.add(&b[3..4], tr("Start (Wiederholung)", "Start (repeat)"), "68".into()),
        Some(_) => out.bad(&b[3..4], tr("Start (Wiederholung)", "Start (repeat)"), tr("falsch, erwartet 68", "wrong, expected 68")),
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
        (Some(&c), Some(&a)) => format!("{} · {} {}", control(c), tr("Adresse", "address"), address(a)),
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
                out.add(&b[cs_at..cs_at + 1], tr("Prüfsumme", "Checksum"), tr("stimmt", "correct"));
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

/// Ab dem CI-Feld; `at` = Position des ersten Datenbytes im Telegramm (nur für die Anzeige)
fn application(out: &mut Out, ci: u8, d: &[u8], _at: usize) {
    let name = tr("CI-Feld", "CI field");
    let ci_text = match ci {
        0x50 => tr("Application Reset", "Application reset"),
        0x51 => tr("Daten an den Zähler", "Data to the meter"),
        0x52 => tr("Sekundäradresse selektieren", "Select secondary address"),
        0x70 => tr("Fehlermeldung des Zählers", "Error report from the meter"),
        0x72 => tr("Antwort, variable Datenstruktur, langer Kopf", "Reply, variable data structure, long header"),
        0x7A => tr("Antwort, variable Datenstruktur, kurzer Kopf", "Reply, variable data structure, short header"),
        0x73 => tr("Antwort, feste Datenstruktur", "Reply, fixed data structure"),
        0xB8..=0xBF => format!(
            "{} {} Bd",
            tr("Baudrate umstellen auf", "Switch baud rate to"),
            crate::mbus::master::BAUD_RATES[(ci - 0xB8) as usize]
        ),
        _ => tr("unbekannt", "unknown"),
    };
    out.add(&[ci], name, format!("{ci:02X}: {ci_text}"));
    match ci {
        0x72 => {
            if d.len() < 12 {
                out.bad(d, tr("Datenkopf", "Data header"), tr("zu kurz", "too short"));
                return;
            }
            long_header(out, &d[..12]);
            records(out, &d[12..]);
        }
        0x7A => {
            if d.len() < 4 {
                out.bad(d, tr("Datenkopf", "Data header"), tr("zu kurz", "too short"));
                return;
            }
            out.add(&d[..1], tr("Zugriffsnummer", "Access number"), d[0].to_string());
            status(out, d[1]);
            signature(out, &d[2..4]);
            records(out, &d[4..]);
        }
        0x52 if d.len() >= 8 => {
            let wild = |s: String| if s.chars().all(|c| c == 'F') { tr("beliebig", "any") } else { s };
            out.add(&d[..4], "ID".into(), wild(format!("{:02X}{:02X}{:02X}{:02X}", d[3], d[2], d[1], d[0])));
            let man = u16::from_le_bytes([d[4], d[5]]);
            out.add(&d[4..6], tr("Hersteller", "Manufacturer"), if man == 0xFFFF { tr("beliebig", "any") } else { tables::manufacturer(man) });
            out.add(&d[6..7], tr("Version", "Version"), if d[6] == 0xFF { tr("beliebig", "any") } else { d[6].to_string() });
            out.add(&d[7..8], "Medium".into(), if d[7] == 0xFF { tr("beliebig", "any") } else { label(tables::medium_key(d[7]), i18n::current()) });
        }
        0x51 => records(out, d),
        0x70 => {
            let code = d.first().copied().unwrap_or(0);
            out.bad(
                &d[..d.len().min(1)],
                tr("Fehlercode", "Error code"),
                format!("{code:02X}: {}", app_error(code)),
            );
        }
        0x50 if !d.is_empty() => out.add(d, tr("Subcode", "Subcode"), hex(d)),
        _ if !d.is_empty() => out.add(d, tr("Daten", "Data"), String::new()),
        _ => {}
    }
}

fn app_error(code: u8) -> String {
    match code {
        0 => tr("nicht näher bezeichnet", "unspecified"),
        1 => tr("CI-Feld nicht unterstützt", "unimplemented CI field"),
        2 => tr("Puffer zu lang", "buffer too long"),
        3 => tr("zu viele Datensätze", "too many records"),
        4 => tr("Datensatz vorzeitig beendet", "premature end of record"),
        5 => tr("mehr als 10 DIFE", "more than 10 DIFEs"),
        6 => tr("mehr als 10 VIFE", "more than 10 VIFEs"),
        8 => tr("Zähler beschäftigt", "application too busy"),
        9 => tr("zu viele Auslesungen", "too many readouts"),
        _ => tr("reserviert", "reserved"),
    }
}

fn long_header(out: &mut Out, h: &[u8]) {
    let id = format!("{:02X}{:02X}{:02X}{:02X}", h[3], h[2], h[1], h[0]);
    if id.chars().all(|c| c.is_ascii_digit()) {
        out.add(&h[..4], "ID".into(), id);
    } else {
        out.bad(&h[..4], "ID".into(), format!("{id} ({})", tr("kein gültiges BCD", "not valid BCD")));
    }
    let man = u16::from_le_bytes([h[4], h[5]]);
    out.add(&h[4..6], tr("Hersteller", "Manufacturer"), tables::manufacturer(man));
    out.add(&h[6..7], tr("Version", "Version"), h[6].to_string());
    out.add(&h[7..8], "Medium".into(), label(tables::medium_key(h[7]), i18n::current()));
    out.add(&h[8..9], tr("Zugriffsnummer", "Access number"), h[8].to_string());
    status(out, h[9]);
    signature(out, &h[10..12]);
}

fn signature(out: &mut Out, sig: &[u8]) {
    let text = if sig.iter().all(|b| *b == 0) {
        tr("keine Verschlüsselung", "no encryption")
    } else {
        tr(
            "Verschlüsselung/Konfiguration gesetzt: Daten evtl. nicht lesbar ohne Schlüssel",
            "encryption/configuration set: data may not be readable without a key",
        )
    };
    out.add(sig, tr("Signatur", "Signature"), text);
}

fn status(out: &mut Out, s: u8) {
    let mut parts = Vec::new();
    match s & 0x03 {
        1 => parts.push(tr("beschäftigt", "busy")),
        2 => parts.push(tr("Anwendungsfehler", "application error")),
        3 => parts.push(tr("Alarm", "alarm")),
        _ => {}
    }
    if s & 0x04 != 0 {
        parts.push(tr("Batterie/Spannung niedrig", "power low"));
    }
    if s & 0x08 != 0 {
        parts.push(tr("dauerhafter Fehler", "permanent error"));
    }
    if s & 0x10 != 0 {
        parts.push(tr("vorübergehender Fehler", "temporary error"));
    }
    if s & 0xE0 != 0 {
        parts.push(tr("herstellerspezifische Bits", "manufacturer-specific bits"));
    }
    let name = tr("Status", "Status");
    if parts.is_empty() {
        out.add(&[s], name, tr("kein Fehler", "no error"));
    } else if s & 0x1F != 0 {
        out.bad(&[s], name, parts.join(", "));
    } else {
        out.add(&[s], name, parts.join(", "));
    }
}

fn records(out: &mut Out, d: &[u8]) {
    if d.is_empty() {
        return;
    }
    let lang = i18n::current();
    match decode_records(d) {
        Ok((list, _)) => {
            for r in list {
                let bytes = format!("{} {} {}", r.dif, r.vif, r.raw).trim().to_string();
                out.fields.push(Field {
                    bytes,
                    name: record_name(&r, lang),
                    value: format!("{} {}", r.value_text, r.unit).trim().to_string(),
                    bad: false,
                });
            }
        }
        Err(e) => out.bad(d, tr("Datensätze", "Records"), e),
    }
}

fn record_name(r: &Record, lang: i18n::Lang) -> String {
    let mut s = label(&r.quantity, lang);
    let notes: Vec<String> = r.notes.iter().map(|n| label(n, lang)).collect();
    if !notes.is_empty() {
        s.push_str(&format!(" ({})", notes.join(", ")));
    }
    let mut extra = Vec::new();
    if r.function != "instantaneous" {
        extra.push(label(r.function, lang));
    }
    if r.storage != 0 {
        extra.push(format!("{} {}", tr("Speicher", "Storage"), r.storage));
    }
    if r.tariff != 0 {
        extra.push(format!("{} {}", tr("Tarif", "Tariff"), r.tariff));
    }
    if !extra.is_empty() {
        s.push_str(&format!(" · {}", extra.join(", ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(h: &str) -> Vec<u8> {
        h.split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect()
    }

    #[test]
    fn explains_good_reply() {
        let a = analyze(&bytes(
            "68 1F 1F 68 08 02 72 78 56 34 12 24 40 01 07 55 00 00 00 03 13 15 31 00 DA 02 3B 13 01 8B 60 04 37 18 02 18 16",
        ));
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        assert!(a.fields.iter().any(|f| f.name == "ID" && f.value == "12345678"));
        assert!(a.fields.iter().any(|f| f.value.contains("PAD")));
    }

    #[test]
    fn finds_checksum_truncation_and_extra_bytes() {
        let a = analyze(&bytes("10 7B 01 00 16"));
        assert!(a.fields.iter().any(|f| f.bad && f.value.contains("7C")), "{:?}", a.fields);
        let a = analyze(&bytes("68 1F 1F 68 08 02 72 78 56 34 12"));
        assert!(a.problems.iter().any(|p| p.contains("abgeschnitten") || p.contains("cut off")), "{:?}", a.problems);
        let a = analyze(&bytes("E5 E5"));
        assert!(a.fields.iter().any(|f| f.bad));
        let a = analyze(&bytes("68 04 04 68 08 0B 70 08 8B 16"));
        assert!(a.fields.iter().any(|f| f.bad && f.value.contains("08")));
    }
}
