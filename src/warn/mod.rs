//! Warnungen (Konzept E-Mail 5): offene Probleme mit den zuletzt gemeldeten vergleichen, neue
//! gebündelt per E-Mail und als Meldungen in der Oberfläche. Hier stehen nur die Regeln; was offen
//! ist, sammelt der Zeitplan (`scheduler::warn`), die Zählerprobleme liefert die Datenbank.

use anyhow::{bail, Result};
use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::i18n::{current, tr, Lang, Text};

mod found;

/// Arten von Problemen, jede einzeln abschaltbar
pub const KINDS: &[&str] = &[
    "disturbed",
    "fault",
    "battery",
    "swap",
    "line",
    "mqtt",
    "disk",
    "billing",
];
/// wählbare Abstände zwischen zwei Warnmails in Minuten (0 = sofort)
pub const INTERVALS: &[i64] = &[0, 15, 60, 240, 1440];
/// Wartezeit für Linie und MQTT: höchstens ein Tag
const DELAY_MAX: i64 = 1440;
/// so viele Probleme je Abschnitt stehen in einer Mail, der Rest als Anzahl
const MAIL_LIST_MAX: usize = 50;
/// fehlgeschlagene Warnmail: so oft neu versuchen, aufgeben nach einem Tag
const RETRY_MINUTES: i64 = 15;
const GIVE_UP_HOURS: i64 = 24;
/// „täglich erinnern“
const REMIND_HOURS: i64 = 24;
const TIME: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WarnSettings {
    /// Empfänger; leer = keine Warnmails (die Meldungen in der Oberfläche gibt es trotzdem)
    pub to: String,
    /// leer = Vorgabe (`default_subject`)
    pub subject: String,
    /// ausgeschaltete Arten (aus `KINDS`); neue Arten sind so von selbst an
    pub off: Vec<String>,
    /// Linie im Dauerbetrieb so lange nicht verbunden
    pub line_minutes: i64,
    /// MQTT so lange nicht verbunden
    pub mqtt_minutes: i64,
    /// höchstens eine Warnmail pro so viele Minuten (`INTERVALS`)
    pub interval_minutes: i64,
    /// was seit der letzten Warnmail behoben ist, in der nächsten nennen
    pub resolved: bool,
    /// offene Probleme täglich erneut nennen
    pub remind: bool,
}

impl Default for WarnSettings {
    fn default() -> Self {
        WarnSettings {
            to: String::new(),
            subject: String::new(),
            off: Vec::new(),
            line_minutes: 30,
            mqtt_minutes: 60,
            interval_minutes: 60,
            resolved: true,
            remind: false,
        }
    }
}

impl WarnSettings {
    pub fn check(&self) -> Result<()> {
        if let Some(k) = self.off.iter().find(|k| !KINDS.contains(&k.as_str())) {
            bail!("{}: {k}", tr("Unbekannte Art", "Unknown kind"));
        }
        if !(0..=DELAY_MAX).contains(&self.line_minutes)
            || !(0..=DELAY_MAX).contains(&self.mqtt_minutes)
        {
            bail!(tr(
                "Wartezeit: 0 bis 1440 Minuten",
                "Waiting time: 0 to 1440 minutes"
            ));
        }
        if !INTERVALS.contains(&self.interval_minutes) {
            bail!(tr("Abstand unbekannt", "Unknown interval"));
        }
        crate::mail::recipients(&self.to)?;
        Ok(())
    }

    pub fn on(&self, kind: &str) -> bool {
        !self.off.iter().any(|k| k == kind)
    }

    /// Minuten, die ein Problem dieser Art besteht, bevor es zählt
    fn delay(&self, kind: &str) -> i64 {
        match kind {
            "line" => self.line_minutes,
            "mqtt" => self.mqtt_minutes,
            _ => 0,
        }
    }
}

pub fn default_subject() -> String {
    tr(
        "mbusio {geraet}: {anzahl} neue Probleme",
        "mbusio {geraet}: {anzahl} new problems",
    )
}

/// Ein Problem, wie es gerade gefunden wurde
pub struct Problem {
    /// eindeutig je Problem, z.B. `disturbed:12`
    pub key: String,
    pub kind: &'static str,
    pub text: Text,
    /// Linie oder Projekt; leer beim ganzen Gerät
    pub place: String,
    /// seit wann (wenn bekannt), sonst ab jetzt
    pub since: Option<String>,
    /// wohin die Oberfläche springt
    pub line: Option<i64>,
    pub meter: Option<i64>,
    pub project: Option<i64>,
}

