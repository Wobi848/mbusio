//! Bericht per E-Mail (Konzept E-Mail 6): wann er fällig ist und was drinsteht. Die Zahlen sammelt
//! der Zeitplan (`scheduler::report`), die Auslesungen und die Weitergabe liefert die Datenbank.

use anyhow::{bail, Result};
use chrono::{Datelike, Duration, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::storage::DiskState;
use crate::warn::Open;

mod gather;

pub use gather::{OutputCount, ReadLine};

/// Häufigkeit
pub const FREQUENCIES: &[&str] = &["off", "daily", "weekly"];
/// Teile des Berichts, jeder einzeln abwählbar
pub const PARTS: &[&str] = &["reading", "outputs", "problems", "billing", "disk"];
/// nicht gesendet: so oft neu versuchen; der Zeitraum bleibt ab dem letzten gesendeten
const RETRY_MINUTES: i64 = 60;
const TIME: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportSettings {
    /// Empfänger; leer = kein Bericht
    pub to: String,
    /// leer = Vorgabe (`default_subject`)
    pub subject: String,
    /// aus `FREQUENCIES`
    pub frequency: String,
    /// Uhrzeit hh:mm
    pub time: String,
    /// Wochentag beim wöchentlichen Bericht, 0 = Montag
    pub weekday: u32,
    /// abgewählte Teile (aus `PARTS`); neue Teile sind so von selbst dabei
    pub off: Vec<String>,
}

impl Default for ReportSettings {
    fn default() -> Self {
        ReportSettings {
            to: String::new(),
            subject: String::new(),
            frequency: "daily".into(),
            time: "07:00".into(),
            weekday: 0,
            off: Vec::new(),
        }
    }
}

impl ReportSettings {
    pub fn check(&self) -> Result<()> {
        if !FREQUENCIES.contains(&self.frequency.as_str()) {
            bail!(tr("Häufigkeit unbekannt", "Unknown frequency"));
        }
        if self.at().is_none() {
            bail!(tr("Uhrzeit wie 07:00", "Time like 07:00"));
        }
        if self.weekday > 6 {
            bail!(tr("Wochentag unbekannt", "Unknown weekday"));
        }
        if let Some(k) = self.off.iter().find(|k| !PARTS.contains(&k.as_str())) {
            bail!("{}: {k}", tr("Unbekannter Teil", "Unknown part"));
        }
        crate::mail::recipients(&self.to)?;
        Ok(())
    }

    pub fn on(&self, part: &str) -> bool {
        !self.off.iter().any(|k| k == part)
    }

    fn active(&self) -> bool {
        self.frequency != "off" && !self.to.trim().is_empty()
    }

    fn at(&self) -> Option<NaiveTime> {
        NaiveTime::parse_from_str(self.time.trim(), "%H:%M").ok()
    }

    /// Letzter Termin bis jetzt
    fn slot(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        let at = self.at()?;
        let (back, step) = match self.frequency.as_str() {
            "daily" => (0, 1),
            "weekly" => (
                (now.weekday().num_days_from_monday() + 7 - self.weekday) % 7,
                7,
            ),
            _ => return None,
        };
        let slot = (now.date() - Duration::days(back.into())).and_time(at);
        Some(if slot > now {
            slot - Duration::days(step)
        } else {
            slot
        })
    }
}

pub fn default_subject() -> String {
    tr(
        "mbusio {geraet}: Bericht {datum}",
        "mbusio {geraet}: report {datum}",
    )
}

fn parse(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIME).ok()
}

fn stamp(t: NaiveDateTime) -> String {
    t.format(TIME).to_string()
}

/// Gespeicherter Zustand (`meta` `report_state`)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportState {
    /// letzter gesendeter Bericht = Beginn des nächsten Zeitraums
    last: Option<String>,
    /// letzter Versuch, der nicht geklappt hat
    failed: Option<String>,
}

impl ReportState {
    /// Fälliger Bericht: Beginn seines Zeitraums. Verpasste Termine (Gerät aus) ergeben einen
    /// Bericht, nicht mehrere. Eingeschaltet (oder noch nie einer): der Zeitraum beginnt beim
    /// letzten Termin, ohne gleich zu senden. Liefert dazu, ob der Zustand zu speichern ist.
    pub fn due(&mut self, s: &ReportSettings, now: NaiveDateTime) -> (bool, Option<NaiveDateTime>) {
        let slot = s.slot(now).filter(|_| s.active());
        let Some(slot) = slot else {
            let changed = self.last.is_some();
            *self = ReportState::default();
            return (changed, None);
        };
        let Some(last) = self.last.as_deref().and_then(parse) else {
            self.last = Some(stamp(slot));
            return (true, None);
        };
        let retry = self
            .failed
            .as_deref()
            .and_then(parse)
            .is_some_and(|t| now - t < Duration::minutes(RETRY_MINUTES));
        (false, (slot > last && !retry).then_some(last))
    }

