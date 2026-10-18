//! Schema der Datenbank und Migrationen (fehlende Spalten und Tabellen werden ergänzt)

use super::*;

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    customer TEXT NOT NULL DEFAULT '',
    site TEXT NOT NULL DEFAULT '',
    technician TEXT NOT NULL DEFAULT '',
    order_no TEXT NOT NULL DEFAULT '',
    remarks TEXT NOT NULL DEFAULT '',
    created TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS gateways (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    address TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS lines (
    id INTEGER PRIMARY KEY,
    gateway_id INTEGER NOT NULL REFERENCES gateways(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    connection TEXT NOT NULL DEFAULT '',
    baud INTEGER NOT NULL DEFAULT 2400,
    timeout_ms INTEGER NOT NULL DEFAULT 0,
    last_scan TEXT,
    scanned TEXT
);
CREATE TABLE IF NOT EXISTS meters (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL,
    baud INTEGER,
    meter_id TEXT,
    manufacturer TEXT,
    medium TEXT,
    primary_address INTEGER,
    last_read TEXT,
    last_value TEXT,
    last_quantity TEXT,
    last_error TEXT
);
CREATE TABLE IF NOT EXISTS readings (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    meter INTEGER REFERENCES meters(id) ON DELETE SET NULL,
    timestamp TEXT NOT NULL,
    data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS readings_line ON readings(line_id, id);
CREATE TABLE IF NOT EXISTS changes (
    id INTEGER PRIMARY KEY,
    line_id INTEGER NOT NULL REFERENCES lines(id) ON DELETE CASCADE,
    time TEXT NOT NULL,
    text TEXT NOT NULL
);
";

impl Db {
    pub(super) fn init(&mut self) -> Result<()> {
        self.conn.execute_batch(
            // WAL mit synchronous=NORMAL: nach Stromausfall bleibt die Datenbank intakt
            // (höchstens die letzten Sekunden fehlen), viel weniger Schreibzugriffe auf die SD-Karte
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
        )?;
        self.conn.execute_batch(SCHEMA)?;
        self.migrate()?;
        self.ensure_active()?;
        self.mqtt_enabled = self.mqtt_config().enabled;
        // eigene Simulationen für Verbindungen mit SIMULATOR:<name>
        if self.path.is_some() {
            self.sim_load()?;
        }
        Ok(())
    }

    /// Spalten, die nach der ersten Version dazugekommen sind
    pub(super) fn migrate(&mut self) -> Result<()> {
        let has = |conn: &Connection, table: &str, col: &str| -> Result<bool> {
            let mut st = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let names = st.query_map([], |r| r.get::<_, String>(1))?;
            for n in names {
                if n? == col {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        if !has(&self.conn, "lines", "issues")? {
            // Auffälligkeiten des letzten Scans: {"count": n, "items": [...]}
            self.conn
                .execute_batch("ALTER TABLE lines ADD COLUMN issues TEXT")?;
        }
        if !has(&self.conn, "lines", "scans")? {
            // letzter vollständiger Scan je Art, zum Vergleich: {"primary": {time, scope, hits}, …}
            self.conn
                .execute_batch("ALTER TABLE lines ADD COLUMN scans TEXT")?;
        }
        if !has(&self.conn, "lines", "addressing")? {
            self.conn.execute_batch(
                "ALTER TABLE lines ADD COLUMN addressing TEXT NOT NULL DEFAULT 'secondary'",
            )?;
        }
        for table in ["gateways", "lines"] {
            if !has(&self.conn, table, "position")? {
                // Reihenfolge zum Sortieren; bisher galt die Reihenfolge des Anlegens
                self.conn.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
                     UPDATE {table} SET position = id;"
                ))?;
            }
        }
        if !has(&self.conn, "meters", "secondary_address")? {
            // aus der Adresse selbst oder der letzten Auslesung nachtragen
            self.conn.execute_batch(
                "ALTER TABLE meters ADD COLUMN secondary_address TEXT;
                 UPDATE meters SET secondary_address = address
                   WHERE length(address) = 16 AND address NOT LIKE '%F%';
                 UPDATE meters SET secondary_address = (
                     SELECT json_extract(data, '$.header.secondary_address') FROM readings
                     WHERE readings.meter = meters.id ORDER BY id DESC LIMIT 1)
                   WHERE secondary_address IS NULL;",
            )?;
        }
        if !has(&self.conn, "readings", "raw")? {
            // kompakte Speicherung: Rohtelegramme statt JSON (data dann leer), Zeit auch in UTC
            self.conn.execute_batch(
                "ALTER TABLE readings ADD COLUMN raw BLOB;
                 ALTER TABLE readings ADD COLUMN utc INTEGER;
                 ALTER TABLE readings ADD COLUMN target TEXT;
                 ALTER TABLE readings ADD COLUMN time_unsure INTEGER NOT NULL DEFAULT 0;
                 CREATE INDEX IF NOT EXISTS readings_meter ON readings(meter, id);",
            )?;
        }
        if !has(&self.conn, "projects", "thin_days")? {
            // Verlauf ausdünnen: nach so vielen Tagen nur noch ein Wert pro Tag (NULL = alles behalten)
            self.conn
                .execute_batch("ALTER TABLE projects ADD COLUMN thin_days INTEGER")?;
        }
        if !has(&self.conn, "projects", "dp_next")? {
            // Datenpunkte (Phase 2): stabile Nummern je Projekt, Vorlagen einmal je neuem Zähler
            self.conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN dp_next INTEGER NOT NULL DEFAULT 1;
                 ALTER TABLE meters ADD COLUMN templated INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        self.conn.execute_batch(crate::datapoint::SCHEMA)?;
        self.conn.execute_batch(crate::mqtt::SCHEMA)?;
        self.conn.execute_batch(crate::sim::SCHEMA)?;
        if !has(&self.conn, "dp_templates", "outputs")? {
            // Ausgänge, an die neue Datenpunkte einer Vorlage gleich gehen (JSON-Liste)
            self.conn.execute_batch(
                "ALTER TABLE dp_templates ADD COLUMN outputs TEXT NOT NULL DEFAULT '[]'",
            )?;
        }
        if has(&self.conn, "dp_templates", "mqtt")? {
            // Zwischenstand vor den allgemeinen Ausgängen: MQTT-Häkchen übernehmen
            self.conn.execute_batch(
                "UPDATE dp_templates SET outputs = '[\"mqtt\"]' WHERE mqtt = 1;
                 ALTER TABLE dp_templates DROP COLUMN mqtt;",
            )?;
        }
        if !has(&self.conn, "datapoints", "last_unsure")? {
            // letzter Wert mit unsicherer Uhrzeit gelesen (für die Weitergabe)
            self.conn.execute_batch(
                "ALTER TABLE datapoints ADD COLUMN last_unsure INTEGER NOT NULL DEFAULT 0",
            )?;
        }
        if has(&self.conn, "datapoints", "mqtt")? {
            // Zwischenstand vor den allgemeinen Ausgängen: MQTT-Spalten nach dp_outputs
            self.conn.execute_batch(
                "INSERT OR IGNORE INTO dp_outputs (dp, output, enabled, address, sent_value, sent_quality, sent_day)
                   SELECT id, 'mqtt', mqtt, mqtt_topic, mqtt_value, mqtt_quality, mqtt_day FROM datapoints
                   WHERE mqtt = 1 OR mqtt_topic IS NOT NULL;
                 ALTER TABLE datapoints DROP COLUMN mqtt;
                 ALTER TABLE datapoints DROP COLUMN mqtt_topic;
                 ALTER TABLE datapoints DROP COLUMN mqtt_value;
                 ALTER TABLE datapoints DROP COLUMN mqtt_quality;
                 ALTER TABLE datapoints DROP COLUMN mqtt_day;",
            )?;
        }
        if !has(&self.conn, "projects", "plan")? {
            // Ausleseplan (Dauerbetrieb); Standard überall aus
            self.conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN plan TEXT NOT NULL DEFAULT 'off';
                 ALTER TABLE lines ADD COLUMN plan TEXT;
                 ALTER TABLE lines ADD COLUMN plan_result TEXT;
                 ALTER TABLE meters ADD COLUMN plan TEXT;
                 ALTER TABLE meters ADD COLUMN plan_key TEXT;
                 ALTER TABLE meters ADD COLUMN plan_slot TEXT;
                 ALTER TABLE meters ADD COLUMN plan_pause TEXT;
                 ALTER TABLE meters ADD COLUMN plan_fails INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE meters ADD COLUMN plan_ms REAL;",
            )?;
        }
        // Merken über PA ist Standard (18.10.2026): Linien ohne Zähler einmalig umstellen; Linien
        // mit Zählern bleiben, sonst entstünde eine gemischte Liste
        if self.meta("addressing_pa")?.is_none() {
            self.conn.execute(
                "UPDATE lines SET addressing = 'primary'
                 WHERE NOT EXISTS (SELECT 1 FROM meters WHERE meters.line_id = lines.id)",
                [],
            )?;
            self.set_meta("addressing_pa", "1")?;
        }
        Ok(())
    }
}
