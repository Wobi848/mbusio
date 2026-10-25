//! Werte an die Ausgänge geben: wann (Weitergabe-Art, geänderte Qualität), was zuletzt gesendet
//! wurde, Abgleich mit Zusatznachrichten der Ausgänge.

use super::*;

/// Soll gesendet werden? Nach der Weitergabe-Art des Datenpunkts; geänderte Qualität immer
pub(in crate::datapoint) fn should_send(mode: &str, last: &Sent, s: &Sample) -> bool {
    let quality_changed = last.quality.as_deref() != Some(s.quality);
    match mode {
        "change" => last.value != Some(s.value) || quality_changed,
        "daily" => last.day.as_deref() != Some(s.day) || quality_changed,
        _ => true,
    }
}

/// Was an einen Ausgang zuletzt gesendet wurde
#[derive(Default)]
pub(in crate::datapoint) struct Sent {
    pub(in crate::datapoint) value: Option<f64>,
    pub(in crate::datapoint) quality: Option<String>,
    pub(in crate::datapoint) day: Option<String>,
}

impl Db {
    fn sent(&self, dp_id: i64, key: &str) -> Result<Sent> {
        Ok(self
            .conn
            .query_row(
                "SELECT sent_value, sent_quality, sent_day FROM dp_outputs WHERE dp = ?1 AND output = ?2",
                params![dp_id, key],
                |r| {
                    Ok(Sent {
                        value: r.get(0)?,
                        quality: r.get(1)?,
                        day: r.get(2)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    /// Wert an einen Ausgang geben und als gesendet merken
    fn send_to(&mut self, o: &dyn Output, d: &Datapoint, address: &str, s: &Sample) -> Result<()> {
        o.deliver(self, d, address, s)?;
        self.conn.execute(
            "UPDATE dp_outputs SET sent_value = ?3, sent_quality = ?4, sent_day = ?5
             WHERE dp = ?1 AND output = ?2",
            params![d.id, o.key(), s.value, s.quality, s.day],
        )?;
        Ok(())
    }

    /// Eingeschaltete Ausgänge eines Datenpunkts mit ihrer Adresse (nur Ausgänge, die als Ganzes
    /// an sind)
    fn active_outputs<'a>(&self, d: &'a Datapoint) -> Vec<(&'static dyn Output, &'a str)> {
        if !d.active {
            return Vec::new();
        }
        OUTPUTS
            .iter()
            .filter_map(|o| {
                let x = d.outputs.get(o.key())?;
                let address = x.address.as_deref().filter(|_| x.enabled)?;
                o.is_on(self).then_some((*o, address))
            })
            .collect()
    }

    /// Neuer Wert eines Datenpunkts: an alle eingeschalteten Ausgänge, wie eingestellt
    pub(in crate::datapoint) fn dispatch(&mut self, d: &Datapoint, s: &Sample) -> Result<()> {
        for (o, address) in self.active_outputs(d) {
            if should_send(&d.send, &self.sent(d.id, o.key())?, s) {
                self.send_to(o, d, address, s)?;
            }
        }
        Ok(())
    }

    /// Geänderte Qualität melden (Zähler antwortet nicht mehr, Wert veraltet), gerade
    /// eingeschaltete Datenpunkte zum ersten Mal senden und die Zusatznachrichten der Ausgänge
    /// nachführen. Liefert die Anzahl Nachrichten.
    pub fn outputs_refresh(&mut self) -> Result<usize> {
        // Qualität kann sich mit der Zeit ändern (veraltet), Einstellungen ebenso
        self.notify();
        let on: Vec<&dyn Output> = OUTPUTS.iter().copied().filter(|o| o.is_on(self)).collect();
        if on.is_empty() {
            return Ok(0);
        }
        let mut n = 0;
        for o in on {
            n += o.sync(self)?;
        }
        for v in self.dp_list(None, None)? {
            // nur Linien im Dauerbetrieb melden ihre Qualität
            if v["operation"] != true {
                continue;
            }
            let quality = v["quality"].as_str().unwrap_or("none").to_string();
            let Some(value) = v["last_value"].as_f64() else {
                continue;
            };
            if quality == "none" {
                continue;
            }
            let mut d: Datapoint = serde_json::from_value(v.clone())?;
            d.id = v["id"].as_i64().unwrap_or(0);
            let unsure: bool = self.conn.query_row(
                "SELECT last_unsure FROM datapoints WHERE id = ?1",
                [d.id],
                |r| r.get(0),
            )?;
            let day = v["last_time"]
                .as_str()
                .unwrap_or("")
                .get(..10)
                .unwrap_or("")
                .to_string();
            let s = Sample {
                value,
                quality: &quality,
                utc: v["last_utc"]
                    .as_i64()
                    .unwrap_or_else(crate::clock::utc_secs),
                unsure,
                day: &day,
            };
            for (o, address) in self.active_outputs(&d) {
                if self.sent(d.id, o.key())?.quality.as_deref() != Some(quality.as_str()) {
                    self.send_to(o, &d, address, &s)?;
                    n += 1;
                }
            }
        }
        Ok(n)
    }
}
