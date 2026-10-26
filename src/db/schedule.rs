//! Ausleseplan: Pläne setzen, Zustand je Zähler, Übersicht je Linie

use super::*;

impl Db {
    pub fn set_project_plan(&mut self, id: i64, plan: &str) -> Result<()> {
        let key = Plan::parse(plan)?.key();
        if self.conn.execute(
            "UPDATE projects SET plan = ?2 WHERE id = ?1",
            params![id, key],
        )? == 0
        {
            return Err(not_found(&tr("Projekt", "Project"), id));
        }
        Ok(())
    }

    /// None = wie Projekt
    pub fn set_line_plan(&mut self, id: i64, plan: Option<&str>) -> Result<()> {
        let key = plan.map(|p| Plan::parse(p).map(|p| p.key())).transpose()?;
        if self
            .conn
            .execute("UPDATE lines SET plan = ?2 WHERE id = ?1", params![id, key])?
            == 0
        {
            return Err(not_found(&tr("Linie", "Line"), id));
        }
        Ok(())
    }

    /// None = wie Linie
    pub fn set_meter_plan(&mut self, id: i64, plan: Option<&str>) -> Result<()> {
        let key = plan.map(|p| Plan::parse(p).map(|p| p.key())).transpose()?;
        if self.conn.execute(
            "UPDATE meters SET plan = ?2 WHERE id = ?1",
            params![id, key],
        )? == 0
        {
            return Err(anyhow!(
                "{} {id}",
                tr("Zähler nicht in der Liste:", "Meter not in list:")
            ));
        }
        Ok(())
    }

    /// Vorgabe des Projekts, zu dem die Linie gehört
    pub fn project_plan_of_line(&self, line_id: i64) -> Result<String> {
        Ok(self.conn.query_row(
            "SELECT projects.plan FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             JOIN projects ON projects.id = gateways.project_id WHERE lines.id = ?1",
            [line_id],
            |r| r.get(0),
        )?)
    }

    /// Alle Linien aller Projekte mit dem gültigen Plan je Zähler (für den Zeitplan)
    pub fn planned(&self) -> Result<Vec<PlannedLine>> {
        let mut st = self.conn.prepare(
            "SELECT lines.*, projects.plan AS project_plan, gateways.name AS gateway_name
             FROM lines JOIN gateways ON gateways.id = lines.gateway_id
             JOIN projects ON projects.id = gateways.project_id ORDER BY lines.id",
        )?;
        let rows: Vec<(Line, String, String)> = st
            .query_map([], |r| {
                Ok((line_of(r)?, r.get("project_plan")?, r.get("gateway_name")?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (line, project_plan, gateway) in rows {
            let meters = self
                .meters(line.id)?
                .into_iter()
                .map(|m| {
                    let (p, _) =
                        plan::effective(m.plan.as_deref(), line.plan.as_deref(), &project_plan);
                    (m, p)
                })
                .collect();
            out.push(PlannedLine {
                label: format!("{gateway} › {}", line.name),
                line,
                meters,
            });
        }
        Ok(out)
    }

    /// Plan neu (oder Uhr weit zurück): auf diesen Termin setzen, ohne auszulesen
    pub fn plan_start(&mut self, meter_id: i64, key: &str, slot: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE meters SET plan_key = ?2, plan_slot = ?3, plan_pause = NULL, plan_fails = 0 WHERE id = ?1",
            params![meter_id, key, slot],
        )?;
        self.disturbed_sync(meter_id)
    }

    /// Plan aus: Zustand vergessen
    pub fn plan_reset(&mut self, meter_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE meters SET plan_key = NULL, plan_slot = NULL, plan_pause = NULL, plan_fails = 0 WHERE id = ?1",
            [meter_id],
        )?;
        self.disturbed_sync(meter_id)
    }

    /// Termin erledigt; liefert die Anzahl verpasster Termine nacheinander
    pub fn plan_done(&mut self, meter_id: i64, slot: &str, outcome: PlanOutcome) -> Result<i64> {
        match outcome {
            PlanOutcome::Read { ms } => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_fails = 0, plan_pause = NULL,
                   plan_ms = CASE WHEN plan_ms IS NULL THEN ?3 ELSE plan_ms * 0.7 + ?3 * 0.3 END
                 WHERE id = ?1",
                params![meter_id, slot, ms],
            )?,
            PlanOutcome::Failed => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_fails = plan_fails + 1 WHERE id = ?1",
                params![meter_id, slot],
            )?,
            PlanOutcome::Refused { until } => self.conn.execute(
                "UPDATE meters SET plan_slot = ?2, plan_pause = ?3 WHERE id = ?1",
                params![meter_id, slot, until],
            )?,
        };
        self.disturbed_sync(meter_id)?;
        self.plan_fails(meter_id)
    }

