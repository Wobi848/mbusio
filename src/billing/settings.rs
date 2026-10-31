//! Einstellungen der Abrechnung je Projekt (Konzept Export 3.1, 3.7; Konzept E-Mail 9): Vorgaben
//! so, dass die meisten nichts ändern müssen. Zeiträume, ihre Bezeichnung und das Verschieben um
//! Jahre für den Vergleich mit Vorjahren.

use super::*;

/// Vorjahre zum Vergleich höchstens (sonst wird die Tabelle unlesbar)
pub const MAX_COMPARE: u8 = 10;
/// Betreff, wenn keiner eingestellt ist (in der Sprache der Oberfläche)
pub fn default_subject() -> String {
    tr(
        "Abrechnung {projekt} {zeitraum}",
        "Billing {projekt} {zeitraum}",
    )
}

/// Zeiträume zur Wahl; alle ausser "custom" gehen auch automatisch
const PERIODS: [&str; 5] = [
    "prev_month",
    "prev_quarter",
    "prev_half",
    "prev_year",
    "custom",
];

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
    /// einer aus `PERIODS`; "custom" mit from, to als JJJJ-MM-TT
    pub period: String,
    pub from: Option<String>,
    pub to: Option<String>,
    /// "," oder "."; leer = nach Sprache (Deutsch Komma, Englisch Punkt)
    pub decimal: String,
    /// Abrechnung: Verbrauch im selben Zeitraum so vieler Vorjahre als weitere Spalten
    pub compare_years: u8,
    /// nach Ende des Zeitraums selbst als Datei ablegen; ruht bei eigenem Zeitraum
    pub auto: bool,
    /// automatisch: auf eine Auslesung nach dem Ende höchstens so viele Tage warten
    pub wait_days: i64,
    /// nur diese Linien (leer = alle des Projekts; gelöschte zählen nicht)
    pub lines: Vec<i64>,
    /// abgelegte Dateien an diese Adressen senden (Komma oder Semikolon); leer = nur ablegen
    pub mail_to: String,
    /// Betreff mit Platzhaltern; leer = `default_subject()`
    pub mail_subject: String,
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
            compare_years: 1,
            auto: false,
            wait_days: 2,
            lines: Vec::new(),
            mail_to: String::new(),
            mail_subject: String::new(),
        }
    }
}

/// Erster Tag des Monats
fn month_start(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default()
}

/// Letzter Tag des Monats
fn month_end(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    month_start(y, m) - Duration::days(1)
}

/// Tag um `years` Jahre früher; ein Monatsende bleibt ein Monatsende (29.02. ↔ 28.02.)
pub fn years_back(d: NaiveDate, years: i32) -> NaiveDate {
    let y = d.year() - years;
    if d == month_end(d.year(), d.month()) {
        return month_end(y, d.month());
    }
    d.with_year(y).unwrap_or_else(|| month_end(y, d.month()))
}

impl Settings {
    pub(super) fn check(&self) -> Result<()> {
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
        if !PERIODS.contains(&self.period.as_str()) {
            bail!(tr("Zeitraum unbekannt", "Unknown period"));
        }
        if self.compare_years > MAX_COMPARE {
            bail!(tr("Vorjahre: höchstens 10", "Previous years: at most 10"));
        }
        if !(0..=31).contains(&self.wait_days) {
            bail!(tr(
                "Warten auf Auslesung: 0 bis 31 Tage",
                "Wait for a reading: 0 to 31 days"
            ));
        }
        crate::mail::recipients(&self.mail_to)?;
        Ok(())
    }

    /// Erster und letzter Tag des Zeitraums (beide eingeschlossen)
    pub fn range(&self, today: NaiveDate) -> Result<(NaiveDate, NaiveDate)> {
        let (y, m) = (today.year(), today.month());
        // Monate seit Jahresbeginn bis zum Anfang des laufenden Quartals bzw. Halbjahrs
        let back = |len: u32| {
            let first = (m - 1) / len * len + 1;
            let start = if first > len {
                month_start(y, first - len)
            } else {
                month_start(y - 1, first + 12 - len)
            };
            let end = month_end(start.year(), start.month() + len - 1);
            (start, end)
        };
        Ok(match self.period.as_str() {
            "prev_month" => back(1),
            "prev_quarter" => back(3),
            "prev_half" => back(6),
            "prev_year" => (month_start(y - 1, 1), month_end(y - 1, 12)),
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

    /// Kurz für Betreff und Spalten: 09/2026, Q3/2026, H2/2026, 2026; eigener Zeitraum von–bis
    pub fn label(&self, from: NaiveDate, to: NaiveDate) -> String {
        match self.period.as_str() {
            "prev_month" => from.format("%m/%Y").to_string(),
            "prev_quarter" => format!("Q{}/{}", (from.month() - 1) / 3 + 1, from.year()),
            "prev_half" => format!("H{}/{}", (from.month() - 1) / 6 + 1, from.year()),
            "prev_year" => from.year().to_string(),
            _ => {
                let f = crate::i18n::current().date_format();
                format!("{}–{}", from.format(f), to.format(f))
            }
        }
    }
}
