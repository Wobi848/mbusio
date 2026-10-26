//! Zähler einer Linie und Speichern von Auslesungen

use super::*;
use crate::i18n::both;

impl Db {
    pub fn meters(&self, line_id: i64) -> Result<Vec<Meter>> {
        let mut st = self
            .conn
            .prepare("SELECT * FROM meters WHERE line_id = ?1 ORDER BY id")?;
        let rows = st.query_map([line_id], meter_of)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn meter(&self, id: i64) -> Result<Meter> {
        self.conn
            .query_row("SELECT * FROM meters WHERE id = ?1", [id], meter_of)
            .optional()?
            .ok_or_else(|| {
                anyhow!(
                    "{} {id}",
                    tr("Zähler nicht in der Liste:", "Meter not in list:")
                )
            })
    }

    pub(super) fn check_meter(&self, line_id: i64, m: &Meter, skip_id: i64) -> Result<()> {
        let target = Target::parse(&m.address)?;
        if self
            .meters(line_id)?
            .iter()
            .any(|x| x.id != skip_id && listed(x, &target))
        {
            bail!(
                "{}: {}",
                tr(
                    "Zähler mit dieser Adresse ist schon in der Liste",
                    "A meter with this address is already in the list"
                ),
                m.address
            );
        }
        check_baud(m.baud)
    }

    pub fn add_meter(&mut self, line_id: i64, mut m: Meter) -> Result<Meter> {
        m.address = m.address.trim().to_ascii_uppercase();
        self.check_meter(line_id, &m, 0)?;
        let id = insert_meter(&self.conn, line_id, &m)?;
        self.meter(id)
    }

    /// Ändert die vom Benutzer gepflegten Felder, Messinfos bleiben.
    pub fn update_meter(&mut self, id: i64, mut new: Meter) -> Result<Meter> {
        let old = self.meter(id)?;
        new.address = new.address.trim().to_ascii_uppercase();
        self.check_meter(old.line_id, &new, id)?;
        let error = if old.address != new.address {
            None
        } else {
            old.last_error
        };
        self.conn.execute(
            "UPDATE meters SET name = ?2, location = ?3, note = ?4, address = ?5, baud = ?6, last_error = ?7
             WHERE id = ?1",
            params![id, new.name, new.location, new.note, new.address, new.baud, error],
        )?;
        self.meter(id)
    }

    /// Zählerliste nach einer Änderung am Zähler nachführen (Primäradresse gesetzt, Baudrate
    /// umgestellt), damit jede Oberfläche gleich arbeitet: über die PA gemerkt → neue Adresse;
    /// eigene Baudrate → die neue, oder keine, wenn sie der Baudrate der Linie entspricht.
    /// Liefert false, wenn der Zähler nicht in der Liste steht.
    pub fn follow_meter_change(
        &mut self,
        line_id: i64,
        target: &Target,
        new_pa: Option<u8>,
        new_baud: Option<u32>,
    ) -> Result<bool> {
        let Some(mut m) = self.find_meter(line_id, target)? else {
            return Ok(false);
        };
        if let Some(pa) = new_pa {
            if matches!(Target::parse(&m.address), Ok(Target::Primary(_))) {
                m.address = pa.to_string();
            }
            self.conn.execute(
                "UPDATE meters SET primary_address = ?2 WHERE id = ?1",
                params![m.id, pa],
            )?;
        }
        if let Some(b) = new_baud {
            m.baud = (b != self.line(line_id)?.baud).then_some(b);
        }
        self.update_meter(m.id, m)?;
        Ok(true)
    }

    pub fn remove_meter(&mut self, id: i64) -> Result<()> {
        self.meter(id)?;
        self.conn
            .execute("DELETE FROM meters WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_meters(&mut self, line_id: i64) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM meters WHERE line_id = ?1", [line_id])?)
    }

    /// Fügt Zähler hinzu, deren Adresse noch nicht in der Liste ist. Liefert die Anzahl.
    pub fn import_meters(&mut self, line_id: i64, meters: Vec<Meter>) -> Result<usize> {
        let mut n = 0;
        for mut m in meters {
            m.address = m.address.trim().to_ascii_uppercase();
            if self.check_meter(line_id, &m, 0).is_err() {
                continue;
            }
            insert_meter(&self.conn, line_id, &m)?;
            n += 1;
        }
        Ok(n)
    }

    /// Zähler der Linie, der zur Adresse passt
    pub(super) fn find_meter(&self, line_id: i64, target: &Target) -> Result<Option<Meter>> {
        Ok(self
            .meters(line_id)?
            .into_iter()
            .find(|m| listed(m, target)))
    }

    /// Ergebnis einer Auslesung speichern: Verlauf und, falls in der Liste, beim Zähler.
    /// `compact`: nur die Rohtelegramme (geplante Auslesungen), sonst zusätzlich alles als JSON.
    /// Danach erfahren Empfänger (Node-RED) davon, auch bei einem Fehler (Qualität „gestört“).
    pub fn record(
        &mut self,
        line_id: i64,
        target: &Target,
        result: Result<&MeterReading, String>,
        compact: bool,
    ) -> Result<()> {
        let r = self.store_reading(line_id, target, result, compact);
        self.notify();
        r
    }

