//! Eigene Simulationen in der Datenbank speichern

use super::*;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sim_scenarios (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    meters TEXT NOT NULL
);
";

impl crate::db::Db {
    pub fn sim_list(&self) -> Result<Vec<Scenario>> {
        let mut st = self
            .conn
            .prepare("SELECT key, name, meters FROM sim_scenarios ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            Ok(Scenario {
                key: r.get(0)?,
                name: r.get(1)?,
                builtin: false,
                meters: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Eigene Simulationen in die Liste für neue Verbindungen übernehmen
    pub fn sim_load(&self) -> Result<()> {
        set_user(self.sim_list()?);
        Ok(())
    }

    /// Neue Simulation, leer oder als Kopie; liefert den Schlüssel
    pub fn sim_create(&mut self, name: &str, from: Option<&str>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let meters = match from {
            Some(k) => all()
                .into_iter()
                .find(|s| s.key == k)
                .map(|s| s.meters)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{} „{k}“ {}",
                        tr("Simulation", "Simulation"),
                        tr("gibt es nicht", "does not exist")
                    )
                })?,
            None => Vec::new(),
        };
        let base = crate::mqtt::slug(name);
        let base = if base.is_empty() {
            "sim".to_string()
        } else {
            base
        };
        let mut key = base.clone();
        let mut n = 2;
        while all().iter().any(|s| s.key == key) {
            key = format!("{base}-{n}");
            n += 1;
        }
        self.conn.execute(
            "INSERT INTO sim_scenarios (key, name, meters) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, name, serde_json::to_string(&meters)?],
        )?;
        self.sim_load()?;
        Ok(key)
    }

    pub fn sim_save(&mut self, key: &str, name: &str, meters: &[SimMeter]) -> Result<()> {
        if builtins().iter().any(|s| s.key == key) {
            bail!(tr(
                "Vorlagen lassen sich nicht ändern, zuerst kopieren",
                "Templates cannot be changed, copy them first"
            ));
        }
        if name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        if meters.len() > 500 {
            bail!(tr(
                "Höchstens 500 Zähler je Simulation",
                "At most 500 meters per simulation"
            ));
        }
        for m in meters {
            m.check()?;
        }
        let n = self.conn.execute(
            "UPDATE sim_scenarios SET name = ?2, meters = ?3 WHERE key = ?1",
            rusqlite::params![key, name.trim(), serde_json::to_string(meters)?],
        )?;
        if n == 0 {
            bail!(
                "{} „{key}“ {}",
                tr("Simulation", "Simulation"),
                tr("gibt es nicht", "does not exist")
            );
        }
        self.sim_load()
    }

    /// Löschen, wenn keine Linie sie verwendet
    pub fn sim_delete(&mut self, key: &str) -> Result<()> {
        let conn = format!("SIMULATOR:{key}");
        let users: Vec<String> = self
            .conn
            .prepare("SELECT gateways.name || ' › ' || lines.name FROM lines JOIN gateways ON gateways.id = lines.gateway_id WHERE lines.connection = ?1")?
            .query_map([&conn], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !users.is_empty() {
            bail!(
                "{}: {}",
                tr("Wird noch verwendet von", "Still used by"),
                users.join(", ")
            );
        }
        self.conn
            .execute("DELETE FROM sim_scenarios WHERE key = ?1", [key])?;
        self.sim_load()
    }
}
