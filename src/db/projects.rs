//! Projekte, Gateways, Linien, aktive Linie und Projektbaum

use super::*;

impl Db {
    /// Neues Projekt, gleich mit einem Gateway und einer Linie, damit man sofort scannen kann
    pub fn create_project(&mut self, p: Project) -> Result<Project> {
        let name = p.name.trim();
        if name.is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        self.conn.execute(
            "INSERT INTO projects (name, customer, site, technician, order_no, remarks, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                name,
                p.customer,
                p.site,
                p.technician,
                p.order_no,
                p.remarks,
                now()
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.create_gateway(
            id,
            Gateway {
                name: "Gateway 1".into(),
                ..Default::default()
            },
        )?;
        self.project(id)
    }

    pub fn project(&self, id: i64) -> Result<Project> {
        self.conn
            .query_row("SELECT * FROM projects WHERE id = ?1", [id], project_of)
            .optional()?
            .ok_or_else(|| not_found(&tr("Projekt", "Project"), id))
    }

    pub fn update_project(&mut self, id: i64, p: Project) -> Result<Project> {
        if p.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let n = self.conn.execute(
            "UPDATE projects SET name = ?2, customer = ?3, site = ?4, technician = ?5,
             order_no = ?6, remarks = ?7 WHERE id = ?1",
            params![
                id,
                p.name.trim(),
                p.customer,
                p.site,
                p.technician,
                p.order_no,
                p.remarks
            ],
        )?;
        if n == 0 {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        self.project(id)
    }

    pub fn delete_project(&mut self, id: i64) -> Result<()> {
        if self
            .conn
            .execute("DELETE FROM projects WHERE id = ?1", [id])?
            == 0
        {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        self.ensure_active()?;
        Ok(())
    }

    /// Neues Gateway mit einer ersten Linie
    pub fn create_gateway(&mut self, project_id: i64, g: Gateway) -> Result<Gateway> {
        self.project(project_id)?;
        let name = if g.name.trim().is_empty() {
            "Gateway".to_string()
        } else {
            g.name.trim().to_string()
        };
        self.conn.execute(
            "INSERT INTO gateways (project_id, name, address, note, position)
             VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM gateways WHERE project_id = ?1))",
            params![project_id, name, g.address.trim(), g.note],
        )?;
        let id = self.conn.last_insert_rowid();
        self.create_line(
            id,
            Line {
                name: format!("{} 1", tr("Linie", "Line")),
                baud: 2400,
                ..Default::default()
            },
        )?;
        self.gateway(id)
    }

    pub fn gateway(&self, id: i64) -> Result<Gateway> {
        self.conn
            .query_row("SELECT * FROM gateways WHERE id = ?1", [id], gateway_of)
            .optional()?
            .ok_or_else(|| not_found("Gateway", id))
    }

    pub fn update_gateway(&mut self, id: i64, g: Gateway) -> Result<Gateway> {
        if g.name.trim().is_empty() {
            bail!(tr("Name fehlt", "Name missing"));
        }
        let n = self.conn.execute(
            "UPDATE gateways SET name = ?2, address = ?3, note = ?4 WHERE id = ?1",
            params![id, g.name.trim(), g.address.trim(), g.note],
        )?;
        if n == 0 {
            return Err(not_found("Gateway", id));
        }
        self.gateway(id)
    }

    /// Gateway (table "gateways", Eltern "project_id") bzw. Linie ("lines", "gateway_id") um eine
    /// Stelle nach oben (-1) oder unten (+1) schieben
    pub(super) fn move_row(&mut self, table: &str, parent: &str, id: i64, dir: i64) -> Result<()> {
        let parent_id: i64 = self
            .conn
            .query_row(
                &format!("SELECT {parent} FROM {table} WHERE id = ?1"),
                [id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| not_found(table, id))?;
        let mut st = self.conn.prepare(&format!(
            "SELECT id FROM {table} WHERE {parent} = ?1 ORDER BY position, id"
        ))?;
        let mut ids: Vec<i64> = st
            .query_map([parent_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        drop(st);
        let k = ids.iter().position(|x| *x == id).unwrap_or(0);
        let j = k as i64 + dir.signum();
        if j < 0 || j >= ids.len() as i64 {
            return Ok(());
        }
        ids.swap(k, j as usize);
        let tx = self.conn.transaction()?;
        for (pos, row) in ids.iter().enumerate() {
            tx.execute(
                &format!("UPDATE {table} SET position = ?2 WHERE id = ?1"),
                params![row, pos as i64 + 1],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_gateway(&mut self, id: i64, dir: i64) -> Result<()> {
        self.move_row("gateways", "project_id", id, dir)
    }

    pub fn move_line(&mut self, id: i64, dir: i64) -> Result<()> {
        self.move_row("lines", "gateway_id", id, dir)
    }

    pub fn delete_gateway(&mut self, id: i64) -> Result<()> {
        if self
            .conn
            .execute("DELETE FROM gateways WHERE id = ?1", [id])?
            == 0
        {
            return Err(not_found("Gateway", id));
        }
        self.ensure_active()?;
        Ok(())
    }

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

    /// Projekt leeren für einen neuen Einsatz: Zähler (mit Datenpunkten), Auslesungen, Änderungen
    /// und den letzten Scan aller Linien löschen. Projekt, Linien, Anschluss, Plan und Ausgänge
    /// bleiben. Liefert die Anzahl gelöschter Zähler.
    pub fn clear_project(&mut self, project_id: i64) -> Result<usize> {
        self.project(project_id)?;
        self.tx(|db| {
            let mut n = 0;
            for line in db.lines_of_project(project_id)? {
                db.clear_readings(line.id)?;
                n += db.clear_meters(line.id)?;
                db.conn.execute(
                    "UPDATE lines SET last_scan = NULL, issues = NULL, scans = NULL WHERE id = ?1",
                    [line.id],
                )?;
            }
            db.clear_changes(project_id)?;
            Ok(n)
        })
    }

    pub fn active_line(&self) -> Result<Line> {
        let id = self
            .meta("active_line")?
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or_else(|| anyhow!(tr("Keine Linie aktiv", "No line active")))?;
        self.line(id)
    }

    pub fn set_active(&mut self, line_id: i64) -> Result<Line> {
        let line = self.line(line_id)?;
        self.set_meta("active_line", &line_id.to_string())?;
        Ok(line)
    }

    pub fn active(&self) -> Result<Active> {
        let line = self.active_line()?;
        let gateway = self.gateway(line.gateway_id)?;
        let project = self.project(gateway.project_id)?;
        Ok(Active {
            project,
            gateway,
            line,
        })
    }

    /// Alle Projekte mit Gateways und Linien, je mit Anzahl Zähler und letzter Auslesung
    pub fn tree(&self) -> Result<Vec<ProjectNode>> {
        let mut out = Vec::new();
        let mut ps = self
            .conn
            .prepare("SELECT * FROM projects ORDER BY name COLLATE NOCASE, id")?;
        let projects: Vec<Project> = ps
            .query_map([], project_of)?
            .collect::<rusqlite::Result<_>>()?;
        for project in projects {
            let mut gs = self
                .conn
                .prepare("SELECT * FROM gateways WHERE project_id = ?1 ORDER BY position, id")?;
            let gateways: Vec<Gateway> = gs
                .query_map([project.id], gateway_of)?
                .collect::<rusqlite::Result<_>>()?;
            let mut gnodes = Vec::new();
            for gateway in gateways {
                let mut ls = self.conn.prepare(
                    "SELECT lines.*,
                       (SELECT COUNT(*) FROM meters WHERE line_id = lines.id) AS n,
                       (SELECT MAX(last_read) FROM meters WHERE line_id = lines.id) AS lr,
                       (SELECT COUNT(*) FROM datapoints JOIN meters ON meters.id = datapoints.meter_id
                         WHERE meters.line_id = lines.id) AS dp_n,
                       (SELECT COUNT(*) FROM datapoints JOIN meters ON meters.id = datapoints.meter_id
                         WHERE meters.line_id = lines.id AND datapoints.invalid IS NOT NULL) AS dp_bad
                     FROM lines WHERE gateway_id = ?1 ORDER BY position, id",
                )?;
                let mut lines: Vec<LineInfo> = ls
                    .query_map([gateway.id], |r| {
                        Ok(LineInfo {
                            line: line_of(r)?,
                            meters: r.get("n")?,
                            last_read: r.get("lr")?,
                            scanned: r.get("scanned")?,
                            issues: r
                                .get::<_, Option<String>>("issues")?
                                .and_then(|t| serde_json::from_str(&t).ok()),
                            plan_info: r
                                .get::<_, Option<String>>("plan_result")?
                                .and_then(|t| serde_json::from_str(&t).ok())
                                .unwrap_or(Value::Null),
                            datapoints: serde_json::json!({
                                "count": r.get::<_, i64>("dp_n")?,
                                "invalid": r.get::<_, i64>("dp_bad")?,
                            }),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                for l in &mut lines {
                    let result = std::mem::take(&mut l.plan_info);
                    l.plan_info = self.line_plan_info(&l.line, &project.plan, result)?;
                }
                gnodes.push(GatewayNode { gateway, lines });
            }
            out.push(ProjectNode {
                project,
                gateways: gnodes,
            });
        }
        Ok(out)
    }

    /// Alle Zähler eines Projekts mit der Bezeichnung ihrer Linie („Gateway › Linie“)
    pub fn meters_with_line_label(&self, project_id: i64) -> Result<Vec<(String, Meter)>> {
        let lines: Vec<(i64, String)> = self
            .conn
            .prepare(
                "SELECT lines.id, gateways.name || ' › ' || lines.name FROM lines
                 JOIN gateways ON gateways.id = lines.gateway_id WHERE gateways.project_id = ?1
                 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
            )?
            .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (id, label) in lines {
            for m in self.meters(id)? {
                out.push((label.clone(), m));
            }
        }
        Ok(out)
    }
}
