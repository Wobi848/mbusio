//! Anwendungsschicht (EN 13757-3) eines Telegramms erklären: Datenkopf, Status, Datensätze

use super::*;

/// Ab dem CI-Feld; `at` = Position des ersten Datenbytes im Telegramm (nur für die Anzeige)
pub(super) fn application(out: &mut Out, ci: u8, d: &[u8], _at: usize) {
    let name = tr("CI-Feld", "CI field");
    let ci_text = match ci {
        0x50 => tr("Application Reset", "Application reset"),
        0x51 => tr("Daten an den Zähler", "Data to the meter"),
        0x52 => tr("Sekundäradresse selektieren", "Select secondary address"),
        0x70 => tr("Fehlermeldung des Zählers", "Error report from the meter"),
        0x72 => tr(
            "Antwort, variable Datenstruktur, langer Kopf",
            "Reply, variable data structure, long header",
        ),
        0x7A => tr(
            "Antwort, variable Datenstruktur, kurzer Kopf",
            "Reply, variable data structure, short header",
        ),
        0x73 => tr(
            "Antwort, feste Datenstruktur",
            "Reply, fixed data structure",
        ),
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
                out.bad(
                    d,
                    tr("Datenkopf", "Data header"),
                    tr("zu kurz", "too short"),
                );
                return;
            }
            long_header(out, &d[..12]);
            records(out, &d[12..]);
        }
        0x7A => {
            if d.len() < 4 {
                out.bad(
                    d,
                    tr("Datenkopf", "Data header"),
                    tr("zu kurz", "too short"),
                );
                return;
            }
            out.add(
                &d[..1],
                tr("Zugriffsnummer", "Access number"),
                d[0].to_string(),
            );
            status(out, d[1]);
            signature(out, &d[2..4]);
            records(out, &d[4..]);
        }
        0x52 if d.len() >= 8 => {
            let wild = |s: String| {
                if s.chars().all(|c| c == 'F') {
                    tr("beliebig", "any")
                } else {
                    s
                }
            };
            out.add(
                &d[..4],
                "ID".into(),
                wild(format!("{:02X}{:02X}{:02X}{:02X}", d[3], d[2], d[1], d[0])),
            );
            let man = u16::from_le_bytes([d[4], d[5]]);
            out.add(
                &d[4..6],
                tr("Hersteller", "Manufacturer"),
                if man == 0xFFFF {
                    tr("beliebig", "any")
                } else {
                    tables::manufacturer(man)
                },
            );
            out.add(
                &d[6..7],
                tr("Version", "Version"),
                if d[6] == 0xFF {
                    tr("beliebig", "any")
                } else {
                    d[6].to_string()
                },
            );
            out.add(
                &d[7..8],
                "Medium".into(),
                if d[7] == 0xFF {
                    tr("beliebig", "any")
                } else {
                    label(tables::medium_key(d[7]), i18n::current())
                },
            );
        }
        0x51 => records(out, d),
        0x70 => {
            let code = d.first().copied().unwrap_or(0);
            out.bad(
                &d[..d.len().min(1)],
                tr("Fehlercode", "Error code"),
                format!("{code:02X}: {}", crate::mbus::decode::app_error(code)),
            );
        }
        0x50 if !d.is_empty() => out.add(d, tr("Subcode", "Subcode"), hex(d)),
        _ if !d.is_empty() => out.add(d, tr("Daten", "Data"), String::new()),
        _ => {}
    }
}

fn long_header(out: &mut Out, h: &[u8]) {
    let id = format!("{:02X}{:02X}{:02X}{:02X}", h[3], h[2], h[1], h[0]);
    if id.chars().all(|c| c.is_ascii_digit()) {
        out.add(&h[..4], "ID".into(), id);
    } else {
        out.bad(
            &h[..4],
            "ID".into(),
            format!("{id} ({})", tr("kein gültiges BCD", "not valid BCD")),
        );
    }
    let man = u16::from_le_bytes([h[4], h[5]]);
    out.add(
        &h[4..6],
        tr("Hersteller", "Manufacturer"),
        tables::manufacturer(man),
    );
    out.add(&h[6..7], tr("Version", "Version"), h[6].to_string());
    out.add(
        &h[7..8],
        "Medium".into(),
        label(tables::medium_key(h[7]), i18n::current()),
    );
    out.add(
        &h[8..9],
        tr("Zugriffsnummer", "Access number"),
        h[8].to_string(),
    );
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
        parts.push(tr(
            "herstellerspezifische Bits",
            "manufacturer-specific bits",
        ));
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
        extra.push(label(&r.function, lang));
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
