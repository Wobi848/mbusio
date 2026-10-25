//! MQTT in der Datenbank: Einstellungen lesen und speichern, Warteschlange (Zwischenspeicher bis
//! zur Bestätigung durch den Broker).

use anyhow::Result;
use rusqlite::params;

use super::MqttConfig;
use crate::db::Db;

/// Warteschlange: älter wird verworfen, mehr auch (älteste zuerst)
const QUEUE_MAX_AGE_S: i64 = 7 * 86400;
const QUEUE_MAX_ROWS: i64 = 200_000;

/// `mqtt_queue.abs`: 1 = Topic vollständig (Home Assistant), 0 = relativ zum Basis-Topic.
/// `mqtt_ha`: zuletzt gesendete Konfig-Nachrichten für Home Assistant (Topic → Nachricht).
/// Keine SQL-Kommentare in `CREATE TABLE`: SQLite speichert sie mit, und `DROP COLUMN` in einer
/// späteren Migration scheitert daran.
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mqtt_queue (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    payload TEXT NOT NULL,
    created INTEGER NOT NULL,
    abs INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS mqtt_ha (
    topic TEXT PRIMARY KEY,
    payload TEXT NOT NULL
);
";

impl Db {
    pub fn mqtt_config(&self) -> MqttConfig {
        self.meta("mqtt")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    pub fn set_mqtt_config(&mut self, c: &MqttConfig) -> Result<()> {
        c.check()?;
        self.set_meta("mqtt", &serde_json::to_string(c)?)?;
        Ok(())
    }

    /// Nachricht für den Broker vormerken (Topic relativ zum Basis-Topic)
    pub(crate) fn mqtt_enqueue(&mut self, topic: &str, payload: &str) -> Result<()> {
        self.mqtt_push(topic, payload, false)
    }

    /// Nachricht mit vollständigem Topic vormerken (nicht unter dem Basis-Topic, z.B. Home Assistant)
    pub(crate) fn mqtt_enqueue_abs(&mut self, topic: &str, payload: &str) -> Result<()> {
        self.mqtt_push(topic, payload, true)
    }

    fn mqtt_push(&mut self, topic: &str, payload: &str, abs: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mqtt_queue (topic, payload, created, abs) VALUES (?1, ?2, ?3, ?4)",
            params![topic, payload, crate::clock::utc_secs(), abs],
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

    pub(super) fn mqtt_done(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM mqtt_queue WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn mqtt_queued(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM mqtt_queue", [], |r| r.get(0))?)
    }

    /// Warteschlange begrenzen; liefert die Anzahl verworfener Nachrichten
    pub fn mqtt_trim(&mut self) -> Result<usize> {
        let old = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE created < ?1",
            [crate::clock::utc_secs() - QUEUE_MAX_AGE_S],
        )?;
        let over = self.conn.execute(
            "DELETE FROM mqtt_queue WHERE id <= (SELECT MAX(id) FROM mqtt_queue) - ?1",
            [QUEUE_MAX_ROWS],
        )?;
        Ok(old + over)
    }
}
