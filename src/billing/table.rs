//! Tabelle des Exports, gemeinsam für CSV und Vorschau: Abrechnung (Anfang, Ende, Verbrauch je
//! Zähler und Wert, dazu Vorjahre) hier, Zeitreihe in series.rs. Gemeinsame Teile: Hinweise,
//! Zahlen, Quelle eines Stands, Spalten zum Zähler.

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
pub(super) struct Notes {
    texts: Vec<String>,
    kinds: Vec<&'static str>,
}

impl Notes {
    pub(super) fn add(&mut self, kind: &'static str, text: String) {
        if !self.kinds.contains(&kind) {
            self.kinds.push(kind);
            self.texts.push(text);
        }
    }

    /// Fehler, den der Zähler in einer Auslesung selbst meldet
    pub(super) fn fault(&mut self, st: &Stand) {
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
    pub(super) fn count(&mut self, n: &Notes) {
        self.notes += usize::from(!n.kinds.is_empty());
        for k in &n.kinds {
            *self.summary.entry((*k).to_string()).or_default() += 1;
        }
    }
}

/// Zahl ohne Rundungsrauschen (höchstens 6 Stellen), mit dem gewählten Dezimaltrennzeichen
pub(super) fn num(x: f64, decimal: &str) -> String {
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

pub(super) fn source(st: &Stand, lang: Lang) -> String {
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

/// Was jede Zeile braucht: Sprache, Dezimalzeichen, Projekt, Grenze der Vorschau
pub(super) struct Ctx {
    pub lang: Lang,
    pub decimal: &'static str,
    pub project: String,
    pub limit: Option<usize>,
}

impl Ctx {
    pub fn full(&self, t: &Table) -> bool {
        self.limit.is_some_and(|n| t.rows.len() >= n)
    }

    /// Spalten zum Zähler: Projekt, Linie, Name, Ort, ID, PA, Medium
    pub fn place(&self, it: &Item) -> Vec<String> {
        let m = &it.meter;
        vec![
            self.project.clone(),
            it.line.clone(),
            m.name.clone(),
            m.location.clone(),
            crate::meterlist::meter_ident(m).unwrap_or_default(),
            m.primary_address.map(|p| p.to_string()).unwrap_or_default(),
            m.medium
                .as_deref()
                .map(|x| label(x, self.lang))
                .unwrap_or_default(),
        ]
    }

    pub fn number(&self, v: Option<f64>, factor: f64) -> String {
        v.map(|v| num(v * factor, self.decimal)).unwrap_or_default()
    }
}

/// Kopf zum Zähler und Wert (für beide Formen gleich)
pub(super) fn head() -> Vec<String> {
    [
        ("Projekt", "Project"),
        ("Linie", "Line"),
        ("Name", "Name"),
        ("Ort", "Location"),
        ("ID", "ID"),
        ("PA", "PA"),
        ("Medium", "Medium"),
        ("Wert", "Value"),
        ("Einheit", "Unit"),
    ]
    .iter()
    .map(|(de, en)| tr(de, en))
    .collect()
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
        self.billing_table_for(project_id, s, from, to, limit)
    }

    /// Tabelle für einen bestimmten Zeitraum (automatisch: ein vergangener, mit den Bezeichnungen
    /// des eingestellten Zeitraums, z.B. „Verbrauch 09/2025“)
    pub fn billing_table_for(
        &self,
        project_id: i64,
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
        limit: Option<usize>,
    ) -> Result<Table> {
        let lang = crate::i18n::current();
        let decimal = match s.decimal.as_str() {
            "" if lang == Lang::De => ",",
            "," => ",",
            _ => ".",
        };
        let ctx = Ctx {
            lang,
            decimal,
            project: self.project(project_id)?.name,
            limit,
        };
        let items = self.billing_items(project_id, s)?;
        let mut t = Table {
            from: from.to_string(),
            to: to.to_string(),
            header: Vec::new(),
            rows: Vec::new(),
            total: 0,
            notes: 0,
            summary: BTreeMap::new(),
        };
        if s.form == "series" {
            self.series_rows(&mut t, &ctx, &items, s, from, to)?;
        } else {
            self.billing_rows(&mut t, &ctx, &items, s, from, to)?;
        }
        Ok(t)
    }

    /// Abrechnung: eine Zeile je Zähler und Wert mit Anfang, Ende, Verbrauch, Vorjahren, Hinweis
    fn billing_rows(
        &self,
        t: &mut Table,
        ctx: &Ctx,
        items: &[Item],
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<()> {
        let lang = ctx.lang;
        let years: Vec<(NaiveDate, NaiveDate)> = (1..=i32::from(s.compare_years))
            .map(|k| (years_back(from, k), years_back(to, k)))
            .collect();
        t.total = items.iter().map(|it| it.keys.len().max(1)).sum();
        t.header = [
            head(),
            vec![
                tr("Stand Anfang", "Reading start"),
                tr("Quelle Anfang", "Source start"),
                tr("Stand Ende", "Reading end"),
                tr("Quelle Ende", "Source end"),
                tr("Verbrauch", "Consumption"),
            ],
            years
                .iter()
                .map(|(f, e)| format!("{} {}", tr("Verbrauch", "Consumption"), s.label(*f, *e)))
                .collect(),
            vec![tr("Hinweis", "Note")],
        ]
        .concat();
        let (start_ts, end_ts) = (
            stand::after_day(from - Duration::days(1)),
            stand::after_day(to),
        );
        for it in items {
            if ctx.full(t) {
                break;
            }
            if it.keys.is_empty() {
                let mut n = Notes::default();
                n.add("never_read", tr("noch nie ausgelesen", "never read"));
                t.count(&n);
                let empty = vec![String::new(); 7 + years.len()];
                t.rows.push([ctx.place(it), empty, n.texts].concat());
                continue;
            }
            // je Zähler, für alle seine Werte gleich
            let read_in_period = self.meter_reading_count(it.meter.id, &start_ts, &end_ts)? > 0;
            let disturbed = self.disturbances_between(it.meter.id, &start_ts, &end_ts)?;
            for key in &it.keys {
                if ctx.full(t) {
                    break;
                }
                let (factor, unit) = scale_of(it, key);
                let u = self.usage(it.meter.id, key, from, to)?;
                let mut n = Notes::default();
                for sw in &u.swaps {
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
                }
                if u.start.value.is_none() || u.end.value.is_none() {
                    n.add("missing", tr("Stand fehlt", "reading missing"));
                } else if u.used.is_none() {
                    n.add(
                        "swap_gap",
                        tr("Stand beim Tausch fehlt", "reading at replacement missing"),
                    );
                }
                if u.used.is_some_and(|x| x < 0.0) {
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
                if let Some(text) = disturbed_text(&disturbed, lang) {
                    n.add("disturbed", text);
                } else if it.meter.plan_fails >= crate::db::DISTURBED_AFTER {
                    // gestört seit vor dem Festhalten der Störungen (älterer Stand)
                    n.add("disturbed", tr("zurzeit gestört", "currently disturbed"));
                }
                n.fault(&u.start);
                n.fault(&u.end);
                if u.start.unsure || u.end.unsure {
                    n.add("unsure", tr("Zeit unsicher", "time unsure"));
                }
                let mut previous = Vec::new();
                let mut gaps = Vec::new();
                for (f, e) in &years {
                    let p = self.usage(it.meter.id, key, *f, *e)?;
                    if p.used.is_none() {
                        gaps.push(s.label(*f, *e));
                    }
                    previous.push(ctx.number(p.used, factor));
                }
                if !gaps.is_empty() {
                    n.add(
                        "prev_missing",
                        format!(
                            "{}: {}",
                            tr("Vorjahr unvollständig", "previous year incomplete"),
                            gaps.join(", ")
                        ),
                    );
                }
                t.count(&n);
                t.rows.push(
                    [
                        ctx.place(it),
                        vec![
                            key_label(key, lang),
                            unit,
                            ctx.number(u.start.value, factor),
                            source(&u.start, lang),
                            ctx.number(u.end.value, factor),
                            source(&u.end, lang),
                            ctx.number(u.used, factor),
                        ],
                        previous,
                        vec![n.texts.join("; ")],
                    ]
                    .concat(),
                );
            }
        }
        Ok(())
    }
}

/// Faktor und Einheit: wie beim Datenpunkt, sonst wie gelesen
pub(super) fn scale_of(it: &Item, key: &str) -> (f64, String) {
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
