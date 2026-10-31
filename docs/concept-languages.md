# Concept: more languages

Date: 2026-10-31 · Status: **steps 1 to 4 built**; French, Italian and Spanish are translated, not reviewed and not in the menu

Goal: add French, Italian and Spanish to mbusio, and make any further language mostly a matter of
translating, without touching the code each time. German and English must behave exactly as today.

## 1. Where languages live today

| Part | How it works | Size |
|---|---|---|
| Server messages | `tr("deutsch", "english")` in the code (`src/i18n.rs`) | 568 calls in 78 files, 559 of them plain literals |
| Quantity and medium names | `LABELS` table with key, German, English (`src/i18n.rs`) | about 85 entries |
| Messages shown later | `both()` builds a `Text { de, en }` so the language can change afterwards | 52 calls |
| Open warnings | `Open { de, en, … }`, **saved as JSON in the database** (`src/warn/mod.rs`) | stored data |
| Interface and report | Keys in `UI.de` / `UI.en` (`web/js/texts.js`, `web/app/*-texte.js`) | about 950 keys, some are functions for plurals |
| Formats | `lang === "de"` in the interface, `Lang::De` in the server | 7 lines in `web/`, a few in `src/` |
| Node-RED | `node-red/nodes/locales/{de,en-US}` | 6 files per language |

The current language is an enum with two values, stored as a code (`ui_lang` in `meta`), and it is
sent with each request (`X-Lang`, `?lang=`). Background work follows the language of the interface.

**What already helps:** meter data uses neutral keys (`energy`, `medium_water`), the interface
separates keys from code, and the CSV import of datapoints reads columns by position, not by name.

## 2. Decisions

### Server: keep `tr(de, en)`, use the English text as the key

The 559 literal calls stay as they are. For any other language, `tr` looks up the English text in a
catalog and falls back to English:

```rust
pub fn tr(de: &str, en: &str) -> String {
    match current() {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
        other => catalog(other).get(en).copied().unwrap_or(en).to_string(),
    }
}
```

Catalogs are JSON files embedded in the binary (`locales/fr.json`, one per language):
`{ "Unknown encryption": "Chiffrement inconnu" }`.

Why: no rewrite of 568 call sites, so the risk for German and English is close to zero, the code
stays readable, and a missing translation shows English instead of an error.

The downside: one English text can only have one translation. If a word needs two translations
depending on context, the English texts get a small difference. We expect this to be rare.

The 9 non-literal calls were handled one by one:
- 3 in `src/line.rs` and `src/web/state.rs` now use literals with placeholders, because users see them.
- 5 in `src/service/win.rs` are console messages during Windows service setup. They stay English in
  the new languages through the fallback.
- 1 in `src/billing/table.rs` is now a plain list of `tr` calls.

`tr_in(lang, "de", "en")` does the same for code that has its language as a parameter. All texts stay
literals, because the catalog check reads them from the source.

`LABELS` gets the same treatment: the catalog holds `energy`-style keys in a `labels` section.

### Stored and delayed texts: extend without breaking old data

`Text` and `Open` keep `de` and `en` and gain a map for the other languages:

```rust
#[serde(default)]
pub more: BTreeMap<String, String>,
```

`both()` evaluates its closure once per language and fills the map. A warning saved before the
update has no entry, so it shows English in the new languages until it is created again. Old
databases load unchanged, and a backup taken with a new version still loads in an older one because
unknown JSON fields are ignored (to be confirmed by a test in step 1).

### Interface: one file per language, loaded when needed

German and English stay where they are. Each new language gets one flat file with all its keys
(`web/js/lang-fr.js` sets `UI.fr = { … }`), loaded before the first render in `app.js` and
`report.js`. The loader comes with the first language in step 2, because until then nothing would use
it. Plurals stay functions, because French and others count differently:

```js
ps_lines: (n) => (n <= 1 ? `${n} ligne` : `${n} lignes`),
```

`t()` falls back in this order: selected language, English, German, key. Language codes and their
native names are defined once and fill the menu.

### Formats: one place

One function per side returns the formats of a language. All `lang === "de"` and `Lang::De` checks
for formats move there, so a new language can no longer silently be treated as English.

| Language | Locale | Date | CSV decimal mark (default) |
|---|---|---|---|
| de | de-CH | 31.10.2026 | `,` |
| en | en-GB | 31/10/2026 | `.` |
| fr | fr-CH | 31.10.2026 | `,` |
| it | it-CH | 31.10.2026 | `,` |
| es | es-ES | 31/10/2026 | `,` |

Places to move: `web/js/base.js`, `web/js/plot.js`, `web/app/zaehler-verlauf.js`,
`web/app/weitergabe-abrechnung.js`, `src/billing/settings.rs`, `src/billing/table.rs`.

Words chosen by a comparison (`src/datapoint/key.rs`, `src/datapoint/csv.rs`, `src/meterlist.rs`,
`src/export.rs`, `src/web/outputs.rs`) go through `tr_in`.

