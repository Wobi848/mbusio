//! Protokoll der Weitergabe: was an einen Ausgang ging und was verworfen wurde (Konzept Export,
//! Teil A). Was gerade wartet, steht in der Warteschlange des Ausgangs und wird beim Abfragen
//! dazugenommen, nicht doppelt gespeichert.

use super::*;

/// Aufbewahrung: älter wird gelöscht, mehr auch (älteste zuerst)
const KEEP_S: i64 = 30 * 86400;
const KEEP_ROWS: i64 = 200_000;
/// Höchstens so viele Zeilen je Abfrage (die Oberfläche filtert weiter ein)
const PAGE: usize = 500;

/// Ein Eintrag, wie ihn die Oberfläche und die CSV bekommen
#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub time: String,
    pub output: String,
    pub dp: Option<i64>,
    /// Name des Datenpunkts zum Zeitpunkt des Sendens (leer: Status, Home Assistant)
    pub name: String,
    pub address: String,
    pub value: String,
    pub quality: String,
    /// sent · dropped · waiting
    pub result: String,
    /// bei dropped: too_old · too_many
    pub reason: String,
}

/// Filter der Ansicht; leer = alle
#[derive(Debug, Default, Deserialize)]
pub struct LogFilter {
    #[serde(default)]
    pub output: String,
    pub dp: Option<i64>,
    /// nur die letzten n Tage (0 = alle 30)
    #[serde(default)]
    pub days: i64,
    /// sent · dropped · waiting
    #[serde(default)]
    pub result: String,
    /// Suche in Name und Adresse
    #[serde(default)]
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct LogPage {
    pub rows: Vec<LogRow>,
    /// Anzahl je Ergebnis im Zeitraum, ohne Filter auf das Ergebnis (für die Knöpfe)
    pub counts: BTreeMap<String, i64>,
    /// mehr Zeilen als gezeigt
    pub more: bool,
}

/// Wert und Qualität aus einer MQTT-Nachricht eines Datenpunkts (JSON mit value, unit, quality)
pub fn sample_of(payload: &str) -> (String, String) {
    let v: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
    // Zahl wie gelesen, ohne angehängtes „.0“
    let value = match (v["value"].as_f64(), v["unit"].as_str()) {
        (None, _) => String::new(),
        (Some(x), Some(u)) if !u.is_empty() => format!("{x} {u}"),
        (Some(x), _) => x.to_string(),
    };
    (value, v["quality"].as_str().unwrap_or("").to_string())
}

/// Eine Zeile ins Protokoll; `dp` mit Namen, wenn es ein Datenpunkt ist
pub struct Entry<'a> {
    pub output: &'a str,
    pub dp: Option<i64>,
    pub address: &'a str,
    pub value: &'a str,
    pub quality: &'a str,
    pub result: &'a str,
    pub reason: &'a str,
}

impl Db {
    /// Eintragen; bei vollem Speicher nicht (wie die Auslesungen)
    pub(crate) fn out_log(&mut self, e: &Entry) -> Result<()> {
        if self.readings_paused {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO out_log (utc, time, output, dp, name, address, value, quality, result, reason)
             VALUES (?1, ?2, ?3, ?4, COALESCE((SELECT name FROM datapoints WHERE id = ?4), ''), ?5, ?6, ?7, ?8, ?9)",
            params![
                crate::clock::utc_secs(),
                crate::clock::stamp(),
                e.output,
                e.dp,
                e.address,
                e.value,
                e.quality,
                e.result,
                e.reason
            ],
        )?;
        Ok(())
    }

    /// Älteres als 30 Tage und mehr als die Grenze löschen; liefert die Anzahl
    pub fn out_log_trim(&mut self) -> Result<usize> {
        let old = self.conn.execute(
            "DELETE FROM out_log WHERE utc < ?1",
            [crate::clock::utc_secs() - KEEP_S],
        )?;
        let over = self.conn.execute(
            "DELETE FROM out_log WHERE id <= (SELECT MAX(id) FROM out_log) - ?1",
            [KEEP_ROWS],
        )?;
        Ok(old + over)
    }

    /// Protokoll nach Filter, neueste zuerst; `waiting` kommt aus `waiting` (Warteschlangen der
    /// Ausgänge), damit nichts doppelt gespeichert wird
    pub fn out_log_page(&self, f: &LogFilter, waiting: Vec<LogRow>) -> Result<LogPage> {
        let since = if f.days > 0 {
            crate::clock::utc_secs() - f.days * 86400
        } else {
            0
        };
        let q = format!("%{}%", f.q.trim());
        let base =
            "FROM out_log WHERE utc >= ?1 AND (?2 = '' OR output = ?2) AND (?3 IS NULL OR dp = ?3)
                    AND (name LIKE ?4 OR address LIKE ?4)";
        let mut counts = BTreeMap::new();
        let mut st = self
            .conn
            .prepare(&format!("SELECT result, COUNT(*) {base} GROUP BY result"))?;
        for r in st.query_map(params![since, f.output, f.dp, q], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })? {
            let (k, n) = r?;
            counts.insert(k, n);
        }
        let waiting: Vec<LogRow> = waiting
            .into_iter()
            .filter(|w| {
                (f.output.is_empty() || w.output == f.output)
                    && f.dp.is_none_or(|d| w.dp == Some(d))
                    && (f.q.trim().is_empty()
                        || [&w.name, &w.address]
                            .iter()
                            .any(|x| x.to_lowercase().contains(&f.q.trim().to_lowercase())))
            })
            .collect();
        counts.insert("waiting".into(), waiting.len() as i64);
        let mut rows = Vec::new();
        if f.result.is_empty() || f.result == "waiting" {
            rows.extend(waiting.into_iter().rev());
        }
        if f.result != "waiting" {
            let mut st = self.conn.prepare(&format!(
                "SELECT time, output, dp, name, address, value, quality, result, reason {base}
                 AND (?5 = '' OR result = ?5) ORDER BY id DESC LIMIT ?6"
            ))?;
            let found = st.query_map(
                params![since, f.output, f.dp, q, f.result, (PAGE + 1) as i64],
                |r| {
                    Ok(LogRow {
                        time: r.get(0)?,
                        output: r.get(1)?,
                        dp: r.get(2)?,
                        name: r.get(3)?,
                        address: r.get(4)?,
                        value: r.get(5)?,
                        quality: r.get(6)?,
                        result: r.get(7)?,
                        reason: r.get(8)?,
                    })
                },
            )?;
            for r in found {
                rows.push(r?);
            }
        }
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        Ok(LogPage { rows, counts, more })
    }
}
