//! Feste Datenstruktur (CI 73) und Fehlermeldungen des Zählers (CI 70)

use super::*;

/// Feste Datenstruktur (CI 73), EN 1434-3: ID, Zugriffsnummer, Status,
/// Medium/Einheiten (2 Byte), zwei Zählerstände à 4 Byte.
pub(super) fn fixed(d: &[u8]) -> Result<Telegram, String> {
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

/// Text zum Anwendungsfehler des Zählers (EN 13757-3); auch für die Telegramm-Analyse
pub(in crate::mbus) fn app_error(code: u8) -> String {
    match code {
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
    }
}

/// Fehlermeldung des Zählers (CI 70), EN 13757-3.
pub(super) fn application_error(d: &[u8]) -> String {
    let code = d.first().copied().unwrap_or(0);
    let text = app_error(code);
    format!(
        "{} {code:02X}: {text}",
        tr(
            "Zähler meldet Anwendungsfehler",
            "Meter reports application error"
        )
    )
}
