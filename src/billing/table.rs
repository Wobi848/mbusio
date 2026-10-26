//! Tabelle des Exports, gemeinsam für CSV und Vorschau: Abrechnung (Anfang, Ende, Verbrauch je
//! Zähler und Wert) oder Zeitreihe (Stand je Tag bzw. Monatsende).

use super::*;
use crate::i18n::{label, Lang};

#[derive(Debug, Serialize)]
pub struct Table {
    /// Zeitraum als JJJJ-MM-TT
    pub from: String,
    pub to: String,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Zeilen insgesamt (die Vorschau zeigt nur die ersten)
    pub total: usize,
    /// Zeilen mit Hinweis (fehlt, Zeit unsicher, Tausch, gesunken)
    pub notes: usize,
}

/// Zahl ohne Rundungsrauschen (höchstens 6 Stellen), mit dem gewählten Dezimaltrennzeichen
fn num(x: f64, decimal: &str) -> String {
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', decimal)
}

/// Zeitpunkt lesbar: Deutsch 30.09.2026 23:45, Englisch 2026-09-30 23:45
fn when(time: &str, lang: Lang) -> String {
    match crate::plan::parse_time(time) {
        Some(t) if lang == Lang::De => t.format("%d.%m.%Y %H:%M").to_string(),
        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        None => time.to_string(),
    }
}

fn source(st: &Stand, lang: Lang) -> String {
    let what = match st.source {
        Source::Meter => tr("Zähler (Stichtag)", "meter (due date)"),
        Source::Reading => format!("{} {}", tr("Auslesung", "reading"), when(&st.time, lang)),
        Source::After => format!(
            "{} {}",
            tr("erste Auslesung danach", "first reading after"),
            when(&st.time, lang)
        ),
        Source::Missing => tr("fehlt", "missing"),
    };
    if st.unsure {
        format!("{what} · {}", tr("Zeit unsicher", "time unsure"))
    } else {
        what
    }
}

/// Monatsenden im Zeitraum, dazu das Ende des Zeitraums
fn steps(from: NaiveDate, to: NaiveDate, step: &str) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let mut d = from;
    while d <= to {
        let next = d + Duration::days(1);
        if step == "day" || next.day() == 1 || d == to {
            out.push(d);
        }
        d = next;
    }
    out
}

