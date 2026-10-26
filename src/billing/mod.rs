//! Export für die Abrechnung (Konzept Export, Teil B): Stände zum Stichtag je Zähler und Wert,
//! als Abrechnung (eine Zeile je Zähler und Wert: Anfang, Ende, Verbrauch) oder Zeitreihe. Die
//! Werte kommen aus den gespeicherten Auslesungen, also auch rückwirkend für Werte, die nie als
//! Datenpunkt gewählt waren. Einstellungen je Projekt in `meta` (`billing:<projekt>`).

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::datapoint::{key_label, key_of};
use crate::db::{main_value, Db, Meter};
use crate::i18n::tr;
use crate::mbus::master::MeterReading;

pub mod auto;
mod stand;
mod table;

pub use stand::{Source, Stand};
pub use table::csv;

/// Einstellungen eines Projekts; Vorgaben so, dass die meisten nichts ändern müssen
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// "billing" (eine Zeile je Zähler und Wert) oder "series" (Stand je Tag bzw. Monat)
    pub form: String,
    /// Zeitreihe: "day" oder "month"
    pub step: String,
    /// Hauptwert je Zähler (Energie, Volumen …)
    pub main: bool,
    /// weitere Werte (Kennungen), soweit ein Zähler sie hat
    pub keys: Vec<String>,
    /// "prev_month", "prev_year" oder "custom" (from, to als JJJJ-MM-TT)
    pub period: String,
    pub from: Option<String>,
    pub to: Option<String>,
    /// "," oder "."; leer = nach Sprache (Deutsch Komma, Englisch Punkt)
    pub decimal: String,
    /// nach Ende des Zeitraums selbst als Datei ablegen; ruht bei eigenem Zeitraum
    pub auto: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            form: "billing".into(),
            step: "month".into(),
            main: true,
            keys: Vec::new(),
            period: "prev_month".into(),
            from: None,
            to: None,
            decimal: String::new(),
            auto: false,
        }
    }
}

impl Settings {
    fn check(&self) -> Result<()> {
        if !["billing", "series"].contains(&self.form.as_str())
            || !["day", "month"].contains(&self.step.as_str())
        {
            bail!(tr("Form des Exports unbekannt", "Unknown export form"));
        }
        if !["", ",", "."].contains(&self.decimal.as_str()) {
            bail!(tr(
                "Dezimaltrennzeichen: Komma oder Punkt",
                "Decimal separator: comma or point"
            ));
        }
        Ok(())
    }

    /// Erster und letzter Tag des Zeitraums (beide eingeschlossen)
    pub fn range(&self, today: NaiveDate) -> Result<(NaiveDate, NaiveDate)> {
        let month_start = today.with_day(1).unwrap_or(today);
        Ok(match self.period.as_str() {
            "prev_month" => {
                let end = month_start - Duration::days(1);
                (end.with_day(1).unwrap_or(end), end)
            }
            "prev_year" => (
                NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap_or(today),
                NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap_or(today),
            ),
            "custom" => {
                let day = |s: &Option<String>| {
                    s.as_deref()
                        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                };
                match (day(&self.from), day(&self.to)) {
                    (Some(f), Some(t)) if f <= t => (f, t),
                    (Some(_), Some(_)) => {
                        bail!(tr("„von“ liegt nach „bis“", "“from” is after “to”"))
                    }
                    _ => bail!(tr(
                        "Zeitraum: „von“ und „bis“ angeben",
                        "Period: give “from” and “to”"
                    )),
                }
            }
            _ => bail!(tr("Zeitraum unbekannt", "Unknown period")),
        })
    }
}

/// Ein Wert, den es im Projekt gibt, zur Auswahl
#[derive(Debug, Serialize)]
pub struct Kind {
    pub key: String,
    pub label: String,
    pub unit: String,
    /// so viele Zähler haben ihn
    pub meters: usize,
    /// bei so vielen ist er der Hauptwert
    pub main: usize,
}

/// Ein Zähler mit allem, was der Export braucht
pub(crate) struct Item {
    pub line: String,
    pub meter: Meter,
    /// Kennungen in der Reihenfolge der Ausgabe (Hauptwert zuerst); leer = nie ausgelesen
    pub keys: Vec<String>,
    /// Faktor und Einheit, wo der Wert ein Datenpunkt ist (z.B. Wh → kWh)
    pub scale: BTreeMap<String, (f64, String)>,
}

impl Db {
    pub fn billing_settings(&self, project_id: i64) -> Settings {
        self.meta_json(&format!("billing:{project_id}"))
    }

    pub fn set_billing_settings(&mut self, project_id: i64, s: &Settings) -> Result<()> {
        s.check()?;
        self.project(project_id)?;
        if s.auto && !self.billing_settings(project_id).auto {
            self.billing_auto_restart(project_id)?;
        }
        self.set_meta(&format!("billing:{project_id}"), &serde_json::to_string(s)?)
    }

    /// Werte, die die Zähler des Projekts in ihrer letzten Auslesung liefern (Messwerte mit
    /// Einheit, aktuelle Werte; ohne Fabrikationsnummer, Fehlerflags …), meistgenutzte zuerst
    pub fn billing_kinds(&self, project_id: i64) -> Result<Vec<Kind>> {
        let lang = crate::i18n::current();
        let mut kinds: BTreeMap<String, Kind> = BTreeMap::new();
        for (_, m) in self.meters_with_line_label(project_id)? {
            let Some(r) = self.latest_reading(m.id)? else {
                continue;
            };
            let main = main_value(&r).map(key_of);
            let mut seen = std::collections::HashSet::new();
            for x in r
                .records
                .iter()
                .filter(|x| x.value.is_number() && x.storage == 0 && !x.unit.is_empty())
            {
                let key = key_of(x);
                if !seen.insert(key.clone()) {
                    continue;
                }
                let k = kinds.entry(key.clone()).or_insert_with(|| Kind {
                    label: key_label(&key, lang),
                    unit: x.unit.clone(),
                    key: key.clone(),
                    meters: 0,
                    main: 0,
                });
                k.meters += 1;
                k.main += usize::from(main.as_deref() == Some(key.as_str()));
            }
        }
        let mut out: Vec<Kind> = kinds.into_values().collect();
        out.sort_by(|a, b| {
            b.main
                .cmp(&a.main)
                .then(b.meters.cmp(&a.meters))
                .then(a.label.cmp(&b.label))
        });
        Ok(out)
    }

    /// Zähler des Projekts mit den Werten, die nach den Einstellungen ausgegeben werden
    pub(crate) fn billing_items(&self, project_id: i64, s: &Settings) -> Result<Vec<Item>> {
        let mut out = Vec::new();
        for (line, m) in self.meters_with_line_label(project_id)? {
            let latest = self.latest_reading(m.id)?;
            let has = |r: &MeterReading, k: &str| stand::value_of(r, k).is_some();
            let mut keys: Vec<String> = Vec::new();
            if let Some(r) = &latest {
                if s.main {
                    keys.extend(main_value(r).map(key_of));
                }
                for k in s.keys.iter().filter(|k| has(r, k)) {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
            let scale = self
                .dp_by_meter(m.id)?
                .into_iter()
                .map(|d| (d.key, (d.factor, d.unit)))
                .collect();
            out.push(Item {
                line,
                meter: m,
                keys,
                scale,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
