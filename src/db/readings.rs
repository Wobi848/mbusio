//! Auslesungen (vollständig oder kompakt), letzter Scan, Änderungen

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
        self.decoded(
            &format!(
                "SELECT * FROM (SELECT id, data, raw, timestamp, utc, target, time_unsure FROM readings
                   WHERE {column} = ?1 ORDER BY id DESC LIMIT ?2)
                 ORDER BY id"
            ),
            id,
            limit,
        )
    }

    /// Zeilen aus `sql` (Parameter ?1 = id, ?2 = limit) als Auslesungen, unlesbare übersprungen
    fn decoded(&self, sql: &str, id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        let mut st = self.conn.prepare(sql)?;
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

    /// Verlauf eines Zählers über den ganzen Zeitraum, älteste zuerst: alle Auslesungen; sind es
    /// mehr als `limit`, die letzte je Tag (bei Zählerständen genügt der Tageswert für Diagramm und
    /// CSV, und der Anfang des Verlaufs fällt nicht weg), höchstens `limit` Tage
    pub fn meter_history(&self, meter_id: i64, limit: usize) -> Result<Vec<MeterReading>> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM readings WHERE meter = ?1",
            [meter_id],
            |r| r.get(0),
        )?;
        if n as usize <= limit {
            return self.query_readings("meter", meter_id, limit);
        }
        self.decoded(
            "SELECT * FROM (SELECT id, data, raw, timestamp, utc, target, time_unsure FROM readings
               WHERE id IN (SELECT MAX(id) FROM readings WHERE meter = ?1 GROUP BY substr(timestamp, 1, 10))
               ORDER BY id DESC LIMIT ?2)
             ORDER BY id",
            meter_id,
            limit,
        )
    }

    /// Neueste Auslesung eines Zählers der Liste
    pub fn latest_reading(&self, meter_id: i64) -> Result<Option<MeterReading>> {
        Ok(self.query_readings("meter", meter_id, 1)?.pop())
    }

    /// Anzahl Auslesungen eines Zählers mit `from <= timestamp < to`
    pub fn meter_reading_count(&self, meter_id: i64, from: &str, to: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM readings WHERE meter = ?1 AND timestamp >= ?2 AND timestamp < ?3",
            params![meter_id, from, to],
            |r| r.get(0),
        )?)
    }

    /// Auslesungen eines Zählers vor `time` (Ortszeit wie `timestamp`), neueste zuerst
    pub fn meter_readings_before(
        &self,
        meter_id: i64,
        time: &str,
        limit: usize,
    ) -> Result<Vec<MeterReading>> {
        self.meter_readings_where(
            "timestamp < ?2 ORDER BY timestamp DESC",
            meter_id,
            time,
            limit,
        )
    }

    /// Auslesungen eines Zählers ab `time`, älteste zuerst
    pub fn meter_readings_from(
        &self,
        meter_id: i64,
        time: &str,
        limit: usize,
    ) -> Result<Vec<MeterReading>> {
        self.meter_readings_where("timestamp >= ?2 ORDER BY timestamp", meter_id, time, limit)
    }

    fn meter_readings_where(
        &self,
        cond: &str,
        meter_id: i64,
        time: &str,
        limit: usize,
    ) -> Result<Vec<MeterReading>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT id, data, raw, timestamp, utc, target, time_unsure FROM readings
             WHERE meter = ?1 AND {cond} LIMIT ?3"
        ))?;
        let rows = st.query_map(params![meter_id, time, limit as i64], reading_row)?;
        let mut out = Vec::new();
        for row in rows {
            if let Some(r) = row?.into_reading() {
                out.push(r);
            }
        }
        Ok(out)
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

    /// Wie `tx`, aber auch innerhalb einer laufenden Transaktion (geplante Auslesungen speichern
    /// schon in einer): ganz oder gar nicht
    pub(crate) fn savepoint<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("SAVEPOINT nested")?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("RELEASE nested")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self
                    .conn
                    .execute_batch("ROLLBACK TO nested; RELEASE nested");
                Err(e)
            }
        }
    }

    /// Grösse der Datenbank in Byte (mit WAL)
    pub fn size(&self) -> u64 {
        let Some(p) = &self.path else { return 0 };
        let len = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        len(p) + len(&p.with_extension("db-wal"))
    }

    /// Erwarteter Zuwachs je Tag beim heutigen Plan, nur Linien im Dauerbetrieb: je Termin eine
    /// Auslesung, je aktivem Datenpunkt ein Wert, je eingeschaltetem Ausgang ein Eintrag im
    /// Protokoll der Weitergabe (Grössen gemessen, Konzept Export „Langzeitspeicher“)
    pub fn growth_per_day(&self) -> Result<u64> {
        const READING: f64 = 370.0;
        const VALUE: f64 = 70.0;
        const LOGGED: f64 = 200.0;
        let mut st = self.conn.prepare(
            "SELECT d.meter_id, COUNT(DISTINCT d.id),
                    (SELECT COUNT(*) FROM dp_outputs o JOIN datapoints x ON x.id = o.dp
                     WHERE x.meter_id = d.meter_id AND x.active = 1 AND o.enabled = 1)
             FROM datapoints d WHERE d.active = 1 GROUP BY d.meter_id",
        )?;
        let per_meter: std::collections::HashMap<i64, (f64, f64)> = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    (r.get::<_, f64>(1)?, r.get::<_, f64>(2)?),
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut bytes = 0.0;
        for pl in self.planned()?.iter().filter(|pl| pl.line.operation) {
            for (m, plan) in &pl.meters {
                let Some(every) = plan.interval_s() else {
                    continue;
                };
                let (values, logged) = per_meter.get(&m.id).copied().unwrap_or_default();
                bytes += 86400.0 / every * (READING + values * VALUE + logged * LOGGED);
            }
        }
        Ok(bytes as u64)
    }

    /// Ergebnis eines Scans merken; ein vollständiger Scan wird zusätzlich je Art (und Bereich,
    /// `scope`) für den Vergleich mit dem nächsten Scan gemerkt
    pub fn set_last_scan(&mut self, line_id: i64, scan: &Value, complete: bool) -> Result<()> {
        let time = now();
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2, scanned = ?3 WHERE id = ?1",
            params![line_id, scan.to_string(), time],
        )?;
        if complete {
            self.set_scan_of_kind(line_id, scan, Some(&time))?;
        }
        Ok(())
    }

    /// Trefferliste nachführen (z.B. nach "PA ändern"), ohne den Zeitpunkt des Scans zu ändern;
    /// auch der gemerkte Scan dieser Art, damit gewollte Änderungen nicht als Änderung erscheinen
    pub fn replace_last_scan(&mut self, line_id: i64, scan: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET last_scan = ?2 WHERE id = ?1",
            params![line_id, scan.to_string()],
        )?;
        self.set_scan_of_kind(line_id, scan, None)
    }

    /// Letzter vollständiger Scan dieser Art mit gleichem Bereich: {time, scope, hits}
    pub fn previous_scan(&self, line_id: i64, kind: &str, scope: &str) -> Result<Option<Value>> {
        let prev = self.scans(line_id)?.remove(kind);
        Ok(prev.filter(|p| p["scope"] == scope))
    }

    fn scans(&self, line_id: i64) -> Result<serde_json::Map<String, Value>> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT scans FROM lines WHERE id = ?1", [line_id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        Ok(text
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default())
    }

    /// `time` None = Zeit des gemerkten Scans behalten (nur Treffer nachführen)
    fn set_scan_of_kind(&mut self, line_id: i64, scan: &Value, time: Option<&str>) -> Result<()> {
        let Some(kind) = scan["kind"].as_str() else {
            return Ok(());
        };
        let mut all = self.scans(line_id)?;
        let entry = match (time, all.get(kind)) {
            (Some(t), _) => json!({ "time": t, "scope": scan["scope"], "hits": scan["hits"] }),
            // nachführen nur aus einem vollständigen Scan desselben Bereichs (ein abgebrochener
            // hat nicht alle Zähler und würde den gemerkten Scan verkürzen)
            (None, Some(old)) if old["scope"] == scan["scope"] && scan["complete"] == true => {
                let mut e = old.clone();
                e["hits"] = scan["hits"].clone();
                e
            }
            _ => return Ok(()),
        };
        all.insert(kind.to_string(), entry);
        self.conn.execute(
            "UPDATE lines SET scans = ?2 WHERE id = ?1",
            params![line_id, Value::Object(all).to_string()],
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