    /// Antwortet unter der Primäradresse ein anderer Zähler (Konzept Export 3.6)? Ein Tausch ist
    /// es nur, wenn mbusio den alten Zähler schon selbst gelesen hat; sonst stimmt die Zählerliste
    /// nicht (z.B. falsch zugeordnet importiert): dann nur eine Meldung, die neue ID gilt.
    fn swap_check(&mut self, m: &Meter, target: &Target, r: &MeterReading) -> Result<bool> {
        let (Target::Primary(_), Some(old), Some(new)) = (
            target,
            m.meter_id.as_deref(),
            r.header.as_ref().map(|h| h.id.as_str()),
        ) else {
            return Ok(false);
        };
        if old.is_empty() || new.is_empty() || old == new {
            return Ok(false);
        }
        let label = crate::datapoint::meter_label(m);
        let read_before = self
            .latest_reading(m.id)?
            .and_then(|x| x.header)
            .is_some_and(|h| h.id == old);
        let (old, new) = (old.to_string(), new.to_string());
        self.notes.push(if read_before {
            both(|| {
                format!(
                    "{label}: {} {old} → {new}",
                    tr("Zählertausch erkannt, ID", "meter replacement detected, ID")
                )
            })
        } else {
            both(|| {
                format!(
                    "{label}: {} {new} {} {old}",
                    tr("ID weicht von der Liste ab:", "ID differs from the list:"),
                    tr("statt", "instead of")
                )
            })
        });
        Ok(read_before)
    }

    fn store_reading(
        &mut self,
        line_id: i64,
        target: &Target,
        result: Result<&MeterReading, String>,
        compact: bool,
    ) -> Result<()> {
        let meter = self.find_meter(line_id, target)?;
        match result {
            Ok(r) => {
                if let Some(m) = &meter {
                    if self.swap_check(m, target, r)? {
                        // wie „Zähler getauscht“, aber erkannt: speichert die Auslesung selbst
                        self.savepoint(|db| {
                            db.exchange_core(m, &m.address, r, &r.timestamp, true)
                        })?;
                        return Ok(());
                    }
                }
                if !self.readings_paused {
                    let data = if compact {
                        String::new()
                    } else {
                        serde_json::to_string(r)?
                    };
                    self.conn.execute(
                        "INSERT INTO readings (line_id, meter, timestamp, data, raw, utc, target, time_unsure)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            line_id,
                            meter.as_ref().map(|m| m.id),
                            r.timestamp,
                            data,
                            r.raw_bytes(),
                            r.utc,
                            r.target,
                            r.time_unsure
                        ],
                    )?;
                }
                let Some(m) = meter else {
                    return Ok(());
                };
                let h = r.header.as_ref();
                let main = main_value(r);
                self.conn.execute(
                    "UPDATE meters SET last_read = ?2, last_error = NULL, primary_address = ?3,
                       meter_id = COALESCE(?4, meter_id), manufacturer = COALESCE(?5, manufacturer),
                       medium = COALESCE(?6, medium), last_value = COALESCE(?7, last_value),
                       last_quantity = COALESCE(?8, last_quantity),
                       secondary_address = COALESCE(?9, secondary_address), clock_off = ?10,
                       fault = ?11
                     WHERE id = ?1",
                    params![
                        m.id,
                        r.timestamp,
                        r.primary_address,
                        h.map(|h| h.id.clone()),
                        h.map(|h| h.manufacturer.clone()).filter(|s| !s.is_empty()),
                        h.map(|h| h.medium.clone()),
                        main.map(|x| format!("{} {}", x.value_text, x.unit).trim().to_string()),
                        main.map(|x| x.quantity.clone()),
                        h.map(|h| h.secondary_address.clone()),
                        clock_offset(r),
                        fault_of(r)
                    ],
                )?;
                // Datenpunkte nachführen (auch wenn der Speicher voll ist, dann ohne Verlauf)
                let notes = self.dp_apply(m.id, r, !self.readings_paused)?;
                self.notes.extend(notes);
                self.dp_auto_templates(m.id)?;
            }
            Err(e) => {
                if let Some(m) = meter {
                    self.conn.execute(
                        "UPDATE meters SET last_error = ?2 WHERE id = ?1",
                        params![m.id, e],
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Zähler einfügen; auch für den Projekt-Import (gleiche Spalten an einer Stelle)
pub(super) fn insert_meter(conn: &rusqlite::Connection, line_id: i64, m: &Meter) -> Result<i64> {
    conn.execute(
        "INSERT INTO meters (line_id, name, location, note, address, baud, meter_id, manufacturer,
           medium, primary_address, last_read, last_value, last_quantity, last_error, secondary_address, plan, clock_off)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            line_id,
            m.name,
            m.location,
            m.note,
            m.address,
            m.baud,
            m.meter_id,
            m.manufacturer,
            m.medium,
            m.primary_address,
            m.last_read,
            m.last_value,
            m.last_quantity,
            m.last_error,
            m.secondary_address,
            valid_plan(m.plan.as_deref()),
            m.clock_off
        ],
    )?;
    Ok(conn.last_insert_rowid())
}