    fn plan_fails(&self, meter_id: i64) -> Result<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT plan_fails FROM meters WHERE id = ?1",
                [meter_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Störung von–bis festhalten: beginnt, wenn ein Zähler als gestört gilt, endet, sobald er
    /// wieder gelesen oder sein Plan zurückgesetzt wird
    pub(crate) fn disturbed_sync(&self, meter_id: i64) -> Result<()> {
        let disturbed = self.plan_fails(meter_id)? >= DISTURBED_AFTER;
        let open: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM disturbances WHERE meter = ?1 AND end IS NULL",
                [meter_id],
                |r| r.get(0),
            )
            .optional()?;
        let now = crate::clock::stamp();
        match (disturbed, open) {
            (true, None) => self.conn.execute(
                "INSERT INTO disturbances (meter, start) VALUES (?1, ?2)",
                params![meter_id, now],
            )?,
            (false, Some(id)) => self.conn.execute(
                "UPDATE disturbances SET end = ?2 WHERE id = ?1",
                params![id, now],
            )?,
            _ => 0,
        };
        Ok(())
    }

    /// Störungen eines Zählers, die in [from, to) fallen: (Beginn, Ende oder None = noch gestört)
    pub fn disturbances_between(
        &self,
        meter_id: i64,
        from: &str,
        to: &str,
    ) -> Result<Vec<(String, Option<String>)>> {
        Ok(self
            .conn
            .prepare(
                "SELECT start, end FROM disturbances
                 WHERE meter = ?1 AND start < ?3 AND (end IS NULL OR end >= ?2) ORDER BY start",
            )?
            .query_map(params![meter_id, from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Dauerbetrieb einer Linie ein- oder ausschalten (Konzept Bedienung §13). Ein nur mit
    /// Anschluss und wirksamem Plan für mindestens einen Zähler.
    pub fn set_operation(&mut self, line_id: i64, on: bool) -> Result<()> {
        let line = self.line(line_id)?;
        if on {
            if line.connection.is_empty() {
                bail!(tr(
                    "Zuerst den Anschluss der Linie festlegen",
                    "Set the line's connection first"
                ));
            }
            let project_plan: String = self.conn.query_row(
                "SELECT projects.plan FROM projects JOIN gateways ON gateways.project_id = projects.id
                 WHERE gateways.id = ?1",
                [line.gateway_id],
                |r| r.get(0),
            )?;
            let planned = self.meters(line_id)?.iter().any(|m| {
                plan::effective(m.plan.as_deref(), line.plan.as_deref(), &project_plan).0
                    != Plan::Off
            });
            if !planned {
                bail!(tr(
                    "Zuerst festlegen, wie oft ausgelesen wird (Ausleseplan)",
                    "First set how often to read (read plan)"
                ));
            }
        }
        self.conn.execute(
            "UPDATE lines SET operation = ?2 WHERE id = ?1",
            params![line_id, on],
        )?;
        Ok(())
    }

    /// Tests: Dauerbetrieb ohne die Prüfungen von `set_operation` einschalten
    #[cfg(test)]
    pub(crate) fn operation_on(&mut self, line_id: i64) {
        self.conn
            .execute("UPDATE lines SET operation = 1 WHERE id = ?1", [line_id])
            .unwrap();
    }

    /// Zähler auf einer Linie im Dauerbetrieb? Nur dann gehen seine Werte an die Ausgänge.
    pub(crate) fn meter_in_operation(&self, meter_id: i64) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT lines.operation FROM meters JOIN lines ON lines.id = meters.line_id
                 WHERE meters.id = ?1",
                [meter_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    /// Ergebnis des letzten Termins einer Linie: {"time", "ok", "total", "error"}
    pub fn set_plan_result(&mut self, line_id: i64, result: &Value) -> Result<()> {
        self.conn.execute(
            "UPDATE lines SET plan_result = ?2 WHERE id = ?1",
            params![line_id, result.to_string()],
        )?;
        Ok(())
    }

    /// Plan eines Zählers für die Oberfläche: gültiger Plan, woher, nächster Termin, Zustand
    pub fn meter_plan_info(
        m: &Meter,
        line_plan: Option<&str>,
        project_plan: &str,
        now: chrono::NaiveDateTime,
    ) -> Value {
        let (p, source) = plan::effective(m.plan.as_deref(), line_plan, project_plan);
        let pause = m
            .plan_pause
            .as_deref()
            .and_then(plan::parse_time)
            .filter(|t| *t > now);
        let next = p.next_slot(now).map(|n| match pause {
            Some(t) => p
                .next_slot(t - chrono::Duration::seconds(1))
                .unwrap_or(n)
                .max(n),
            None => n,
        });
        json!({
            "effective": p.key(),
            "source": source,
            "next": next.map(|t| t.format(plan::SLOT_FORMAT).to_string()),
            "paused_until": pause.map(|t| t.format(plan::SLOT_FORMAT).to_string()),
            "fails": m.plan_fails,
            "disturbed": p != Plan::Off && m.plan_fails >= DISTURBED_AFTER,
        })
    }

    /// Plan einer Linie für die Projekt-Karte: eigener bzw. geerbter Plan, nächster Termin,
    /// Ergebnis des letzten Termins, gestörte und ausgesetzte Zähler, Buslast
    pub(super) fn line_plan_info(
        &self,
        line: &Line,
        project_plan: &str,
        result: Value,
    ) -> Result<Value> {
        let now = crate::clock::now();
        let (own, source) = plan::effective(None, line.plan.as_deref(), project_plan);
        let mut planned = 0;
        let mut disturbed = 0;
        let mut paused = 0;
        let mut next: Option<String> = None;
        let mut items = Vec::new();
        for m in self.meters(line.id)? {
            let info = Self::meter_plan_info(&m, line.plan.as_deref(), project_plan, now);
            let p = Plan::parse(info["effective"].as_str().unwrap_or("off")).unwrap_or(Plan::Off);
            if p == Plan::Off {
                continue;
            }
            planned += 1;
            disturbed += info["disturbed"].as_bool().unwrap_or(false) as i64;
            paused += !info["paused_until"].is_null() as i64;
            if let Some(n) = info["next"].as_str() {
                if next.as_deref().is_none_or(|x| n < x) {
                    next = Some(n.to_string());
                }
            }
            let baud = m.baud.unwrap_or(line.baud);
            items.push((p, m.plan_ms.unwrap_or_else(|| plan::estimate_ms(baud))));
        }
        let load = plan::bus_load(items);
        Ok(json!({
            "own": line.plan,
            "effective": own.key(),
            "source": source,
            "meters": planned,
            "next": next,
            "result": result,
            "disturbed": disturbed,
            "paused": paused,
            "load": load,
            "load_warn": load > plan::LOAD_WARN,
        }))
    }
}