impl Db {
    /// Tabelle nach den Einstellungen; `limit`: höchstens so viele Zeilen ausrechnen (Vorschau)
    pub fn billing_table(
        &self,
        project_id: i64,
        s: &Settings,
        today: NaiveDate,
        limit: Option<usize>,
    ) -> Result<Table> {
        let (from, to) = s.range(today)?;
        let lang = crate::i18n::current();
        let decimal = match s.decimal.as_str() {
            "" if lang == Lang::De => ",",
            "" => ".",
            d => d,
        };
        let project = self.project(project_id)?.name;
        let items = self.billing_items(project_id, s)?;
        let dates = steps(from, to, &s.step);
        let per = |it: &Item| it.keys.len().max(1);
        let total: usize = items
            .iter()
            .map(|it| {
                if s.form == "series" {
                    per(it) * dates.len()
                } else {
                    per(it)
                }
            })
            .sum();
        let mut t = Table {
            from: from.to_string(),
            to: to.to_string(),
            header: Vec::new(),
            rows: Vec::new(),
            total,
            notes: 0,
        };
        let full = |t: &Table| limit.is_some_and(|n| t.rows.len() >= n);
        let col = |de: &str, en: &str| tr(de, en);
        let place = |it: &Item| {
            let m = &it.meter;
            vec![
                project.clone(),
                it.line.clone(),
                m.name.clone(),
                m.location.clone(),
                crate::meterlist::meter_ident(m).unwrap_or_default(),
                m.primary_address.map(|p| p.to_string()).unwrap_or_default(),
                m.medium
                    .as_deref()
                    .map(|x| label(x, lang))
                    .unwrap_or_default(),
            ]
        };
        let head = [
            col("Projekt", "Project"),
            col("Linie", "Line"),
            col("Name", "Name"),
            col("Ort", "Location"),
            col("ID", "ID"),
            col("PA", "PA"),
            col("Medium", "Medium"),
            col("Wert", "Value"),
            col("Einheit", "Unit"),
        ];
        if s.form == "series" {
            t.header = [
                vec![col("Datum", "Date")],
                head.to_vec(),
                vec![col("Stand", "Reading"), col("Quelle", "Source")],
            ]
            .concat();
            'all: for it in &items {
                for key in &it.keys {
                    let (factor, unit) = scale_of(it, key);
                    for d in &dates {
                        if full(&t) {
                            break 'all;
                        }
                        let st = self.stand_at(it.meter.id, key, *d, None)?;
                        t.notes += usize::from(st.value.is_none() || st.unsure);
                        let row = [
                            vec![d.to_string()],
                            place(it),
                            vec![
                                key_label(key, lang),
                                unit.clone(),
                                st.value
                                    .map(|v| num(v * factor, decimal))
                                    .unwrap_or_default(),
                                source(&st, lang),
                            ],
                        ];
                        t.rows.push(row.concat());
                    }
                }
                if it.keys.is_empty() && !full(&t) {
                    t.notes += 1;
                    t.rows.push(
                        [
                            vec![String::new()],
                            place(it),
                            vec![
                                String::new(),
                                String::new(),
                                String::new(),
                                tr("noch nie ausgelesen", "never read"),
                            ],
                        ]
                        .concat(),
                    );
                }
            }
            return Ok(t);
        }
        t.header = [
            head.to_vec(),
            vec![
                col("Stand Anfang", "Reading start"),
                col("Quelle Anfang", "Source start"),
                col("Stand Ende", "Reading end"),
                col("Quelle Ende", "Source end"),
                col("Verbrauch", "Consumption"),
                col("Hinweis", "Note"),
            ],
        ]
        .concat();
        let before = from - Duration::days(1);
        for it in &items {
            if full(&t) {
                break;
            }
            if it.keys.is_empty() {
                t.notes += 1;
                t.rows.push(
                    [
                        place(it),
                        vec![String::new(); 7],
                        vec![tr("noch nie ausgelesen", "never read")],
                    ]
                    .concat(),
                );
                continue;
            }
            for key in &it.keys {
                if full(&t) {
                    break;
                }
                let (factor, unit) = scale_of(it, key);
                let start = self.stand_at(it.meter.id, key, before, Some(to))?;
                let end = self.stand_at(it.meter.id, key, to, None)?;
                let swaps = self.exchanges_between(
                    it.meter.id,
                    key,
                    &stand::after_day(before),
                    &stand::after_day(to),
                )?;
                let mut notes = Vec::new();
                let mut used = match (start.value, end.value) {
                    (Some(a), Some(b)) => Some(b - a),
                    _ => None,
                };
                for sw in &swaps {
                    notes.push(format!(
                        "{} {}",
                        tr("Zählertausch", "meter replaced"),
                        when(&sw.time, lang)
                    ));
                    used = match (used, sw.old_end, sw.new_start) {
                        (Some(u), Some(o), Some(n)) => Some(u + o - n),
                        _ => None,
                    };
                }
                if start.value.is_none() || end.value.is_none() {
                    notes.push(tr("Stand fehlt", "reading missing"));
                } else if used.is_none() {
                    notes.push(tr(
                        "Stand beim Tausch fehlt",
                        "reading at replacement missing",
                    ));
                }
                if used.is_some_and(|u| u < 0.0) {
                    notes.push(tr("Zählerstand gesunken", "meter reading decreased"));
                }
                if start.unsure || end.unsure {
                    notes.push(tr("Zeit unsicher", "time unsure"));
                }
                t.notes += usize::from(!notes.is_empty());
                let n = |v: Option<f64>| v.map(|v| num(v * factor, decimal)).unwrap_or_default();
                t.rows.push(
                    [
                        place(it),
                        vec![
                            key_label(key, lang),
                            unit,
                            n(start.value),
                            source(&start, lang),
                            n(end.value),
                            source(&end, lang),
                            n(used),
                            notes.join("; "),
                        ],
                    ]
                    .concat(),
                );
            }
        }
        Ok(t)
    }
}

/// Faktor und Einheit: wie beim Datenpunkt, sonst wie gelesen
fn scale_of(it: &Item, key: &str) -> (f64, String) {
    it.scale
        .get(key)
        .cloned()
        .unwrap_or_else(|| (1.0, crate::datapoint::key_unit(key)))
}

/// CSV für Excel aus der Tabelle (ohne BOM; das setzt der Download)
pub fn csv(t: &Table) -> String {
    let mut out = crate::export::row(&t.header);
    for r in &t.rows {
        out.push_str(&crate::export::row(r));
    }
    out
}
