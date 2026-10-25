//! Werte eines Zählers zur Auswahl anbieten und die Auswahl als Datenpunkte übernehmen.

use super::*;

impl Db {
    /// Werte eines Zählers zur Auswahl: alle Zahlenwerte seiner letzten Auslesung, mit Kennung,
    /// Bezeichnung, Wert, ob schon Datenpunkt und ob Hauptwert (Vorschlag)
    pub fn dp_candidates(&self, meter_id: i64, lang: Lang) -> Result<Value> {
        let meter = self.meter(meter_id)?;
        let existing: HashMap<String, i64> = self
            .dp_by_meter(meter_id)?
            .into_iter()
            .map(|d| (d.key, d.id))
            .collect();
        let reading = self.latest_reading(meter_id)?;
        let main = reading.as_ref().and_then(crate::db::main_value).map(key_of);
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();
        for r in reading.iter().flat_map(|r| r.records.iter()) {
            if !r.value.is_number() {
                continue;
            }
            let key = key_of(r);
            if !seen.insert(key.clone()) {
                continue;
            }
            items.push(json!({
                "key": key,
                "label": key_label(&key, lang),
                "value": r.value_text,
                "unit": r.unit,
                "selected": existing.contains_key(&key),
                "main": main.as_deref() == Some(key.as_str()),
                "counter": is_counter(&key),
            }));
        }
        // Datenpunkte, deren Wert in der letzten Auslesung fehlt, trotzdem zeigen
        for (key, _) in existing.iter().filter(|(k, _)| !seen.contains(*k)) {
            items.push(json!({ "key": key, "label": key_label(key, lang), "value": null, "unit": key_unit(key),
                "selected": true, "main": false, "counter": is_counter(key) }));
        }
        Ok(json!({
            "meter": meter,
            "read": reading.as_ref().map(|r| r.timestamp.clone()),
            "values": items,
            "template": self.templates(meter.line_id)?.into_iter().find(|t| Some(&t.medium) == meter.medium.as_ref()),
        }))
    }

    /// Auswahl eines Zählers setzen: fehlende Datenpunkte anlegen (gleich an diese Ausgänge),
    /// abgewählte löschen
    pub fn dp_select(
        &mut self,
        meter_id: i64,
        keys: &[String],
        outputs: &[String],
        lang: Lang,
    ) -> Result<(usize, usize)> {
        let meter = self.meter(meter_id)?;
        self.tx(|db| {
            let mut added = 0;
            for k in keys {
                if let Some(id) = db.dp_create(&meter, k, lang)? {
                    for o in outputs {
                        db.dp_set_output(id, o, true)?;
                    }
                    added += 1;
                }
            }
            let mut removed = 0;
            for d in db.dp_by_meter(meter_id)? {
                if !keys.contains(&d.key) {
                    db.conn
                        .execute("DELETE FROM datapoints WHERE id = ?1", [d.id])?;
                    removed += 1;
                }
            }
            Ok((added, removed))
        })
    }
}