/// Ein offenes Problem, wie es gespeichert wird
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Open {
    pub key: String,
    pub kind: String,
    pub de: String,
    pub en: String,
    pub place: String,
    pub since: String,
    #[serde(default)]
    pub line: Option<i64>,
    #[serde(default)]
    pub meter: Option<i64>,
    #[serde(default)]
    pub project: Option<i64>,
    /// zuletzt per E-Mail gemeldet
    #[serde(default)]
    pub mailed: Option<String>,
    /// in der Oberfläche weggeklickt
    #[serde(default)]
    pub seen: bool,
}

impl Open {
    pub fn text(&self) -> &str {
        match current() {
            Lang::De => &self.de,
            Lang::En => &self.en,
        }
    }

    /// besteht lange genug, um zu zählen
    /// (ohne Wartezeit immer, auch wenn die Uhr zurückgesprungen ist)
    pub fn active(&self, s: &WarnSettings, now: NaiveDateTime) -> bool {
        let delay = s.delay(&self.kind);
        delay == 0 || parse(&self.since).is_none_or(|t| now - t >= Duration::minutes(delay))
    }

    fn line_text(&self) -> String {
        let since = parse(&self.since)
            .map(|t| t.format("%d.%m. %H:%M").to_string())
            .unwrap_or_default();
        let place = if self.place.is_empty() {
            String::new()
        } else {
            format!("{} · ", self.place)
        };
        format!("- {place}{} ({} {since})", self.text(), tr("seit", "since"))
    }
}

fn parse(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIME).ok()
}

fn stamp(t: NaiveDateTime) -> String {
    t.format(TIME).to_string()
}

/// Gespeicherter Zustand (`meta` `warn_state`): offene Probleme, behobene für die nächste Mail
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WarnState {
    pub open: Vec<Open>,
    /// gemeldet und seither behoben; kommt in die nächste Warnmail
    resolved: Vec<Open>,
    /// letzter Versuch, eine Warnmail zu senden
    last_try: Option<String>,
    /// seit wann Warnmails nicht rausgehen
    failing_since: Option<String>,
}

/// Eine fällige Warnmail; `keys` gelten nach dem Senden als gemeldet
pub struct WarnMail {
    pub subject: String,
    pub text: String,
    pub keys: Vec<String>,
}

impl WarnState {
    /// Gefundene Probleme übernehmen. `unsure`: Schlüssel, über die gerade nichts bekannt ist (z.B.
    /// Linie verbindet noch); sie bleiben, wie sie sind. Liefert, ob sich etwas geändert hat.
    pub fn update(
        &mut self,
        found: Vec<Problem>,
        unsure: &[String],
        s: &WarnSettings,
        now: NaiveDateTime,
    ) -> bool {
        let found: Vec<Problem> = found.into_iter().filter(|p| s.on(p.kind)).collect();
        let (keep, gone): (Vec<Open>, Vec<Open>) =
            std::mem::take(&mut self.open).into_iter().partition(|o| {
                s.on(&o.kind) && (unsure.contains(&o.key) || found.iter().any(|p| p.key == o.key))
            });
        let mut changed = !gone.is_empty();
        self.open = keep;
        for o in gone {
            if o.mailed.is_some() && s.on(&o.kind) && s.resolved {
                self.resolved.retain(|r| r.key != o.key);
                self.resolved.push(o);
            }
        }
        for p in found {
            if let Some(o) = self.open.iter_mut().find(|o| o.key == p.key) {
                if (&o.de, &o.en, &o.place) != (&p.text.de, &p.text.en, &p.place) {
                    (o.de, o.en, o.place) = (p.text.de, p.text.en, p.place);
                    changed = true;
                }
                continue;
            }
            // kurz behoben und wieder da, bevor es gemeldet wurde: gilt als gemeldet
            let back = self.resolved.iter().position(|r| r.key == p.key);
            let mailed = back.and_then(|i| self.resolved.remove(i).mailed);
            self.open.push(Open {
                key: p.key,
                kind: p.kind.to_string(),
                de: p.text.de,
                en: p.text.en,
                place: p.place,
                since: p.since.unwrap_or_else(|| stamp(now)),
                line: p.line,
                meter: p.meter,
                project: p.project,
                mailed,
                seen: false,
            });
            changed = true;
        }
        changed
    }