Imports: datapoint CSV files are read by column position and do not depend on the language. **The
meter list import guesses columns from header words** (`GUESS` in `web/js/listlogic.js`), in German
and English only. Step 2 adds the words of each new language there.

### Scope

In: interface, report, server messages, e-mails, exports, quantity and medium names, Node-RED.

Out for now:
- README and concepts. English and German only.
- Command line help.
- Right-to-left languages and CJK fonts. They would need layout and font work of their own.

Datapoint names already saved stay in the language they were created in; they are user data.

## 3. Compatibility

- **API:** `X-Lang` and `?lang=` also accept `fr`, `it`, `es`. These codes returned German until now;
  this is a behaviour change for release notes. Unknown codes stay German, as today.
- **Browser default:** without a saved choice, a browser language mbusio does not know now gets
  English instead of German. Also for the release notes. A saved choice is never changed.
- **Stored data:** no migration needed, see section 2.
- **Version:** new feature without a break, so a minor version. Node-RED likewise.
- **Release:** nothing is tagged or published as part of this work unless the maintainer asks.

## 4. Terms

Translators work from one glossary so the same word stays the same word. It belongs in the catalog
folder. **Draft, needs review by someone from the trade.**

| German | English | French | Italian | Spanish |
|---|---|---|---|---|
| Zähler | Meter | Compteur | Contatore | Contador |
| Wärmezähler | Heat meter | Compteur de chaleur | Contatore di calore | Contador de calor |
| Primäradresse | Primary address | Adresse primaire | Indirizzo primario | Dirección primaria |
| Sekundäradresse | Secondary address | Adresse secondaire | Indirizzo secondario | Dirección secundaria |
| Linie | Line | Ligne | Linea | Línea |
| Anlage | Site | Installation | Impianto | Instalación |
| Dauerbetrieb | Continuous operation | Fonctionnement continu | Funzionamento continuo | Funcionamiento continuo |
| Ausleseplan | Read plan | Plan de relevé | Piano di lettura | Plan de lectura |
| Auslesen | Read | Relever | Leggere | Leer |
| Datenpunkt | Datapoint | Point de données | Punto dati | Punto de datos |
| Weitergabe | Pass on | Transmission | Trasmissione | Transmisión |
| Abrechnung | Billing | Facturation | Fatturazione | Facturación |
| Vorlauf / Rücklauf | Inlet / outlet | Départ / retour | Mandata / ritorno | Impulsión / retorno |

Protocol terms such as M-Bus, Baud, MQTT, Modbus, Telegram and Scan stay as they are where that is
common in the trade. The review decides the rest.

## 5. Tests

- **Catalog check (Rust):** scans the source for `tr("…", "…")` and reports English texts missing
  from a catalog, and catalog entries nothing uses any more. Missing texts fail the test for every
  language that is switched on, so a language only appears in the menu once it is complete.
- **Key check (UI test `28-texte`):** every language has the same keys as English, and functions take the same
  arguments.
- **`tests/ui/08-sprachen`:** runs for all languages. Today's check for German words in English
  applies to all languages except German.
- **Layout:** the check for overflow and overlap from the design work becomes a permanent test, per
  language at 390, 1000 and 1250 px. French and Spanish are longer than English.
- **Old data:** a database with stored warnings in `de`/`en` loads and shows them in all languages.
- **Unchanged:** all existing tests stay green without changes in step 1. That is the proof that
  German and English are untouched.

## 6. Steps

Each step is its own commit and leaves everything working. The new languages stay hidden from the
menu until they are complete.

1. **Engine, no new language (done).** `Lang` with five values, catalogs, `tr`, `Text`, `Open`, one
   place for formats, checks. German and English identical to before.
2. **French, complete (done, not reviewed).** Includes the interface loader and the header words of
   the list import. The language is not in the menu yet; reviewers open it with `?lang=fr`, which
   `LANG_PREVIEW` in `web/js/base.js` allows.
3. **Italian and Spanish (done, not reviewed).** Same preview as French: `?lang=it`, `?lang=es`.
4. **Node-RED** in all three languages **(done, not reviewed)**. The folders are `fr`, `it` and `es`:
   Node-RED falls back from `es-MX` or `fr-CH` to the base code, so `es-ES` would miss other regions.
   Node-RED follows the language of the browser, so these translations are live as soon as the package
   with them is installed. It needs no menu.
5. **Review** by someone from the trade, then switch the languages on in the menu.

This branch builds on `design-polish`, which has to reach `main` first.

## 7. Size of the translation

About 1 500 texts per language: 950 interface keys, around 500 distinct server messages (many of the
559 repeat, exact number from the catalog check) and 85 names, plus Node-RED.

The first draft is machine-made. That is why the review in step 5 is a requirement, not an extra:
a wrong term on a meter tool costs trust, and the people using it will notice right away.

## 8. Open

- Who reviews French, Italian and Spanish?
