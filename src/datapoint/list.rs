//! Datenpunkte je Linie oder Projekt mit Qualität und Bezeichnung: Grundlage für Oberfläche,
//! CSV, Ausgänge und Node-RED.

use super::*;

impl Db {
    /// Zähler einer Linie (oder eines Projekts) mit ihren Datenpunkten, je Datenpunkt mit
    /// Qualität und Bezeichnung des Werts. Grundlage für Liste, Übersicht und CSV.
    pub(super) fn dp_by_meters(
        &self,
        line_id: Option<i64>,
        project_id: Option<i64>,
    ) -> Result<Vec<(String, Meter, Vec<Value>)>> {
        let now = crate::clock::now();
        // Linie: id, Name, Plan, Plan des Projekts, beim letzten Termin nicht erreichbar
        type LineRow = (i64, String, Option<String>, String, bool, bool);
        let lines: Vec<LineRow> = self
            .conn
            .prepare(
                "SELECT lines.id, gateways.name || ' › ' || lines.name, lines.plan, projects.plan,
                   COALESCE(json_extract(lines.plan_result, '$.offline'), 0)
                     OR COALESCE(json_extract(lines.plan_result, '$.line_down'), 0),
                   lines.operation
                 FROM lines JOIN gateways ON gateways.id = lines.gateway_id JOIN projects ON projects.id = gateways.project_id
                 WHERE (?1 IS NULL OR lines.id = ?1) AND (?2 IS NULL OR projects.id = ?2)
                 ORDER BY gateways.position, gateways.id, lines.position, lines.id",
            )?
            .query_map(params![line_id, project_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (lid, label, line_plan, project_plan, line_down, operation) in lines {
            for m in self.meters(lid)? {
                let (p, _) =
                    plan::effective(m.plan.as_deref(), line_plan.as_deref(), &project_plan);
                let mut dps = Vec::new();
                for d in self.dp_by_meter(m.id)? {
                    let mut v = serde_json::to_value(&d)?;
                    v["quality"] = json!(quality(&d, &m, p, line_down, now));
                    v["label"] = json!(key_label(&d.key, i18n::current()));
                    v["line_id"] = json!(lid);
                    v["operation"] = json!(operation);
                    v["line"] = json!(label);
                    v["meter"] = json!(meter_label(&m));
                    v["address"] = json!(m.address);
                    dps.push(v);
                }
                out.push((label.clone(), m, dps));
            }
        }
        Ok(out)
    }

    /// Alle Datenpunkte einer Linie (oder eines Projekts), nach Nummer
    pub fn dp_list(&self, line_id: Option<i64>, project_id: Option<i64>) -> Result<Vec<Value>> {
        let mut out: Vec<Value> = self
            .dp_by_meters(line_id, project_id)?
            .into_iter()
            .flat_map(|(_, _, dps)| dps)
            .collect();
        out.sort_by_key(|v| v["no"].as_i64().unwrap_or(0));
        Ok(out)
    }

    /// Für den Reiter Weitergabe: je Zähler (auch ohne Datenpunkte) seine Datenpunkte
    pub fn dp_overview(&self, line_id: Option<i64>, project_id: Option<i64>) -> Result<Vec<Value>> {
        Ok(self
            .dp_by_meters(line_id, project_id)?
            .into_iter()
            .map(|(line, m, dps)| {
                json!({
                    "id": m.id,
                    "name": meter_label(&m),
                    "location": m.location,
                    "address": m.address,
                    "meter_id": m.meter_id,
                    "medium": m.medium,
                    "last_read": m.last_read,
                    "line_id": m.line_id,
                    "line": line,
                    "datapoints": dps,
                })
            })
            .collect())
    }

    /// Ausgänge für die Oberfläche: Schlüssel, Name, wie die Adresse heisst, eingeschaltet
    pub fn outputs_info(&self) -> Vec<Value> {
        OUTPUTS
            .iter()
            .map(|o| json!({ "key": o.key(), "label": o.label(), "address_label": o.address_label(), "on": o.is_on(self) }))
            .collect()
    }
}