    /// Was die Oberfläche zeigt: offene, die lange genug bestehen
    pub fn visible(&self, s: &WarnSettings, now: NaiveDateTime) -> Vec<&Open> {
        self.open.iter().filter(|o| o.active(s, now)).collect()
    }

    /// Weggeklickt („gesehen“); leer = alle. Liefert, ob sich etwas geändert hat.
    pub fn see(&mut self, keys: &[String]) -> bool {
        let mut changed = false;
        for o in &mut self.open {
            if !o.seen && (keys.is_empty() || keys.contains(&o.key)) {
                o.seen = true;
                changed = true;
            }
        }
        changed
    }

    /// Warnmail, wenn neue Probleme da sind (oder zu erinnern ist) und der Abstand vorbei ist
    pub fn mail_due(&self, s: &WarnSettings, device: &str, now: NaiveDateTime) -> Option<WarnMail> {
        if s.to.trim().is_empty() {
            return None;
        }
        let wait = if self.failing_since.is_some() {
            RETRY_MINUTES
        } else {
            s.interval_minutes
        };
        if self
            .last_try
            .as_deref()
            .and_then(parse)
            .is_some_and(|t| now - t < Duration::minutes(wait))
        {
            return None;
        }
        let active = self.visible(s, now);
        let remind = |o: &&Open| {
            s.remind
                && o.mailed
                    .as_deref()
                    .and_then(parse)
                    .is_some_and(|t| now - t >= Duration::hours(REMIND_HOURS))
        };
        let (new, old): (Vec<&Open>, Vec<&Open>) =
            active.into_iter().partition(|o| o.mailed.is_none());
        let reminded: Vec<&Open> = old.iter().copied().filter(remind).collect();
        if new.is_empty() && reminded.is_empty() {
            return None;
        }
        let mut text = String::new();
        section(&mut text, &tr("Neue Probleme", "New problems"), &new);
        section(&mut text, &tr("Weiterhin offen", "Still open"), &old);
        let resolved: Vec<&Open> = self.resolved.iter().collect();
        section(
            &mut text,
            &tr(
                "Behoben seit der letzten Meldung",
                "Resolved since the last message",
            ),
            &resolved,
        );
        text.push_str(&format!(
            "{} {device}\r\n",
            tr("Gesendet von mbusio", "Sent by mbusio")
        ));
        let template = if s.subject.trim().is_empty() {
            default_subject()
        } else {
            s.subject.clone()
        };
        let count = (new.len() + reminded.len()).to_string();
        Some(WarnMail {
            subject: crate::mail::fill(&template, &[("geraet", device), ("anzahl", &count)]),
            text,
            keys: new.iter().chain(&reminded).map(|o| o.key.clone()).collect(),
        })
    }

    /// Ergebnis einer Warnmail; geht es einen Tag lang nicht, gelten die Probleme als gemeldet
    /// (der Bericht nennt sie weiter), sonst käme nach dem Reparieren eine Flut
    pub fn mail_done(&mut self, keys: &[String], sent: bool, now: NaiveDateTime) {
        self.last_try = Some(stamp(now));
        if !sent {
            let since = self.failing_since.get_or_insert_with(|| stamp(now));
            if parse(since).is_some_and(|t| now - t < Duration::hours(GIVE_UP_HOURS)) {
                return;
            }
        }
        for o in &mut self.open {
            if keys.contains(&o.key) {
                o.mailed = Some(stamp(now));
            }
        }
        self.resolved.clear();
        self.failing_since = None;
    }
}

/// Abschnitt einer Warnmail (auch im Bericht): Überschrift mit Anzahl, Zeilen, Rest als Anzahl
pub(crate) fn section(out: &mut String, title: &str, items: &[&Open]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("{title} ({}):\r\n", items.len()));
    for o in items.iter().take(MAIL_LIST_MAX) {
        out.push_str(&o.line_text());
        out.push_str("\r\n");
    }
    if items.len() > MAIL_LIST_MAX {
        out.push_str(&format!(
            "- … {} {}\r\n",
            items.len() - MAIL_LIST_MAX,
            tr("weitere", "more")
        ));
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests;
