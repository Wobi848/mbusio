//! Datenpunkte ändern: einzeln (mit Prüfung auf doppelte Adressen) und mehrere auf einmal.

use super::*;

impl Db {
    /// Datenpunkt ändern (Name, Einheit, Faktor, Weitergabe, Ausgänge); eine schon vergebene
    /// Adresse wird abgelehnt und nichts geändert
    pub fn dp_update(&mut self, id: i64, d: &Datapoint) -> Result<Datapoint> {
        self.tx(|db| {
            let d = db.dp_write(id, d)?;
            db.check_unique_addresses(&[id])?;
            Ok(d)
        })
    }

    /// Wie `dp_update`, aber ohne Prüfung auf doppelte Adressen: für Änderungen an mehreren
    /// Datenpunkten, die erst am Ende eindeutig sein müssen (Tausch beim CSV-Import)
    pub(super) fn dp_write(&mut self, id: i64, d: &Datapoint) -> Result<Datapoint> {
        if d.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        if !SEND_MODES.contains(&d.send.as_str()) {
            bail!("send: {}", d.send);
        }
        if !d.factor.is_finite() || d.factor == 0.0 {
            bail!(tr(
                "Faktor muss eine Zahl ungleich 0 sein",
                "Factor must be a non-zero number"
            ));
        }
        let old = self.dp(id)?;
        for (key, o) in &d.outputs {
            if let Some(a) = o
                .address
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
            {
                output(key)?.check_address(a)?;
            }
        }
        // anderer Faktor: gespeicherte Werte passen nicht mehr zum Vergleich
        let reset = old.factor != d.factor;
        self.conn.execute(
            "UPDATE datapoints SET name = ?2, unit = ?3, factor = ?4, send = ?5, active = ?6,
               last_good = CASE WHEN ?7 THEN NULL ELSE last_good END,
               invalid = CASE WHEN ?7 THEN NULL ELSE invalid END
             WHERE id = ?1",
            params![
                id,
                d.name.trim(),
                d.unit.trim(),
                d.factor,
                d.send,
                d.active,
                reset
            ],
        )?;
        if reset {
            if let Some(r) = self.latest_reading(old.meter_id)? {
                self.dp_apply(old.meter_id, &r, false)?;
            }
        }
        self.dp_set_outputs(id, &d.outputs)?;
        self.dp(id)
    }

    /// Mehrere Datenpunkte auf einmal: "on", "off", "delete", "send:every|change|daily",
    /// "out:<ausgang>:on|off"
    pub fn dp_bulk(&mut self, ids: &[i64], action: &str) -> Result<usize> {
        self.tx(|db| {
            let mut n = 0;
            for id in ids {
                n += match action {
                    "on" => db
                        .conn
                        .execute("UPDATE datapoints SET active = 1 WHERE id = ?1", [id])?,
                    "off" => db
                        .conn
                        .execute("UPDATE datapoints SET active = 0 WHERE id = ?1", [id])?,
                    "delete" => db
                        .conn
                        .execute("DELETE FROM datapoints WHERE id = ?1", [id])?,
                    a if a.starts_with("out:") => {
                        let (key, on) =
                            match a.strip_prefix("out:").and_then(|x| x.rsplit_once(':')) {
                                Some((k, "on")) => (k, true),
                                Some((k, "off")) => (k, false),
                                _ => bail!("action: {a}"),
                            };
                        db.dp_set_output(*id, key, on)?;
                        1
                    }
                    a => match a.strip_prefix("send:").filter(|m| SEND_MODES.contains(m)) {
                        Some(m) => db.conn.execute(
                            "UPDATE datapoints SET send = ?2 WHERE id = ?1",
                            params![id, m],
                        )?,
                        None => bail!("action: {a}"),
                    },
                };
            }
            Ok(n)
        })
    }
}
