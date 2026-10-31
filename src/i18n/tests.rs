use std::collections::HashSet;
use std::path::Path;

use super::*;

#[test]
fn labels_translate() {
    assert_eq!(label("energy", Lang::En), "Energy");
    assert_eq!(label("medium_water", Lang::De), "Wasser");
    assert_eq!(label("note_error_code:05", Lang::De), "Fehlercode 05");
    assert_eq!(label("no_such_key", Lang::En), "no_such_key");
    assert_eq!(Lang::parse("en-US"), Lang::En);
    assert_eq!(Lang::parse("de-CH"), Lang::De);
}

#[test]
fn both_languages_and_thread_language_kept() {
    set(Lang::En);
    let t = both(|| format!("{} 3", tr("Zähler", "Meter")));
    assert_eq!((t.de.as_str(), t.en.as_str()), ("Zähler 3", "Meter 3"));
    // danach gilt wieder die Sprache des Threads
    assert_eq!(tr("ja", "yes"), "yes");
    assert_eq!(t.get(), "Meter 3");
    // ohne Übersetzung in beiden gleich
    let raw = Text::from("68 03 03 68".to_string());
    assert_eq!(raw.de, raw.en);
    follow_ui();
}

#[test]
fn no_duplicate_keys() {
    let mut keys: Vec<_> = labels::LABELS.iter().map(|l| l.0).collect();
    keys.sort();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n);
}

#[test]
fn catalog_text_or_english() {
    // Kataloge sind in dieser Stufe leer: alles erscheint auf Englisch, nie der Schlüssel
    assert_eq!(tr_in(Lang::De, "Zähler", "Meter"), "Zähler");
    assert_eq!(tr_in(Lang::En, "Zähler", "Meter"), "Meter");
    assert_eq!(
        tr_in(Lang::Fr, "Ein neuer Text", "A new text"),
        "A new text"
    );
    assert_eq!(label("no_such_key", Lang::It), "no_such_key");
    for lang in ALL {
        assert!(!label("energy", lang).is_empty());
    }
}

#[test]
fn unavailable_language_stays_german() {
    // eine Sprache ausserhalb von OFFERED verhält sich wie jede unbekannte
    for code in ["xx", ""] {
        assert_eq!(Lang::parse(code), Lang::De);
    }
    for lang in ALL.into_iter().filter(|l| !OFFERED.contains(l)) {
        assert_eq!(Lang::parse(lang.code()), Lang::De);
    }
}

#[test]
fn stored_text_falls_back_to_english() {
    let more = BTreeMap::from([("fr".to_string(), "Compteur".to_string())]);
    set(Lang::Fr);
    assert_eq!(pick("Zähler", "Meter", &more), "Compteur");
    set(Lang::It);
    assert_eq!(pick("Zähler", "Meter", &more), "Meter");
    set(Lang::De);
    assert_eq!(pick("Zähler", "Meter", &more), "Zähler");
    follow_ui();
}

#[test]
fn formats_by_language() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
    let shown = |l: Lang| date.format(l.date_format()).to_string();
    assert_eq!(shown(Lang::De), "31.10.2026");
    assert_eq!(shown(Lang::En), "2026-10-31");
    assert_eq!(shown(Lang::Fr), "31.10.2026");
    assert_eq!(shown(Lang::Es), "31/10/2026");
    assert_eq!(Lang::De.decimal(), ",");
    assert_eq!(Lang::En.decimal(), ".");
    assert_eq!(Lang::Fr.decimal(), ",");
}

/// Alle Texte aus `tr("de", "en")` und `tr_in(lang, "de", "en")` im Quellcode: englische Fassungen
fn source_texts() -> HashSet<String> {
    fn walk(dir: &Path, out: &mut HashSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                scan(&std::fs::read_to_string(path).unwrap(), out);
            }
        }
    }
    fn scan(src: &str, out: &mut HashSet<String>) {
        for start in ["tr(", "tr_in("] {
            let mut rest = src;
            while let Some(i) = rest.find(start) {
                let after = &rest[i + start.len()..];
                let word = i == 0
                    || !rest.as_bytes()[i - 1].is_ascii_alphanumeric()
                        && rest.as_bytes()[i - 1] != b'_';
                rest = after;
                if !word {
                    continue;
                }
                let args = after
                    .split_once(',')
                    .filter(|_| start == "tr_in(")
                    .map_or(after, |x| x.1);
                if let Some((_, tail)) = literal(args) {
                    if let Some((en, _)) = tail.trim_start().strip_prefix(',').and_then(literal) {
                        out.insert(en);
                    }
                }
            }
        }
    }
    /// Zeichenketten-Literal am Anfang lesen: (Inhalt, Rest)
    fn literal(s: &str) -> Option<(String, &str)> {
        let body = s.trim_start().strip_prefix('"')?;
        let mut text = String::new();
        let mut chars = body.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => return Some((text, &body[i + 1..])),
                '\\' => match chars.next()?.1 {
                    'n' => text.push('\n'),
                    other => text.push(other),
                },
                other => text.push(other),
            }
        }
        None
    }
    let mut out = HashSet::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    out
}

#[test]
fn catalogs_match_the_source() {
    let texts = source_texts();
    assert!(
        texts.len() > 300,
        "Texte im Quellcode gefunden: {}",
        texts.len()
    );
    let keys: HashSet<&str> = labels::LABELS.iter().map(|l| l.0).collect();
    for lang in ALL
        .into_iter()
        .filter(|l| !matches!(l, Lang::De | Lang::En))
    {
        let c = catalog(lang);
        let stale: Vec<_> = c.messages.keys().filter(|k| !texts.contains(*k)).collect();
        assert!(
            stale.is_empty(),
            "{}: nicht mehr benutzt: {stale:?}",
            lang.code()
        );
        let stale: Vec<_> = c
            .labels
            .keys()
            .filter(|k| !keys.contains(k.as_str()))
            .collect();
        assert!(
            stale.is_empty(),
            "{}: unbekannte Namen: {stale:?}",
            lang.code()
        );
        if OFFERED.contains(&lang) {
            let missing: Vec<_> = texts
                .iter()
                .filter(|k| !c.messages.contains_key(*k))
                .collect();
            assert!(missing.is_empty(), "{}: fehlen: {missing:?}", lang.code());
            let missing: Vec<_> = keys
                .iter()
                .filter(|k| !c.labels.contains_key(**k))
                .collect();
            assert!(
                missing.is_empty(),
                "{}: Namen fehlen: {missing:?}",
                lang.code()
            );
        }
    }
}
