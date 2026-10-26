//! MQTT in der Datenbank: Einstellungen lesen und speichern, Warteschlange (Zwischenspeicher bis
//! zur Bestätigung durch den Broker).

use anyhow::Result;
use rusqlite::params;

use super::MqttConfig;
use crate::datapoint::{sample_of, LogRow, OutLogEntry};
use crate::db::Db;

/// Warteschlange: älter wird verworfen, mehr auch (älteste zuerst)
const QUEUE_MAX_AGE_S: i64 = 7 * 86400;
const QUEUE_MAX_ROWS: i64 = 200_000;

/// `mqtt_queue.abs`: 1 = Topic vollständig (Home Assistant), 0 = relativ zum Basis-Topic;
/// `dp`: Datenpunkt der Nachricht (leer: Home Assistant), für das Protokoll der Weitergabe.
/// `mqtt_ha`: zuletzt gesendete Konfig-Nachrichten für Home Assistant (Topic → Nachricht).
/// Keine SQL-Kommentare in `CREATE TABLE`: SQLite speichert sie mit, und `DROP COLUMN` in einer
/// späteren Migration scheitert daran.
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mqtt_queue (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    payload TEXT NOT NULL,
    created INTEGER NOT NULL,
    abs INTEGER NOT NULL DEFAULT 0,
    dp INTEGER
);
CREATE TABLE IF NOT EXISTS mqtt_ha (
    topic TEXT PRIMARY KEY,
    payload TEXT NOT NULL
);
";

impl Db {
    pub fn mqtt_config(&self) -> MqttConfig {
        self.meta_json("mqtt")
    }

    pub fn set_mqtt_config(&mut self, c: &MqttConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mqtt", &serde_json::to_string(c)?)?;
        Ok(())
    }

    /// Wert eines Datenpunkts für den Broker vormerken (Topic relativ zum Basis-Topic)
    pub(crate) fn mqtt_enqueue(&mut self, dp: i64, topic: &str, payload: &str) -> Result<()> {
        self.mqtt_push(Some(dp), topic, payload, false)
    }

    /// Nachricht mit vollständigem Topic vormerken (nicht unter dem Basis-Topic, z.B. Home Assistant)
    pub(crate) fn mqtt_enqueue_abs(&mut self, topic: &str, payload: &str) -> Result<()> {
        self.mqtt_push(None, topic, payload, true)
    }

    fn mqtt_push(&mut self, dp: Option<i64>, topic: &str, payload: &str, abs: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mqtt_queue (topic, payload, created, abs, dp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![topic, payload, crate::clock::utc_secs(), abs, dp],
        )?;
        Ok(())
    }

    /// Die nächsten Nachrichten: (id, Topic, Nachricht, Topic vollständig)
    pub(crate) fn mqtt_peek(&self, n: usize) -> Result<Vec<(i64, String, String, bool)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, topic, payload, abs FROM mqtt_queue ORDER BY id LIMIT ?1")?;
        let rows = st.query_map([n as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Vom Broker bestätigt: aus der Warteschlange, ins Protokoll der Weitergabe
    pub(crate) fn mqtt_done(&mut self, id: i64, topic: &str) -> Result<()> {
        let (dp, payload): (Option<i64>, String) = self.conn.query_row(
            "SELECT dp, payload FROM mqtt_queue WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        self.mqtt_log(dp, topic, &payload, "sent", "")?;
        self.conn
            .execute("DELETE FROM mqtt_queue WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Eine MQTT-Nachricht ins Protokoll; bei Datenpunkten mit Wert und Qualität
    fn mqtt_log(
        &mut self,
        dp: Option<i64>,
        topic: &str,
        payload: &str,
        result: &str,
        reason: &str,
    ) -> Result<()> {
        let (value, quality) = if dp.is_some() {
            sample_of(payload)
        } else {
            Default::default()
        };
        self.out_log(&OutLogEntry {
            output: "mqtt",
            dp,
            name: None,
            address: topic,
            value: &value,
            quality: &quality,
            result,
            reason,
        })
    }

    /// Was gerade auf den Broker wartet, wie Zeilen des Protokolls (älteste zuerst)
    pub fn mqtt_waiting(&self) -> Result<Vec<LogRow>> {
        let cfg = self.mqtt_config();
        let mut st = self.conn.prepare(
            "SELECT q.created, q.dp, COALESCE(d.name, ''), q.topic, q.abs, q.payload
             FROM mqtt_queue q LEFT JOIN datapoints d ON d.id = q.dp ORDER BY q.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (created, dp, name, topic, abs, payload) = row?;
            let (value, quality) = if dp.is_some() {
                sample_of(&payload)
            } else {
                Default::default()
            };
            out.push(LogRow {
                time: crate::clock::stamp_of(created),
                output: "mqtt".into(),
                dp,
                name,
                address: if abs { topic } else { cfg.topic(&topic) },
                value,
                quality,
                result: "waiting".into(),
                reason: String::new(),
            });
        }
        Ok(out)
    }

    pub fn mqtt_queued(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM mqtt_queue", [], |r| r.get(0))?)
    }

    /// Warteschlange begrenzen; liefert die Anzahl verworfener Nachrichten
    /// Verworfenes kommt ins Protokoll der Weitergabe (zu alt: Broker zu lange weg; zu viele)
    pub fn mqtt_trim(&mut self) -> Result<usize> {
        let old = self.mqtt_drop(
            "created < ?1",
            crate::clock::utc_secs() - QUEUE_MAX_AGE_S,
            "too_old",
        )?;
        let over = self.mqtt_drop(
            "id <= (SELECT MAX(id) FROM mqtt_queue) - ?1",
            QUEUE_MAX_ROWS,
            "too_many",
        )?;
        Ok(old + over)
    }

    fn mqtt_drop(&mut self, cond: &str, arg: i64, reason: &str) -> Result<usize> {
        let cfg = self.mqtt_config();
        let dropped: Vec<(Option<i64>, String, bool, String)> = {
            let mut st = self.conn.prepare(&format!(
                "SELECT dp, topic, abs, payload FROM mqtt_queue WHERE {cond}"
            ))?;
            let rows = st.query_map([arg], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (dp, topic, abs, payload) in &dropped {
            let topic = if *abs {
                topic.clone()
            } else {
                cfg.topic(topic)
            };
            self.mqtt_log(*dp, &topic, payload, "dropped", reason)?;
        }
        self.conn
            .execute(&format!("DELETE FROM mqtt_queue WHERE {cond}"), [arg])?;
        Ok(dropped.len())
    }
}
