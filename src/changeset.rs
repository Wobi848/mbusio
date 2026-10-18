//! Änderungssatz für alles, was zuerst geprüft und dann übernommen wird (Datenpunkt-CSV,
//! Zählerliste importieren, Namen erzeugen): was kommt dazu, was ändert sich, was geht nicht.
//! Die Oberfläche zeigt jeden Änderungssatz gleich an (Vorschau → Übernehmen).

use anyhow::Result;
use serde::Serialize;

use crate::i18n::tr;

/// Ein neuer Eintrag: wer (z.B. ID oder Nr.) und was dazukommt
#[derive(Debug, Serialize)]
pub struct Added {
    pub item: String,
    pub detail: String,
}

/// Ein geändertes Feld (Feldname schon übersetzt)
#[derive(Debug, Serialize)]
pub struct Changed {
    pub item: String,
    pub field: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ChangeSet {
    pub added: Vec<Added>,
    pub changed: Vec<Changed>,
    /// Einträge, die schon so sind
    pub unchanged: usize,
    /// Hinweise, die das Übernehmen nicht verhindern
    pub warnings: Vec<String>,
    /// Gründe, warum nichts übernommen wird
    pub errors: Vec<String>,
    pub applied: bool,
}

impl ChangeSet {
    pub fn add(&mut self, item: impl Into<String>, detail: impl Into<String>) {
        self.added.push(Added {
            item: item.into(),
            detail: detail.into(),
        });
    }

    /// Feld vergleichen und bei Unterschied vormerken; liefert, ob es sich ändert
    pub fn change(&mut self, item: &str, field: String, old: &str, new: &str) -> bool {
        if old == new {
            return false;
        }
        self.changed.push(Changed {
            item: item.into(),
            field,
            old: old.into(),
            new: new.into(),
        });
        true
    }

    /// Fehler in einer Zeile der Eingabe (1-basiert)
    pub fn row_error(&mut self, row: usize, msg: impl std::fmt::Display) {
        self.errors
            .push(format!("{} {row}: {msg}", tr("Zeile", "Row")));
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    /// Übernehmen, wenn gewünscht und ohne Fehler (sonst bleibt alles, wie es ist)
    pub fn finish(mut self, apply: bool, f: impl FnOnce() -> Result<()>) -> Result<Self> {
        if apply && self.errors.is_empty() {
            f()?;
            self.applied = true;
        }
        Ok(self)
    }
}
