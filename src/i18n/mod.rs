//! Mehrsprachigkeit (Konzept: docs/concept-languages.md).
//!
//! Messdaten tragen sprachneutrale Schlüssel (`energy`, `medium_water` …), übersetzt
//! wird erst bei der Anzeige oder beim Export. Meldungen aus dem Backend verwenden
//! die Sprache des aktuellen Requests (Task) bzw. Threads.
//!
//! Deutsch und Englisch stehen im Code (`tr`, `labels::LABELS`). Für alle weiteren Sprachen ist der
//! englische Text der Schlüssel in `locales/<sprache>.json`; fehlt er dort, erscheint Englisch.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use serde::Deserialize;

mod labels;
#[cfg(test)]
mod tests;

pub use labels::{label, labels};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    De,
    En,
    Fr,
    It,
    Es,
}

/// Alle Sprachen; die Reihenfolge ist der Wert in `UI`
const ALL: [Lang; 5] = [Lang::De, Lang::En, Lang::Fr, Lang::It, Lang::Es];

/// Übersetzte Sprachen; für sie prüft ein Test, dass kein Text fehlt
#[cfg(test)]
const COMPLETE: [Lang; 3] = [Lang::Fr, Lang::It, Lang::Es];

impl Lang {
    /// "en", "en-US", "de-CH" … ; alles Unbekannte wird Deutsch.
    pub fn parse(s: &str) -> Self {
        let s = s.trim().to_ascii_lowercase();
        ALL.into_iter()
            .find(|l| s.starts_with(l.code()))
            .unwrap_or(Lang::De)
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Es => "es",
        }
    }

    /// Datum in Exporten und Meldungen, für chrono (Gegenstück zu `LANG_FORMATS` in web/js/base.js)
    pub fn date_format(self) -> &'static str {
        match self {
            Lang::En => "%Y-%m-%d",
            Lang::Es => "%d/%m/%Y",
            Lang::De | Lang::Fr | Lang::It => "%d.%m.%Y",
        }
    }

    /// Dezimalzeichen in Exporten, wenn nichts eingestellt ist
    pub fn decimal(self) -> &'static str {
        match self {
            Lang::En => ".",
            Lang::De | Lang::Fr | Lang::It | Lang::Es => ",",
        }
    }
}

/// Übersetzungen einer Sprache aus `locales/`
#[derive(Debug, Default, Deserialize)]
struct Catalog {
    /// Schlüssel der Messdaten (`energy`)
    #[serde(default)]
    labels: HashMap<String, String>,
    /// englischer Text → Übersetzung
    #[serde(default)]
    messages: HashMap<String, String>,
}

/// Eingebettete Übersetzungen; dass sie gültig sind, prüft ein Test
fn catalog(lang: Lang) -> &'static Catalog {
    static CATALOGS: OnceLock<[Catalog; 3]> = OnceLock::new();
    let [fr, it, es] = CATALOGS.get_or_init(|| {
        [
            include_str!("../../locales/fr.json"),
            include_str!("../../locales/it.json"),
            include_str!("../../locales/es.json"),
        ]
        .map(|json| serde_json::from_str(json).expect("locales/*.json gültig"))
    });
    match lang {
        Lang::Fr => fr,
        Lang::It => it,
        Lang::Es => es,
        Lang::De | Lang::En => unreachable!("Deutsch und Englisch stehen im Code"),
    }
}

thread_local! {
    /// Sprache des Threads; `None` = wie die Oberfläche
    static CURRENT: Cell<Option<Lang>> = const { Cell::new(None) };
    /// vorübergehend erzwungen (`both`), gilt vor Request und Thread
    static FORCED: Cell<Option<Lang>> = const { Cell::new(None) };
}

tokio::task_local! {
    static TASK: Lang;
}

/// Sprache der Oberfläche: die der letzten Anfrage mit `X-Lang` (Stelle in `ALL`). Danach
/// richten sich Arbeiten ohne eigene Anfrage (Zeitplan, Ausgänge, Linien-Threads)
static UI: AtomicU8 = AtomicU8::new(0);

/// Sprache des Threads setzen (Aufträge aus einer Anfrage)
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(Some(lang)));
}

/// Thread folgt wieder der Sprache der Oberfläche (Arbeit im Hintergrund)
pub fn follow_ui() {
    CURRENT.with(|c| c.set(None));
}

/// Sprache der Oberfläche merken (Anfrage mit `X-Lang`, beim Start die gespeicherte). Liefert, ob
/// sie sich geändert hat.
pub fn remember_ui(lang: Lang) -> bool {
    UI.swap(lang as u8, Ordering::Relaxed) != lang as u8
}

fn ui() -> Lang {
    ALL[UI.load(Ordering::Relaxed) as usize]
}

/// `f` mit dieser Sprache ausführen, auch über `await` hinweg (ein Request)
pub async fn scope<F: Future>(lang: Lang, f: F) -> F::Output {
    TASK.scope(lang, f).await
}

/// Erzwungene Sprache, sonst die des Requests, sonst des Threads, sonst der Oberfläche
pub fn current() -> Lang {
    FORCED
        .with(|c| c.get())
        .or_else(|| TASK.try_with(|l| *l).ok())
        .or_else(|| CURRENT.with(|c| c.get()))
        .unwrap_or_else(ui)
}

/// Meldung in der Sprache des aktuellen Threads.
pub fn tr(de: &str, en: &str) -> String {
    tr_in(current(), de, en)
}

/// Meldung in einer bestimmten Sprache; beide Texte als Literale, das liest der Test der Übersetzungen
pub fn tr_in(lang: Lang, de: &str, en: &str) -> String {
    match lang {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
        other => catalog(other)
            .messages
            .get(en)
            .map_or(en, String::as_str)
            .to_string(),
    }
}

/// Der passende Text aus einem gespeicherten Satz; fehlt die Sprache, gilt Englisch
pub fn pick<'a>(de: &'a str, en: &'a str, more: &'a BTreeMap<String, String>) -> &'a str {
    match current() {
        Lang::De => de,
        Lang::En => en,
        other => more.get(other.code()).map_or(en, String::as_str),
    }
}

/// Text in allen Sprachen, gezeigt in der Sprache, die gerade fragt (Protokoll: eine Meldung
/// bleibt, die Sprache der Oberfläche kann wechseln)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub de: String,
    pub en: String,
    /// weitere Sprachen, nur wo sie von Englisch abweichen
    pub more: BTreeMap<String, String>,
}

impl Text {
    pub fn get(&self) -> &str {
        pick(&self.de, &self.en, &self.more)
    }
}

/// Ohne Übersetzung (Telegramm, Fehlertext vom System): in allen Sprachen gleich
impl From<String> for Text {
    fn from(s: String) -> Self {
        Text {
            de: s.clone(),
            en: s,
            more: BTreeMap::new(),
        }
    }
}

/// `f` je Sprache einmal auswerten: alles, was darin `tr` benutzt, gibt es dann in allen Sprachen
pub fn both(f: impl Fn() -> String) -> Text {
    let with = |lang| {
        let before = FORCED.with(|c| c.replace(Some(lang)));
        let s = f();
        FORCED.with(|c| c.set(before));
        s
    };
    let en = with(Lang::En);
    let more = ALL
        .into_iter()
        .filter(|l| !matches!(l, Lang::De | Lang::En))
        .filter_map(|l| {
            let s = with(l);
            (s != en).then(|| (l.code().to_string(), s))
        })
        .collect();
    Text {
        de: with(Lang::De),
        en,
        more,
    }
}
