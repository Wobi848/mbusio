//! Bericht, Projekt-Export und -Import

use super::*;

impl Db {
    /// Alles für den Bericht: Projekt, Gateways, Linien, Zähler mit neuester Auslesung, Änderungen
    pub fn report(&self, project_id: i64) -> Result<Value> {
        let project = self.project(project_id)?;
        let tree = self.tree()?;
        let node = tree
            .into_iter()
            .find(|p| p.project.id == project_id)
            .ok_or_else(|| not_found(&tr("Projekt", "Project"), project_id))?;
        let mut gateways = Vec::new();
        for g in node.gateways {
            let mut lines = Vec::new();
            for l in g.lines {
                let mut meters = Vec::new();
                for m in self.meters(l.line.id)? {
                    let reading = self.latest_reading(m.id)?;
                    let datapoints = self.dp_export(m.id)?;
                    meters
                        .push(json!({ "meter": m, "reading": reading, "datapoints": datapoints }));
                }
                let templates = self.templates(l.line.id)?;
                lines.push(json!({ "line": l.line, "meters": meters, "issues": l.issues, "templates": templates }));
            }
            gateways.push(json!({ "gateway": g.gateway, "lines": lines }));
        }
        Ok(json!({
            "project": project,
            "gateways": gateways,
            "changes": self.changes(project_id)?,
        }))
    }

    /// Projekt als Datei: Struktur, Zähler, neueste Auslesung je Zähler, Änderungen
    pub fn export_project(&self, project_id: i64) -> Result<Value> {
        let mut r = self.report(project_id)?;
        r["format"] = json!("mbusio-project");
        r["version"] = json!(1);
        Ok(r)
    }

    /// Exportiertes Projekt als neues Projekt einlesen; liefert die neue Projekt-ID
    pub fn import_project(&mut self, v: &Value) -> Result<i64> {
        if v["format"] != "mbusio-project" {
            bail!(tr(
                "Datei ist kein mbusio-Projekt",
                "File is not an mbusio project"
            ));
        }
        let mut project: Project = serde_json::from_value(v["project"].clone())?;
        // gleicher Name schon vorhanden: in der Auswahl unterscheidbar machen
        let taken: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1)",
            [&project.name],
            |r| r.get(0),
        )?;
        if taken {
            project.name = format!("{} ({})", project.name, tr("importiert", "imported"));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO projects (name, customer, site, technician, order_no, remarks, created, plan, thin_days)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project.name,
                project.customer,
                project.site,
                project.technician,
                project.order_no,
                project.remarks,
                if project.created.is_empty() { now() } else { project.created.clone() },
                valid_plan(Some(&project.plan)).unwrap_or_else(default_plan),
                project.thin_days.filter(|d| *d > 0)
            ],
        )?;
        let pid = tx.last_insert_rowid();
        let mut line_ids = std::collections::HashMap::new();
        // Datenpunkte und Vorlagen nach dem Commit (brauchen die Hilfsfunktionen von Db)
        let mut dps: Vec<(i64, crate::datapoint::Datapoint)> = Vec::new();
        let mut templates: Vec<(i64, crate::datapoint::Template)> = Vec::new();
        for g in v["gateways"].as_array().into_iter().flatten() {
            let gw: Gateway = serde_json::from_value(g["gateway"].clone())?;
            tx.execute(
                "INSERT INTO gateways (project_id, name, address, note, position)
                 VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM gateways WHERE project_id = ?1))",
                params![pid, gw.name, gw.address, gw.note],
            )?;
            let gid = tx.last_insert_rowid();
            for l in g["lines"].as_array().into_iter().flatten() {
                let line: Line = serde_json::from_value(l["line"].clone())?;
                tx.execute(
                    "INSERT INTO lines (gateway_id, name, connection, baud, timeout_ms, addressing, plan, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, (SELECT COALESCE(MAX(position), 0) + 1 FROM lines WHERE gateway_id = ?1))",
                    params![gid, line.name, line.connection, line.baud, line.timeout_ms as i64, line.addressing,
                        valid_plan(line.plan.as_deref())],
                )?;
                let lid = tx.last_insert_rowid();
                line_ids.insert(format!("{} · {}", gw.name, line.name), lid);
                for t in l["templates"].as_array().into_iter().flatten() {
                    if let Some(t) = crate::datapoint::template_from_export(t) {
                        templates.push((lid, t));
                    }
                }
                for entry in l["meters"].as_array().into_iter().flatten() {
                    let m: Meter = serde_json::from_value(entry["meter"].clone())?;
                    tx.execute(
                        "INSERT INTO meters (line_id, name, location, note, address, baud, meter_id, manufacturer,
                           medium, primary_address, last_read, last_value, last_quantity, last_error, secondary_address, plan)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                        params![
                            lid, m.name, m.location, m.note, m.address, m.baud, m.meter_id, m.manufacturer,
                            m.medium, m.primary_address, m.last_read, m.last_value, m.last_quantity, m.last_error,
                            m.secondary_address, valid_plan(m.plan.as_deref())
                        ],
                    )?;
                    let mid = tx.last_insert_rowid();
                    for d in entry["datapoints"].as_array().into_iter().flatten() {
                        if let Some(d) = crate::datapoint::from_export(d) {
                            dps.push((mid, d));
                        }
                    }
                    if let Some(r) = entry.get("reading").filter(|r| !r.is_null()) {
                        tx.execute(
                            "INSERT INTO readings (line_id, meter, timestamp, data) VALUES (?1, ?2, ?3, ?4)",
                            params![lid, mid, r["timestamp"].as_str().unwrap_or(""), r.to_string()],
                        )?;
                    }
                }
            }
        }
        for c in v["changes"].as_array().into_iter().flatten() {
            let c: Change = serde_json::from_value(c.clone())?;
            if let Some(lid) = line_ids.get(&c.line) {
                tx.execute(
                    "INSERT INTO changes (line_id, time, text) VALUES (?1, ?2, ?3)",
                    params![lid, c.time, c.text],
                )?;
            }
        }
        tx.commit()?;
        self.tx(|db| {
            for (mid, d) in &dps {
                db.dp_import_one(*mid, d)?;
            }
            for (lid, t) in &templates {
                db.template_import(*lid, t)?;
            }
            db.dp_fix_next(pid)
        })?;
        Ok(pid)
    }
}
