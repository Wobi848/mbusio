//! Auslesungen (vollständig oder kompakt), Ausdünnen, letzter Scan, Änderungen

use super::*;

pub(super) struct ReadingRow {
    data: String,
    raw: Option<Vec<u8>>,
    timestamp: String,
    utc: Option<i64>,
    target: Option<String>,
    time_unsure: bool,
}

pub(super) fn reading_row(r: &Row) -> rusqlite::Result<ReadingRow> {
    Ok(ReadingRow {
        data: r.get("data")?,
        raw: r.get("raw")?,
        timestamp: r.get("timestamp")?,
        utc: r.get("utc")?,
        target: r.get("target")?,
        time_unsure: r.get("time_unsure")?,
    })
}

impl ReadingRow {
    /// Vollständig gespeichert (JSON) oder aus den Rohtelegrammen decodiert
    fn into_reading(self) -> Option<MeterReading> {
        if !self.data.is_empty() {
            return serde_json::from_str(&self.data).ok();
        }
        MeterReading::from_raw(
            self.raw.as_deref()?,
            self.timestamp,
            self.utc,
            self.target.unwrap_or_default(),
            self.time_unsure,
        )
        .ok()
    }
}

impl Db {
    /// Auslesungen der Linie, älteste zuerst (höchstens `limit` neueste)
    pub fn readings(&self, line_id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        self.query_readings("line_id", line_id, limit)
    }