    pub fn done(&mut self, sent: bool, now: NaiveDateTime) {
        if sent {
            self.last = Some(stamp(now));
            self.failed = None;
        } else {
            self.failed = Some(stamp(now));
        }
    }
}

/// Zahlen eines Berichts
pub struct ReportData {
    pub reading: Vec<ReadLine>,
    pub outputs: Vec<OutputCount>,
    /// offene Probleme wie die Meldungen, auch alte
    pub problems: Vec<Open>,
    /// je Projekt: Name, abgelegt, davon gesendet
    pub billing: Vec<(String, usize, usize)>,
    pub disk: DiskState,
}

/// Betreff und Text
pub fn mail(
    s: &ReportSettings,
    data: &ReportData,
    device: &str,
    since: NaiveDateTime,
    now: NaiveDateTime,
) -> (String, String) {
    let mut text = format!(
        "{} {} – {}\r\n\r\n",
        tr("Zeitraum", "Period"),
        since.format("%d.%m.%Y %H:%M"),
        now.format("%d.%m.%Y %H:%M")
    );
    if s.on("reading") {
        let lines = data.reading.iter().map(|r| {
            format!(
                "- {}: {} {} {} {}",
                r.place,
                r.read,
                tr("von", "of"),
                r.total,
                tr("Zählern gelesen", "meters read")
            )
        });
        part(&mut text, &tr("Auslesung", "Reading"), lines);
    }
    if s.on("outputs") {
        let lines = data.outputs.iter().map(|o| {
            let mut counts = vec![format!("{} {}", o.sent, tr("gesendet", "sent"))];
            if o.dropped > 0 {
                counts.push(format!("{} {}", o.dropped, tr("verworfen", "dropped")));
            }
            if o.failed > 0 {
                counts.push(format!("{} {}", o.failed, tr("nicht gesendet", "not sent")));
            }
            format!("- {}: {}", output_name(&o.output), counts.join(", "))
        });
        part(&mut text, &tr("Weitergabe", "Pass on"), lines);
    }
    if s.on("problems") {
        let title = tr("Offene Probleme", "Open problems");
        if data.problems.is_empty() {
            part(&mut text, &title, std::iter::empty());
        } else {
            crate::warn::section(&mut text, &title, &data.problems.iter().collect::<Vec<_>>());
        }
    }
    if s.on("billing") {
        let lines = data.billing.iter().map(|(project, filed, sent)| {
            format!(
                "- {project}: {filed} {}, {sent} {}",
                tr("abgelegt", "filed"),
                tr("per E-Mail gesendet", "sent by e-mail")
            )
        });
        part(&mut text, &tr("Abrechnungen", "Billing exports"), lines);
    }
    if s.on("disk") {
        part(
            &mut text,
            &tr("Speicher", "Storage"),
            std::iter::once(disk_line(&data.disk)),
        );
    }
    text.push_str(&format!(
        "{} {device}\r\n",
        tr("Gesendet von mbusio", "Sent by mbusio")
    ));
    let template = if s.subject.trim().is_empty() {
        default_subject()
    } else {
        s.subject.clone()
    };
    let date = now.format("%d.%m.%Y").to_string();
    let subject = crate::mail::fill(&template, &[("geraet", device), ("datum", &date)]);
    (subject, text)
}

/// Abschnitt mit Überschrift; ohne Zeilen „keine“
fn part(out: &mut String, title: &str, lines: impl Iterator<Item = String>) {
    out.push_str(&format!("{title}:\r\n"));
    let mut any = false;
    for l in lines {
        out.push_str(&l);
        out.push_str("\r\n");
        any = true;
    }
    if !any {
        out.push_str(&format!("- {}\r\n", tr("keine", "none")));
    }
    out.push_str("\r\n");
}

fn output_name(key: &str) -> String {
    match key {
        "mqtt" => "MQTT".into(),
        "email" => tr("E-Mail", "E-mail"),
        _ => key.into(),
    }
}

fn disk_line(d: &DiskState) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let free = format!(
        "- {} {:.1} GB ({:.0} %)",
        tr("frei", "free"),
        d.free_bytes as f64 / GB,
        d.free_pct
    );
    match d.days_left {
        Some(days) => format!(
            "{free}, {} {days} {}",
            tr("reicht noch ca.", "lasts about"),
            tr("Tage", "days")
        ),
        None => free,
    }
}

#[cfg(test)]
mod tests;
