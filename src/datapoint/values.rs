//! Neue Werte aus einer Auslesung auf die Datenpunkte übertragen (Verlauf, Weitergabe).

use super::*;

impl Db {
    /// Neue Auslesung auf die Datenpunkte des Zählers übertragen. `store`: Wert auch in die
    /// Wertetabelle (Verlauf, Weitergabe). Liefert Meldungen (Zählerstand gesunken).
    pub(crate) fn dp_apply(
        &mut self,
        meter_id: i64,
        r: &MeterReading,
        store: bool,
    ) -> Result<Vec<Text>> {
        let status = r.header.as_ref().map_or(0, |h| h.status);
        let utc = r.utc.unwrap_or_else(crate::clock::utc_secs);
        // weitergegeben wird nur aus Linien im Dauerbetrieb, nie beim Debuggen alter Projekte
        let send = store && self.meter_in_operation(meter_id)?;
        let mut notes = Vec::new();
        for dp in self.dp_by_meter(meter_id)? {
            // meldet der Zähler den Wert als Fehlerwert (Funktion „error_value“), ist er sofort
            // ungültig, statt still zu fehlen und erst nach drei Abständen zu veralten
            let find = |k: &str| r.records.iter().find(|x| key_of(x) == k);
            let err_key = dp
                .key
                .split_once('|')
                .map(|(_, rest)| format!("error_value|{rest}"));
            let (rec, meter_error) = match find(&dp.key) {
                Some(x) => (x, false),
                None => match err_key.as_deref().and_then(find) {
                    Some(x) => (x, true),
                    None => continue,
                },
            };
            let Some(raw) = rec.value.as_f64() else {
                continue;
            };
            let value = raw * dp.factor + dp.shift;
            let invalid = if meter_error {
                Some("error_value")
            } else {
                check(&dp.key, value, dp.last_good, status)
            };
            if invalid == Some("decreased") && dp.invalid.as_deref() != Some("decreased") {
                notes.push(both(|| {
                    format!(
                        "{}: {} {} → {} – {}",
                        dp.name,
                        tr("Zählerstand gesunken", "Meter reading decreased"),
                        dp.last_good.unwrap_or_default(),
                        value,
                        tr("Zählertausch?", "meter replaced?")
                    )
                }));
            }
            let good = if invalid.is_none() {
                Some(value)
            } else {
                dp.last_good
            };
            self.conn.execute(
                "UPDATE datapoints SET last_value = ?2, last_time = ?3, last_utc = ?4, last_good = ?5, invalid = ?6,
                   last_unsure = ?7
                 WHERE id = ?1",
                params![dp.id, value, r.timestamp, utc, good, invalid, r.time_unsure],
            )?;
            if store {
                self.conn.execute(
                    "INSERT INTO dp_values (dp, utc, time, value, valid) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![dp.id, utc, r.timestamp, value, invalid.is_none()],
                )?;
            }
            // an die Ausgänge, wie beim Datenpunkt eingestellt (auch bei vollem Speicher;
            // nicht beim blossen Nachrechnen, z.B. nach geändertem Faktor)
            if send {
                let day = r.timestamp.get(..10).unwrap_or("");
                let sample = Sample {
                    value,
                    quality: if invalid.is_some() { "invalid" } else { "good" },
                    utc,
                    unsure: r.time_unsure,
                    day,
                };
                self.dispatch(&dp, &sample)?;
            }
        }
        Ok(notes)
    }
}