    /// Auslesungen nach Linie oder Zähler; kompakt gespeicherte werden aus den Rohtelegrammen
    /// decodiert, unlesbare übersprungen
    pub(super) fn query_readings(
        &self,
        column: &str,
        id: i64,
        limit: usize,
    ) -> Result<Vec<MeterReading>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT * FROM (SELECT id, data, raw, timestamp, utc, target, time_unsure FROM readings
               WHERE {column} = ?1 ORDER BY id DESC LIMIT ?2)
             ORDER BY id"
        ))?;
        let rows = st.query_map(params![id, limit as i64], reading_row)?;
        let mut out = Vec::new();
        for row in rows {
            if let Some(r) = row?.into_reading() {
                out.push(r);
            }
        }
        Ok(out)
    }

    pub fn reading_count(&self, line_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM readings WHERE line_id = ?1",
            [line_id],
            |r| r.get(0),
        )?)
    }

    pub fn clear_readings(&mut self, line_id: i64) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM dp_values WHERE dp IN (SELECT datapoints.id FROM datapoints
               JOIN meters ON meters.id = datapoints.meter_id WHERE meters.line_id = ?1)",
            [line_id],
        )?;
        Ok(self
            .conn
            .execute("DELETE FROM readings WHERE line_id = ?1", [line_id])?)
    }

    /// Verlauf eines Zählers: seine Auslesungen, älteste zuerst (höchstens `limit` neueste)
    pub fn meter_history(&self, meter_id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        self.query_readings("meter", meter_id, limit)
    }

    /// Neueste Auslesung eines Zählers der Liste
    pub fn latest_reading(&self, meter_id: i64) -> Result<Option<MeterReading>> {
        Ok(self.query_readings("meter", meter_id, 1)?.pop())
    }

    /// Mehrere Änderungen ganz oder gar nicht (auch bei Stromausfall)
    pub fn tx<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Verlauf ausdünnen, wo im Projekt eingeschaltet: Auslesungen älter als `thin_days` bis auf
    /// die erste je Zähler und Tag löschen. Liefert die Anzahl gelöschter Auslesungen.
    pub fn thin_readings(&mut self, now: chrono::NaiveDateTime) -> Result<usize> {
        let projects: Vec<(i64, i64)> = self
            .conn
            .prepare("SELECT id, thin_days FROM projects WHERE thin_days > 0")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut n = 0;
        for (project, days) in projects {
            let cutoff = (now - chrono::Duration::days(days))
                .format("%Y-%m-%d 00:00:00")
                .to_string();
            n += self.conn.execute(
                "DELETE FROM readings WHERE timestamp < ?2 AND meter IN (
                   SELECT meters.id FROM meters JOIN lines ON lines.id = meters.line_id
                   JOIN gateways ON gateways.id = lines.gateway_id WHERE gateways.project_id = ?1)
                 AND id NOT IN (
                   SELECT MIN(id) FROM readings WHERE timestamp < ?2 AND meter IS NOT NULL
                   GROUP BY meter, substr(timestamp, 1, 10))",
                params![project, cutoff],
            )?;
            // Werte der Datenpunkte genauso: erster Wert je Datenpunkt und Tag
            self.conn.execute(
                "DELETE FROM dp_values WHERE time < ?2 AND dp IN (
                   SELECT datapoints.id FROM datapoints JOIN meters ON meters.id = datapoints.meter_id
                   JOIN lines ON lines.id = meters.line_id JOIN gateways ON gateways.id = lines.gateway_id
                   WHERE gateways.project_id = ?1)
                 AND rowid NOT IN (SELECT MIN(rowid) FROM dp_values WHERE time < ?2 GROUP BY dp, substr(time, 1, 10))",
                params![project, cutoff],
            )?;
        }
        Ok(n)
    }

    pub fn set_thin_days(&mut self, project_id: i64, days: Option<i64>) -> Result<()> {
        let days = days.filter(|d| *d > 0);
        if self.conn.execute(
            "UPDATE projects SET thin_days = ?2 WHERE id = ?1",
            params![project_id, days],
        )? == 0
        {
            return Err(not_found(&tr("Projekt", "Project"), project_id));
        }
        Ok(())
    }

    /// Grösse der Datenbank in Byte (mit WAL)
    pub fn size(&self) -> u64 {
        let Some(p) = &self.path else { return 0 };
        let len = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        len(p) + len(&p.with_extension("db-wal"))
    }

    pub fn set_last_scan(&mut self, line_id: i64, scan: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2, scanned = ?3 WHERE id = ?1",
            params![line_id, scan.to_string(), now()],
        )?;
        Ok(())
    }

    /// Trefferliste nachführen (z.B. nach "PA ändern"), ohne den Zeitpunkt des Scans zu ändern
    pub fn replace_last_scan(&mut self, line_id: i64, scan: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2 WHERE id = ?1",
            params![line_id, scan.to_string()],
        )?;
        Ok(())
    }

    pub fn last_scan(&self, line_id: i64) -> Result<Option<Value>> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT last_scan FROM lines WHERE id = ?1",
                [line_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(match text {
            Some(t) => Some(serde_json::from_str(&t)?),
            None => None,
        })
    }

    pub fn set_issues(&mut self, line_id: i64, issues: &Value) -> Result<()> {
        if self.conn.execute(
            "UPDATE lines SET issues = ?2 WHERE id = ?1",
            params![line_id, issues.to_string()],
        )? == 0
        {
            return Err(not_found(&tr("Linie", "Line"), line_id));
        }
        Ok(())
    }

    pub fn add_change(&mut self, line_id: i64, text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO changes (line_id, time, text) VALUES (?1, ?2, ?3)",
            params![line_id, now(), text],
        )?;
        Ok(())
    }

    /// Änderungen eines Projekts, mit Name der Linie
    pub fn changes(&self, project_id: i64) -> Result<Vec<Change>> {
        let mut st = self.conn.prepare(
            "SELECT changes.time, gateways.name || ' · ' || lines.name AS line, changes.text
             FROM changes JOIN lines ON lines.id = changes.line_id JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 ORDER BY changes.id",
        )?;
        let rows = st.query_map([project_id], |r| {
            Ok(Change {
                time: r.get(0)?,
                line: r.get(1)?,
                text: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_changes(&mut self, project_id: i64) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM changes WHERE line_id IN
               (SELECT lines.id FROM lines JOIN gateways ON gateways.id = lines.gateway_id WHERE gateways.project_id = ?1)",
            [project_id],
        )?)
    }
}
