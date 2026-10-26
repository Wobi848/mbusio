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
        self.tx(|db| db.exchange_core(&meter, &new_address, reading, &crate::clock::stamp(), false))
    }

    /// Kern des Tauschs, ohne eigene Transaktion: auch beim Speichern einer Auslesung, wenn ein
    /// anderer Zähler antwortet (`detected`: erkannt, nicht bestätigt; Zeit = Auslesung)
    pub(crate) fn exchange_core(
        &mut self,
        meter: &Meter,
        new_address: &str,
        reading: &MeterReading,
        time: &str,
        detected: bool,
    ) -> Result<Vec<Text>> {
        let meter_id = meter.id;
        let target = Target::parse(new_address)?;
        let new_id = reading.header.as_ref().map(|h| h.id.clone());
        let mut lines: Vec<Text> = Vec::new();
        for dp in self.dp_by_meter(meter_id)? {
            if !is_counter(&dp.key) {
                self.conn.execute(
                    "UPDATE datapoints SET invalid = NULL WHERE id = ?1",
                    [dp.id],
                )?;
                continue;
            }
            let Some(old_end) = dp.last_good.or(dp.last_value) else {
                continue;
            };
            let Some(start) = reading
                .records
                .iter()
                .find(|x| key_of(x) == dp.key)
                .and_then(|x| x.value.as_f64())
            else {
                lines.push(both(|| {
                    format!(
                        "{}: {}",
                        dp.name,
                        tr(
                            "neuer Zähler liefert diesen Wert nicht",
                            "new meter does not provide this value"
                        )
                    )
                }));
                continue;
            };
            let start = start * dp.factor;
            self.conn.execute(
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
        self.conn.execute(
            "UPDATE meters SET address = ?2, meter_id = NULL, manufacturer = NULL, secondary_address = NULL,
               primary_address = NULL, last_error = NULL, plan_fails = 0, plan_pause = NULL WHERE id = ?1",
            params![meter_id, new_address],
        )?;
        self.disturbed_sync(meter_id)?;
        // gespeichert wird in der Sprache der Anfrage, wie alle Datensätze
        let saved: Vec<&str> = lines.iter().map(|l| l.get()).collect();
        self.conn.execute(
            "INSERT INTO exchanges (meter_id, time, old_id, new_id, details, confirmed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![meter_id, time, meter.meter_id, new_id, serde_json::to_string(&saved)?, !detected],
        )?;
        self.add_change(
            meter.line_id,
            &format!(
                "{} {}: {} → {}{}",
                if detected {
                    tr("Zählertausch erkannt", "Meter replacement detected")
                } else {
                    tr("Zähler getauscht", "Meter replaced")
                },
                meter_label(meter),
                meter.meter_id.as_deref().unwrap_or(&meter.address),
                new_id.as_deref().unwrap_or(new_address),
                if lines.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", saved.join("; "))
                }
            ),
        )?;
        self.record(meter.line_id, &target, Ok(reading), false)?;
        Ok(lines)
    }

    /// Erkannte, noch nicht bestätigte Tausche einer Linie, je Zähler: {id, time, old_id, new_id}
    pub fn open_swaps(&self, line_id: i64) -> Result<HashMap<i64, Value>> {
        let mut st = self.conn.prepare(
            "SELECT e.meter_id, e.id, e.time, e.old_id, e.new_id FROM exchanges e
             JOIN meters m ON m.id = e.meter_id WHERE m.line_id = ?1 AND e.confirmed = 0 ORDER BY e.time",
        )?;
        let rows = st.query_map([line_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!({ "id": r.get::<_, i64>(1)?, "time": r.get::<_, String>(2)?,
                        "old_id": r.get::<_, Option<String>>(3)?, "new_id": r.get::<_, Option<String>>(4)? }),
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Erkannten Tausch bestätigen
    pub fn confirm_swap(&mut self, id: i64) -> Result<()> {
        if self
            .conn
            .execute("UPDATE exchanges SET confirmed = 1 WHERE id = ?1", [id])?
            == 0
        {
            bail!(tr("Tausch nicht gefunden", "Replacement not found"));
        }
        Ok(())
    }
}
