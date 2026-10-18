//! Vorlagen: gleiche Auswahl für alle Zähler eines Mediums auf einer Linie

use super::*;

impl Db {
    /// Vorlagen anwenden, die für neue Zähler gelten (einmal je Zähler, nach seiner ersten
    /// Auslesung, wenn das Medium bekannt ist)
    pub(crate) fn dp_auto_templates(&mut self, meter_id: i64) -> Result<usize> {
        let meter = self.meter(meter_id)?;
        let done: bool = self.conn.query_row(
            "SELECT templated FROM meters WHERE id = ?1",
            [meter_id],
            |r| r.get(0),
        )?;
        if done || meter.medium.is_none() {
            return Ok(0);
        }
        self.conn
            .execute("UPDATE meters SET templated = 1 WHERE id = ?1", [meter_id])?;
        let mut n = 0;
        for t in self.templates(meter.line_id)? {
            if t.auto && Some(&t.medium) == meter.medium.as_ref() {
                n += self.apply_template_to(&meter, &t, i18n::current())?;
            }
        }
        Ok(n)
    }

    pub(super) fn apply_template_to(
        &mut self,
        meter: &Meter,
        t: &Template,
        lang: Lang,
    ) -> Result<usize> {
        // nur Werte, die der Zähler auch liefert
        let Some(r) = self.latest_reading(meter.id)? else {
            return Ok(0);
        };
        let mut n = 0;
        for k in &t.keys {
            if r.records
                .iter()
                .any(|x| key_of(x) == *k && x.value.is_number())
            {
                if let Some(id) = self.dp_create(meter, k, lang)? {
                    for o in &t.outputs {
                        self.dp_set_output(id, o, true)?;
                    }
                    n += 1;
                }
            }
        }
        Ok(n)
    }

    pub fn templates(&self, line_id: i64) -> Result<Vec<Template>> {
        let mut st = self
            .conn
            .prepare("SELECT id, line_id, medium, keys, auto, outputs FROM dp_templates WHERE line_id = ?1 ORDER BY id")?;
        let rows = st.query_map([line_id], |r| {
            Ok(Template {
                id: r.get(0)?,
                line_id: r.get(1)?,
                medium: r.get(2)?,
                keys: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                auto: r.get(4)?,
                outputs: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Vorlage für ein Medium der Linie speichern und auf alle passenden Zähler anwenden.
    /// Liefert die Anzahl neuer Datenpunkte.
    pub fn template_save(
        &mut self,
        line_id: i64,
        medium: &str,
        keys: &[String],
        auto: bool,
        outputs: &[String],
        lang: Lang,
    ) -> Result<usize> {
        self.tx(|db| {
            db.conn.execute(
                "INSERT INTO dp_templates (line_id, medium, keys, auto, outputs) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(line_id, medium) DO UPDATE SET keys = excluded.keys, auto = excluded.auto, outputs = excluded.outputs",
                params![line_id, medium, serde_json::to_string(keys)?, auto, serde_json::to_string(outputs)?],
            )?;
            let t = Template {
                id: 0,
                line_id,
                medium: medium.into(),
                keys: keys.to_vec(),
                auto,
                outputs: outputs.to_vec(),
            };
            let mut n = 0;
            for m in db.meters(line_id)? {
                if m.medium.as_deref() == Some(medium) {
                    n += db.apply_template_to(&m, &t, lang)?;
                }
            }
            Ok(n)
        })
    }

    pub fn template_delete(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM dp_templates WHERE id = ?1", [id])?;
        Ok(())
    }

    pub(crate) fn template_import(&mut self, line_id: i64, t: &Template) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO dp_templates (line_id, medium, keys, auto, outputs) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![line_id, t.medium, serde_json::to_string(&t.keys)?, t.auto, serde_json::to_string(&t.outputs)?],
        )?;
        Ok(())
    }
}
