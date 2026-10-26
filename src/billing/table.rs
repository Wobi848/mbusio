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
    /// Zeilen mit Hinweis (fehlt, Zeit unsicher, Tausch, gesunken, Fehler …)
    pub notes: usize,
    /// Zeilen je Art des Hinweises (Konzept Export 3.6), für die Zusammenfassung über der Vorschau
    pub summary: BTreeMap<String, usize>,
}

/// Hinweise einer Zeile: Text für die CSV, Art für die Zusammenfassung
#[derive(Default)]
struct Notes {
    texts: Vec<String>,
    kinds: Vec<&'static str>,
}

impl Notes {
    fn add(&mut self, kind: &'static str, text: String) {
        if !self.kinds.contains(&kind) {
            self.kinds.push(kind);
            self.texts.push(text);
        }
    }

    /// Fehler, den der Zähler in einer Auslesung selbst meldet
    fn fault(&mut self, st: &Stand) {
        match st.fault {
            Some("power_low") => self.add("power_low", tr("Batterie schwach", "battery low")),
            Some("error_flags") => self.add(
                "fault",
                tr(
                    "meldet Fehler (Fehlerflags)",
                    "reports an error (error flags)",
                ),
            ),
            Some(_) => self.add("fault", tr("meldet Fehler", "reports an error")),
            None => {}
        }
    }
}

impl Table {
    /// Zeile mit ihren Hinweisen zählen
    fn count(&mut self, n: &Notes) {
        self.notes += usize::from(!n.kinds.is_empty());
        for k in &n.kinds {
            *self.summary.entry((*k).to_string()).or_default() += 1;
        }
    }
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
    let mut out = what;
    if st.unsure {
        out = format!("{out} · {}", tr("Zeit unsicher", "time unsure"));
    }
    // die Zeitreihe hat keine Spalte „Hinweis“: was der Zähler meldet, steht bei der Quelle
    match st.fault {
        Some("power_low") => format!("{out} · {}", tr("Batterie schwach", "battery low")),
        Some(_) => format!("{out} · {}", tr("meldet Fehler", "reports an error")),
        None => out,
    }
}

/// „gestört 05.09.2026 08:00 – 07.09.2026 09:00“, „gestört seit …“; mehrere mit Komma
fn disturbed_text(list: &[(String, Option<String>)], lang: Lang) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    let spans: Vec<String> = list
        .iter()
        .map(|(start, end)| match end {
            Some(end) => format!("{} – {}", when(start, lang), when(end, lang)),
            None => format!("{} {}", tr("seit", "since"), when(start, lang)),
        })
        .collect();
    Some(format!(
        "{} {}",
        tr("gestört", "disturbed"),
        spans.join(", ")
    ))
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
            summary: BTreeMap::new(),
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
                        let mut n = Notes::default();
                        if st.value.is_none() {
                            n.add("missing", String::new());
                        }
                        if st.unsure {
                            n.add("unsure", String::new());
                        }
                        n.fault(&st);
                        t.count(&n);
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
                    let mut n = Notes::default();
                    n.add("never_read", String::new());
                    t.count(&n);
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
        let (start_ts, end_ts) = (stand::after_day(before), stand::after_day(to));
        for it in &items {
            if full(&t) {
                break;
            }
            if it.keys.is_empty() {
                let mut n = Notes::default();
                n.add("never_read", tr("noch nie ausgelesen", "never read"));
                t.count(&n);
                t.rows
                    .push([place(it), vec![String::new(); 7], n.texts].concat());
                continue;
            }
            // je Zähler, für alle seine Werte gleich
            let read_in_period = self.meter_reading_count(it.meter.id, &start_ts, &end_ts)? > 0;
            for key in &it.keys {
                if full(&t) {
                    break;
                }
                let (factor, unit) = scale_of(it, key);
                let start = self.stand_at(it.meter.id, key, before, Some(to))?;
                let end = self.stand_at(it.meter.id, key, to, None)?;
                let swaps = self.exchanges_between(it.meter.id, key, &start_ts, &end_ts)?;
                let mut n = Notes::default();
                let mut used = match (start.value, end.value) {
                    (Some(a), Some(b)) => Some(b - a),
                    _ => None,
                };
                for sw in &swaps {
                    if sw.confirmed {
                        n.add(
                            "swap",
                            format!(
                                "{} {}",
                                tr("Zählertausch", "meter replaced"),
                                when(&sw.time, lang)
                            ),
                        );
                    } else {
                        n.add(
                            "swap_open",
                            format!(
                                "{} {} ({})",
                                tr("Zählertausch erkannt", "meter replacement detected"),
                                when(&sw.time, lang),
                                tr("nicht bestätigt", "not confirmed")
                            ),
                        );
                    }
                    used = match (used, sw.old_end, sw.new_start) {
                        (Some(u), Some(o), Some(new)) => Some(u + o - new),
                        _ => None,
                    };
                }
                if start.value.is_none() || end.value.is_none() {
                    n.add("missing", tr("Stand fehlt", "reading missing"));
                } else if used.is_none() {
                    n.add(
                        "swap_gap",
                        tr("Stand beim Tausch fehlt", "reading at replacement missing"),
                    );
                }
                if used.is_some_and(|u| u < 0.0) {
                    n.add(
                        "decreased",
                        tr("Zählerstand gesunken", "meter reading decreased"),
                    );
                }
                if !read_in_period {
                    n.add(
                        "not_read",
                        tr("im Zeitraum nicht gelesen", "not read in the period"),
                    );
                }
                if it.meter.last_error.is_some() {
                    n.add(
                        "no_answer",
                        tr("antwortet zurzeit nicht", "currently not answering"),
                    );
                }
                let disturbed = self.disturbances_between(it.meter.id, &start_ts, &end_ts)?;
                if let Some(text) = disturbed_text(&disturbed, lang) {
                    n.add("disturbed", text);
                } else if it.meter.plan_fails >= crate::db::DISTURBED_AFTER {
                    // gestört seit vor dem Festhalten der Störungen (älterer Stand)
                    n.add("disturbed", tr("zurzeit gestört", "currently disturbed"));
                }
                n.fault(&start);
                n.fault(&end);
                if start.unsure || end.unsure {
                    n.add("unsure", tr("Zeit unsicher", "time unsure"));
                }
                t.count(&n);
                let num_of =
                    |v: Option<f64>| v.map(|v| num(v * factor, decimal)).unwrap_or_default();
                t.rows.push(
                    [
                        place(it),
                        vec![
                            key_label(key, lang),
                            unit,
                            num_of(start.value),
                            source(&start, lang),
                            num_of(end.value),
                            source(&end, lang),
                            num_of(used),
                            n.texts.join("; "),
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
