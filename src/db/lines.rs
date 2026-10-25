//! Linien: prüfen, anlegen, ändern, Adressierung umstellen (PA oder SA), löschen, Anschluss,
//! Linien eines Projekts

use super::*;

impl Db {
    pub(super) fn check_line(l: &Line) -> Result<()> {
        if l.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        check_baud([l.baud])
    }

    pub fn create_line(&mut self, gateway_id: i64, l: Line) -> Result<Line> {
        self.gateway(gateway_id)?;
        Self::check_line(&l)?;
        self.conn.execute(
            "INSERT INTO lines (gateway_id, name, connection, baud, timeout_ms, addressing, position)
             VALUES (?1, ?2, ?3, ?4, ?5, 'primary', (SELECT COALESCE(MAX(position), 0) + 1 FROM lines WHERE gateway_id = ?1))",
            params![gateway_id, l.name.trim(), l.connection.trim(), l.baud, l.timeout_ms as i64],
        )?;
        self.line(self.conn.last_insert_rowid())
    }

    pub fn line(&self, id: i64) -> Result<Line> {
        self.conn
            .query_row("SELECT * FROM lines WHERE id = ?1", [id], line_of)
            .optional()?
            .ok_or_else(|| not_found(&tr("Linie", "Line"), id))
    }

    pub fn update_line(&mut self, id: i64, l: Line) -> Result<Line> {
        Self::check_line(&l)?;
        let n = self.conn.execute(
            "UPDATE lines SET name = ?2, connection = ?3, baud = ?4, timeout_ms = ?5 WHERE id = ?1",
            params![
                id,
                l.name.trim(),
                l.connection.trim(),
                l.baud,
                l.timeout_ms as i64
            ],
        )?;
        if n == 0 {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        self.line(id)
    }

    /// Merken über SA oder PA umstellen und die Zählerliste mit umstellen, wo das eindeutig geht.
    /// Liefert die Anzahl umgestellter Zähler und die, die bleiben mussten (mit Grund).
    pub fn set_addressing(&mut self, id: i64, mode: &str) -> Result<(usize, Vec<String>)> {
        if mode != "secondary" && mode != "primary" {
            bail!("addressing: {mode}");
        }
        self.line(id)?;
        self.conn.execute(
            "UPDATE lines SET addressing = ?2 WHERE id = ?1",
            params![id, mode],
        )?;
        let meters = self.meters(id)?;
        let mut changed = 0;
        let mut kept = Vec::new();
        for m in &meters {
            let name = m.meter_id.clone().unwrap_or_else(|| m.address.clone());
            let target = if mode == "secondary" {
                match &m.secondary_address {
                    Some(sa)
                        if meters
                            .iter()
                            .filter(|x| x.secondary_address.as_ref() == Some(sa))
                            .count()
                            == 1 =>
                    {
                        Some(sa.clone())
                    }
                    Some(_) => {
                        kept.push(format!(
                            "{name}: {}",
                            tr(
                                "Sekundäradresse nicht eindeutig",
                                "secondary address not unique"
                            )
                        ));
                        None
                    }
                    None => {
                        kept.push(format!(
                            "{name}: {}",
                            tr(
                                "Sekundäradresse unbekannt, einmal auslesen",
                                "secondary address unknown, read once"
                            )
                        ));
                        None
                    }
                }
            } else {
                match m.primary_address {
                    Some(0) => {
                        kept.push(format!(
                            "{name}: {}",
                            tr(
                                "PA 0 (Werkszustand), zuerst PA ändern",
                                "PA 0 (factory default), change PA first"
                            )
                        ));
                        None
                    }
                    Some(pa)
                        if meters
                            .iter()
                            .filter(|x| x.primary_address == Some(pa))
                            .count()
                            == 1 =>
                    {
                        Some(pa.to_string())
                    }
                    Some(pa) => {
                        kept.push(format!(
                            "{name}: PA {pa} {}",
                            tr("doppelt, zuerst PA ändern", "duplicate, change PA first")
                        ));
                        None
                    }
                    None => {
                        kept.push(format!(
                            "{name}: {}",
                            tr("PA unbekannt, einmal auslesen", "PA unknown, read once")
                        ));
                        None
                    }
                }
            };
            if let Some(t) = target {
                if t != m.address {
                    self.conn.execute(
                        "UPDATE meters SET address = ?2 WHERE id = ?1",
                        params![m.id, t],
                    )?;
                    changed += 1;
                }
            }
        }
        Ok((changed, kept))
    }

    pub fn delete_line(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM lines WHERE id = ?1", [id])? == 0 {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        self.ensure_active()?;
        Ok(())
    }

    /// Verbindungsdaten der Linie beim Verbinden übernehmen
    pub fn set_line_connection(
        &mut self,
        id: i64,
        connection: &str,
        baud: u32,
        timeout_ms: u64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET connection = ?2, baud = ?3, timeout_ms = ?4 WHERE id = ?1",
            params![id, connection, baud, timeout_ms as i64],
        )?;
        Ok(())
    }

    pub fn lines_of_project(&self, project_id: i64) -> Result<Vec<Line>> {
        let mut st = self.conn.prepare(
            "SELECT lines.* FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
        )?;
        let rows = st.query_map([project_id], line_of)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
