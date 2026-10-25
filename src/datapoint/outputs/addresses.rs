//! Adressen je Datenpunkt und Ausgang: vorschlagen, eindeutig halten, nach Muster neu setzen,
//! Ausgang ein- und ausschalten.

use std::collections::HashSet;

use crate::changeset::ChangeSet;

use super::*;

impl Db {
    /// Adresse eines Datenpunkts an einem Ausgang nach dem Muster (noch nicht eindeutig gemacht)
    pub(super) fn pattern_address(&self, key: &str, dp_id: i64) -> Result<String> {
        output(key)?.pattern_address(self, &self.dp_place(dp_id)?, &self.dp(dp_id)?)
    }

    /// Anderer Datenpunkt mit derselben Adresse an diesem Ausgang, über alle Projekte oder nur im
    /// Projekt (siehe `Output::global_addresses`); `enabled`: nur eingeschaltete zählen
    pub(super) fn address_owner(
        &self,
        key: &str,
        address: &str,
        dp_id: i64,
        enabled: bool,
    ) -> Result<Option<i64>> {
        // None = alle Projekte
        let project = if output(key)?.global_addresses() {
            None
        } else {
            Some(self.project_of_meter(self.dp(dp_id)?.meter_id)?)
        };
        Ok(self
            .conn
            .query_row(
                "SELECT dp_outputs.dp FROM dp_outputs JOIN datapoints ON datapoints.id = dp_outputs.dp
                   JOIN meters ON meters.id = datapoints.meter_id JOIN lines ON lines.id = meters.line_id
                   JOIN gateways ON gateways.id = lines.gateway_id
                 WHERE dp_outputs.output = ?1 AND dp_outputs.address = ?2 AND dp_outputs.dp != ?3
                   AND (?4 IS NULL OR gateways.project_id = ?4) AND (?5 = 0 OR dp_outputs.enabled = 1)
                 LIMIT 1",
                params![key, address, dp_id, project, enabled],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Eingeschaltete Ausgänge dieser Datenpunkte: keine Adresse doppelt (sonst würde bei Modbus
    /// ein Wert den anderen überschreiben, bei MQTT zwei Werte dasselbe Topic teilen)
    pub(in crate::datapoint) fn check_unique_addresses(&self, dp_ids: &[i64]) -> Result<()> {
        for &id in dp_ids {
            let d = self.dp(id)?;
            for (key, o) in d.outputs.iter().filter(|(_, o)| o.enabled) {
                let Some(a) = o.address.as_deref() else {
                    continue;
                };
                if let Some(other) = self.address_owner(key, a, id, true)? {
                    let other = self.dp(other)?;
                    anyhow::bail!(
                        "{} „{a}“ {} {} · {}",
                        output(key)?.address_label(),
                        tr("hat schon Datenpunkt", "already used by datapoint"),
                        other.no,
                        other.name
                    );
                }
            }
        }
        Ok(())
    }

    /// Vorschlag für die Adresse eines Datenpunkts an einem Ausgang, eindeutig gemacht
    pub(super) fn default_address(&self, key: &str, dp_id: i64) -> Result<String> {
        let address = self.pattern_address(key, dp_id)?;
        let taken = self.address_owner(key, &address, dp_id, false)?.is_some();
        Ok(if taken {
            format!("{address}-{}", self.dp(dp_id)?.no)
        } else {
            address
        })
    }

    /// Datenpunkte eines Projekts, die einen Ausgang haben (eingeschaltet oder mit Adresse)
    pub(super) fn dps_with_output(
        &self,
        project_id: i64,
        key: &str,
    ) -> Result<Vec<(i64, Option<String>, bool)>> {
        let mut st = self.conn.prepare(
            "SELECT dp_outputs.dp, dp_outputs.address, dp_outputs.enabled FROM dp_outputs
             JOIN datapoints ON datapoints.id = dp_outputs.dp JOIN meters ON meters.id = datapoints.meter_id
             JOIN lines ON lines.id = meters.line_id JOIN gateways ON gateways.id = lines.gateway_id
             WHERE gateways.project_id = ?1 AND dp_outputs.output = ?2 ORDER BY datapoints.no",
        )?;
        let rows = st.query_map(params![project_id, key], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Alle Adressen eines Projekts an einem Ausgang nach dem Muster neu setzen: zuerst nur zeigen,
    /// was sich ändert (`apply` = false). Doppelte werden mit der Nummer des Datenpunkts eindeutig.
    pub fn dp_readdress(&mut self, project_id: i64, key: &str, apply: bool) -> Result<ChangeSet> {
        let o = output(key)?;
        let address_label = o.address_label();
        let list = self.dps_with_output(project_id, key)?;
        let mine: HashSet<i64> = list.iter().map(|(dp, ..)| *dp).collect();
        // Adressen anderer Projekte bleiben belegt, wenn sie global eindeutig sein müssen
        let mut used: HashSet<String> = if !o.global_addresses() {
            HashSet::new()
        } else {
            let mut st = self.conn.prepare(
                "SELECT dp, address FROM dp_outputs WHERE output = ?1 AND address IS NOT NULL",
            )?;
            let rows =
                st.query_map([key], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(|r| r.ok())
                .filter(|(dp, _)| !mine.contains(dp))
                .map(|(_, a)| a)
                .collect()
        };
        let mut cs = ChangeSet::default();
        let mut updates = Vec::new();
        for (dp, old, _) in &list {
            let mut new = self.pattern_address(key, *dp)?;
            if used.contains(&new) {
                new = format!("{new}-{}", self.dp(*dp)?.no);
            }
            used.insert(new.clone());
            let d = self.dp(*dp)?;
            let item = format!("{} · {}", d.no, d.name);
            if cs.change(
                &item,
                address_label.clone(),
                old.as_deref().unwrap_or(""),
                &new,
            ) {
                updates.push((*dp, new));
            } else {
                cs.unchanged += 1;
            }
        }
        let key = key.to_string();
        cs.finish(apply, || {
            self.tx(|db| {
                for (dp, a) in &updates {
                    db.dp_set_output_address(*dp, &key, Some(a))?;
                }
                Ok(())
            })
        })
    }

    /// Ausgang für einen Datenpunkt ein- oder ausschalten; beim Einschalten eine Adresse
    /// vorschlagen (falls noch keine) und den aktuellen Wert beim nächsten Durchgang senden
    pub(in crate::datapoint) fn dp_set_output(
        &mut self,
        dp_id: i64,
        key: &str,
        on: bool,
    ) -> Result<()> {
        output(key)?;
        if !on {
            self.conn.execute(
                "UPDATE dp_outputs SET enabled = 0 WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
            )?;
            return Ok(());
        }
        let current: Option<(bool, Option<String>)> = self
            .conn
            .query_row(
                "SELECT enabled, address FROM dp_outputs WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        // schon an: nichts ändern (sonst würde „zuletzt gesendet“ vergessen)
        if let Some((true, Some(_))) = current {
            return Ok(());
        }
        let address = match current.and_then(|c| c.1) {
            Some(a) => a,
            None => self.default_address(key, dp_id)?,
        };
        self.conn.execute(
            "INSERT INTO dp_outputs (dp, output, enabled, address) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(dp, output) DO UPDATE SET enabled = 1, address = excluded.address, sent_quality = NULL",
            params![dp_id, key, address],
        )?;
        Ok(())
    }

    /// Eigene Adresse setzen (leer = Vorschlag behalten); geändert: aktuellen Wert dort neu senden
    pub(in crate::datapoint) fn dp_set_output_address(
        &mut self,
        dp_id: i64,
        key: &str,
        address: Option<&str>,
    ) -> Result<()> {
        let Some(a) = address.map(str::trim).filter(|a| !a.is_empty()) else {
            return Ok(());
        };
        output(key)?.check_address(a)?;
        self.conn.execute(
            "INSERT INTO dp_outputs (dp, output, enabled, address) VALUES (?1, ?2, 0, ?3)
             ON CONFLICT(dp, output) DO UPDATE SET address = excluded.address,
               sent_quality = CASE WHEN address IS excluded.address THEN sent_quality ELSE NULL END",
            params![dp_id, key, a],
        )?;
        Ok(())
    }

    /// Alle Ausgänge eines Datenpunkts setzen, wie sie in `outputs` stehen
    pub(in crate::datapoint) fn dp_set_outputs(
        &mut self,
        dp_id: i64,
        outputs: &Outputs,
    ) -> Result<()> {
        for (key, o) in outputs {
            self.dp_set_output_address(dp_id, key, o.address.as_deref())?;
            self.dp_set_output(dp_id, key, o.enabled)?;
        }
        Ok(())
    }
}
