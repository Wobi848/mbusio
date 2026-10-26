//! Zählertausch: Zählerstände laufen mit dem neuen Zähler nahtlos weiter

use super::*;

impl Db {
    /// Zählertausch: Zählerstände laufen mit dem neuen Zähler durchgehend weiter. Liefert eine
    /// Zeile je Zählerstand (alter Endstand → neuer Anfangsstand).
    pub fn exchange_meter(
        &mut self,
        meter_id: i64,
        new_address: &str,
        reading: &MeterReading,
    ) -> Result<Vec<Text>> {
        let meter = self.meter(meter_id)?;
        let new_address = new_address.trim().to_ascii_uppercase();
        let target = Target::parse(&new_address)?;
        if self
            .meters(meter.line_id)?
            .iter()
            .any(|m| m.id != meter_id && crate::db::listed(m, &target))
        {
            bail!(tr(
                "Zähler mit dieser Adresse ist schon in der Liste",
                "A meter with this address is already in the list"
            ));
        }
        let new_id = reading.header.as_ref().map(|h| h.id.clone());
        self.tx(|db| {
            let mut lines: Vec<Text> = Vec::new();
            for dp in db.dp_by_meter(meter_id)? {
                if !is_counter(&dp.key) {
                    db.conn.execute("UPDATE datapoints SET invalid = NULL WHERE id = ?1", [dp.id])?;
                    continue;
                }
                let Some(old_end) = dp.last_good.or(dp.last_value) else { continue };
                let Some(start) = reading
                    .records
                    .iter()
                    .find(|x| key_of(x) == dp.key)
                    .and_then(|x| x.value.as_f64())
                else {
                    lines.push(both(|| format!("{}: {}", dp.name, tr("neuer Zähler liefert diesen Wert nicht", "new meter does not provide this value"))));
                    continue;
                };
                let start = start * dp.factor;
                db.conn.execute(
                    "UPDATE datapoints SET shift = ?2, last_good = ?3, invalid = NULL WHERE id = ?1",
                    params![dp.id, old_end - start, old_end],
                )?;
                lines.push(both(|| {
                    format!(
                        "{}: {} {old_end} {} → {} {start} {}",
                        dp.name,
                        tr("Endstand", "final"),
                        dp.unit,
                        tr("Anfang neu", "new start"),
                        dp.unit
                    )
                }));
            }
            db.conn.execute(
                "UPDATE meters SET address = ?2, meter_id = NULL, manufacturer = NULL, secondary_address = NULL,
                   primary_address = NULL, last_error = NULL, plan_fails = 0, plan_pause = NULL WHERE id = ?1",
                params![meter_id, new_address],
            )?;
            // gespeichert wird in der Sprache der Anfrage, wie alle Datensätze
            let saved: Vec<&str> = lines.iter().map(|l| l.get()).collect();
            db.conn.execute(
                "INSERT INTO exchanges (meter_id, time, old_id, new_id, details) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![meter_id, crate::clock::stamp(), meter.meter_id, new_id, serde_json::to_string(&saved)?],
            )?;
            db.add_change(
                meter.line_id,
                &format!(
                    "{} {}: {} → {}{}",
                    tr("Zähler getauscht", "Meter replaced"),
                    meter_label(&meter),
                    meter.meter_id.as_deref().unwrap_or(&meter.address),
                    new_id.as_deref().unwrap_or(&new_address),
                    if lines.is_empty() { String::new() } else { format!(" ({})", saved.join("; ")) }
                ),
            )?;
            db.record(meter.line_id, &target, Ok(reading), false)?;
            Ok(lines)
        })
    }
}
