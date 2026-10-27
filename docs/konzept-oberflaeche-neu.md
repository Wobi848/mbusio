# Konzept: neue Oberfläche (Werkzeug statt Webseite)

Stand: 25.10.2026 · Status: **umgesetzt**, neue Oberfläche ist die Startseite; Entscheidungen in Abschnitt 10

Grundlage: Leitbild [konzept-bedienung.md §14](konzept-bedienung.md) und das Audit vom 25.10.2026
(Abschnitt 2). Die Server-Schnittstelle steht in [api.md](api.md); die neue Oberfläche benutzt
nur diese Endpunkte.

---

## 1. Ziel und Abgrenzung

- mbusio soll sich anfühlen wie ein professionelles Werkzeug: **selbsterklärend**, dicht,
  schnell, mit Tastatur bedienbar, ohne Neuladen und ohne Browser-Dialoge.
- **Gleicher Funktionsumfang** wie heute, keine neuen Funktionen nebenbei (ausser
  Mehrfachauswahl, die das Leitbild verlangt).
- **Parallel bauen:** die neue Oberfläche entsteht in `web/neu/` und ist unter `/neu/` erreichbar.
  Die heutige bleibt unter `/` und wird nur noch bei echten Fehlern angefasst, bis die neue
  alles kann. Dann tauschen (Abschnitt 8).
- **Kein Framework, kein Build-Schritt:** JavaScript-Module (`<script type="module">`), wie
  heute direkt von mbusio ausgeliefert (eingebettet in die Datei). Grund: keine Werkzeugkette,
  läuft so auf dem Pi und offline, und die Probleme aus dem Audit liegen nicht am fehlenden
  Framework, sondern daran, wie neu gezeichnet wird (Abschnitt 5 löst das mit wenig Code).
- Server (Rust): nur zwei kleine Ergänzungen, siehe Abschnitt 7.

## 2. Vorgaben aus dem Audit

Jede Schwachstelle wird zu einer prüfbaren Vorgabe (Abnahme mit den Messskripten aus dem Audit):

| Nr | Heute | Vorgabe für die neue Oberfläche |
| --- | --- | --- |
| 1 | Statusabfrage baut alle 5 s Leisten neu, Fokus geht verloren | Neu gezeichnet wird nur, was sich geändert hat; Fokus, Hover und offene Menüs bleiben (20 s Ruhe: 0 Fokusverluste) |
| 2 | Trefferliste alle 600 ms komplett ersetzt, Klicks gehen verloren | Listen werden zeilenweise nachgeführt; bestehende Zeilen bleiben dieselben Elemente (0 verlorene Klicks bei 150-ms-Klicks während des Scans) |
| 3 | Linie/Projekt wechseln lädt die Seite neu | Kein Neuladen; Filter, Suche, Auswahl, Scrollposition je Bereich bleiben; Wechsel < 150 ms lokal |
| 4 | 8 von 30 Zählern, 3 von 30 Treffern sichtbar (1250×900) | ≥ 18 Zählerzeilen und ≥ 15 Treffer sichtbar bei 1250×900; mehr bei grösseren Bildschirmen |
| 5 | 25 Browser-Dialoge | 0 `confirm`/`prompt`/`alert`; Rückfrage im Kontext nur bei Gefährlichem, sonst „Rückgängig“ |
| 6 | Zustand nur im Tooltip oder Erklärtext, drei Wörter für „Merken“ | Zustand als Wort/Farbe am Element; ein Wort je Aktion; Tooltips nur für Details |
| 7 | Tastatur: nur Tab | Escape, Enter, ↑↓, Leertaste, `/`, Shortcuts (Abschnitt 6) |
| 8 | Keine Mehrfachauswahl | Auswahl in Zähler- und Trefferliste mit Aktionsleiste |
| 9 | „Alle auslesen“ lädt bei jeder Abfrage Liste und Baum neu | Nur betroffene Zeilen nachführen; Baum erst am Ende |
| 10 | Projektbaum unruhig, Plan nur über Umweg | Feste Spalten; Plan und Dauerbetrieb direkt an der Linie änderbar |

## 3. Aufbau

### 3.1 Desktop (ab etwa 1000 px)

```
┌ mbusio  Projekt [MFH Bergstr. 12 ▾]  Linie [Haus A ▾]  ● Verbunden COM3 · 2400  ◐ Dauerbetrieb ein · täglich 00:05   ● 2 im Dauerbetrieb  ⚙ ┐
├ Scan │ Zähler 30 │ Weitergabe 13 │ Anlage │ Werkzeuge │ Simulator ─────────────────────────────────────────────────────────────────────┤
│ [/ Suchen…]  Alle 30 · Gestört 1 · Ohne Namen 6                                   [+ Zähler] [Importieren] [Alle 30 auslesen]       │
├──────────────────────────────────────────────────────────────────────────┬────────────────────────────────────────────────────────────┤
│ ☐ Zähler                ID · PA       Ort      Letzter Wert      Plan     │ Whg 1 Wärme                          Wärme · KAM · 2400 Bd │
│ ☐ Whg 1 Wärme           71000001 · 1  Whg 1    370 590 kWh 17:15  täglich │ ID 71000001 · PA 1                                         │
│ ☑ Whg 1 Kaltwasser      71000002 · 2  Whg 1     12,3 m³    17:15  täglich │ Name [Whg 1 Wärme    ] Ort [Whg 1   ] Notiz [          ]  │
│ ☑ Whg 1 Warmwasser      71000003 · 3  Whg 1      4,1 m³    17:15  täglich │ Plan [wie Linie ▾]   Uhr im Zähler +1 h 3 min             │
│ ✗ Whg 2 Wärme           71000004 · 4  Whg 2    antwortet nicht    täglich │ ── Auslesung 17:15:49 ───────────────────────────────────  │
│   … (36 px je Zeile)                                                      │ Energie          370 590 kWh   ✓ weitergegeben            │
│                                                                           │ Volumen             12,3 m³                                │
│                                                                           │ Vorlauf             55,1 °C                                │
│                                                                           │ [Auslesen R] [Verlauf V] [Werte wählen] [Tauschen…]        │
├ 2 ausgewählt: [Auslesen] [Plan ▾] [Werte wählen] [Löschen]   Esc = Auswahl aufheben ┴────────────────────────────────────────────────────────────┤
└ ● Bereit · COM3 · 2400 Bd   ✓ Keine Auffälligkeiten   MQTT ● verbunden                         Nächster Schritt: Werte wählen [→] ┘
```

- **Eine Kopfzeile** (etwa 44 px) für alles, was immer gilt: Projekt, Linie, Verbindung (Zustand
  und ein Knopf je nach Zustand), Dauerbetrieb der Linie als Schalter mit Plan, Anzahl Linien im
  Dauerbetrieb über alle Projekte (aufklappbar), Einstellungen. Die heutige Linien-Leiste und
  der Dauerbetrieb-Kasten fallen in diese Zeile.
- **Bereiche als Laschen** (etwa 36 px): Scan, Zähler, Weitergabe, Anlage (Projektbaum,
  Projektdaten, Zustand der Anlage, Bericht), Werkzeuge, Simulator (nur wenn eine Simulation
  benutzt wird). Zahlen an der Lasche wie heute.
- **Tabellenleiste** je Bereich: Suche, Filter mit Anzahl, Aktionen des Bereichs.
- **Liste links, Details rechts.** Die gewählte Zeile zeigt rechts ihre Details und Aktionen.
  Bearbeiten direkt in den Feldern rechts (speichert beim Verlassen des Felds, Escape setzt
  zurück); kein Formular, das in der Zeile aufklappt.
- **Auswahlleiste** erscheint nur bei Mehrfachauswahl, über der Statusleiste.
- **Statusleiste unten** (eine Zeile): Bus, Auffälligkeiten, Ausgänge, nächster Schritt kompakt
  als ein Knopf. Der ausführliche geführte Ablauf entfällt; was zu tun ist, zeigt die Oberfläche
  selbst (z.B. Hauptknopf „3 neue merken“ im Scan).

### 3.2 Bereiche

| Bereich | Liste | Details rechts |
| --- | --- | --- |
| **Scan** | Treffer; Suchparameter als eine Zeile in der Tabellenleiste (Art, Bereich, Baudraten, [Primärscan starten]); Fortschritt in der Leiste; Zeile zeigt „✓ in Liste“ oder „neu“ | Treffer mit Auffälligkeit: Ursache, Empfehlung, Knopf zum Beheben (heutige Streifen), Rohtelegramm |
| **Zähler** | Zählerliste wie oben | Stammdaten, Plan, letzte Auslesung mit allen Werten, Verlauf, Werte wählen, Tauschen |
| **Weitergabe** | Datenpunkte (Nr, Name, Wert, Qualität, Ausgänge) | Datenpunkt: Ausgänge, Topic/Register, Sendeart, Faktor; oben die Ausgänge (MQTT, Modbus) mit Schalter und Zustand |
| **Anlage** | Projektbaum (Gateways › Linien) mit festen Spalten: Name, Anschluss, Zähler, Dauerbetrieb-Schalter, Plan, Zustand | Projekt bzw. Gateway bzw. Linie bearbeiten; Zustand der Anlage als Raster; Bericht, Export, Leeren |
| **Werkzeuge** | Ziel oben (Zähler aus Liste/Scan oder Adresse), ein Mal für alle Werkzeuge | Auslesen, Adresse, Baudrate, Reset; Telegramm-Log und Analyse als eigener Teil |
| **Simulator** | Simulationen | Zähler der Simulation mit Verhalten |

### 3.3 Handy und schmale Fenster (unter etwa 1000 px)

- Kopfzeile zweizeilig: oben mbusio, Dauerbetrieb-Anzahl, ⚙; darunter Projekt ▾ und Linie ▾ mit
  Verbindungszustand. Laschen als waagrechte Leiste mit Wort (nicht nur Symbol).
- Liste über die ganze Breite, Zeilen als zweizeilige Karten; Details öffnen sich als eigene
  Ansicht mit „← zurück“ (Escape/Zurück-Taste des Browsers).
- Grosse Treffflächen (mindestens 40 px) für Finger.

### 3.4 Der Ablauf, ohne Erklärung

Die Oberfläche zeigt den nächsten Schritt durch ihren Zustand, nicht durch Text: genau ein
Hauptknopf (gefüllt) an der Stelle, an der man ohnehin hinschaut; was erledigt ist, steht als
Zustand da. Der Knopf „Nächster Schritt“ in der Statusleiste macht dasselbe für die, die nicht
suchen wollen, und führt an die Stelle.

| Stand | Was man sieht (der Hauptknopf ist fett) |
| --- | --- |
| **Neu, kein Anschluss** | Kopfzeile: „● Anschluss noch nicht festgelegt **[Anschluss festlegen]**“. Bereich Scan offen (Start ohne Adresse: keine Zähler → Scan, sonst Zähler). |
| **Anschluss, getrennt** | Kopfzeile: „○ Getrennt **[Verbinden]**“; Statusleiste: Grund (z.B. „USB-Wandler abgezogen?“). |
| **Verbunden, noch nie gesucht** | Scan: Suchzeile mit **[Primärscan starten]**; die Liste darunter leer mit „Noch nicht gesucht“. |
| **Scan läuft** | Zeilen erscheinen einzeln, Fortschritt in der Suchzeile und Statusleiste, **[Abbrechen]**. |
| **Treffer mit Auffälligkeiten** | Zeile rot/orange mit Wort („Kollision“, „andere Baudrate“); oben **[N beheben]**; Details rechts: Ursache, Empfehlung, Knopf. |
| **Treffer in Ordnung, nicht gemerkt** | Zeile „neu“; oben **[N neue merken]**. Schon gemerkte: „✓ in Liste“, kein Knopf. |
| **Gemerkt, nicht ausgelesen** | Zähler: Zeilen „noch nicht ausgelesen“; oben **[Alle N auslesen]**. |
| **Ausgelesen** | Werte mit Zeit; Probleme als Wort („antwortet nicht“). Fertig für den Service: **[Bericht]** (Anlage). |
| **Dauerbetrieb einrichten** | Kopfzeile: Schalter Dauerbetrieb, ohne Plan gesperrt mit „kein Ausleseplan“ und Weg zum Plan; mit Plan **einschalten**. |
| **Werte weitergeben** | Zähler-Details: Kästchen „als Datenpunkt“; Weitergabe: Ausgang mit Schalter und Zustand („MQTT · läuft“). |
| **Läuft** | Kopfzeile: „● N im Dauerbetrieb“ (grün; orange bei Störung oder ohne Dienst). |

Prüfstein für jeden Bereich: Ein Techniker, der mbusio zum ersten Mal sieht, kommt ohne Hilfetext
vom leeren Projekt bis zum Bericht, nur indem er dem einen Hauptknopf folgt.

## 4. Selbsterklärend: Regeln für jedes Element

- **Zustand am Element:** Wort plus Farbe direkt am Ding, z.B. „✓ in Liste“, „antwortet nicht“,
  „● verbunden“, „Uhr +1 h 3 min“. Nie nur Farbe, nie nur Tooltip.
- **Knöpfe sagen, was passiert, mit Anzahl:** „3 neue merken“, „Alle 30 auslesen“, „2 löschen“.
  Gibt es nichts zu tun, fehlt der Knopf (statt gesperrt mit Erklärung). Ist etwas gesperrt,
  steht der Grund als kurzer Text daneben („nicht verbunden“).
- **Ein Wort je Aktion** überall gleich (Liste in `texts.js`, Abschnitt 5.5).
- **Leere Zustände** sagen, was man tun kann, mit dem passenden Knopf („Noch keine Zähler ·
  [Primärscan starten] [Liste importieren]“).
- **Erklärtexte** nur als ausklappbares „Warum?“ an Auffälligkeiten (wie heute), nicht als
  Absätze über Listen.

## 5. Technik

### 5.1 Dateien

```
web/index.html          Gerüst: Kopfzeile, Laschen, Arbeitsfläche, Bus-Konsole, Statusleiste
web/neu/index.html      leitet alte Lesezeichen /neu/ auf / weiter
web/neu/app.js          Start, Texte, Einstellungen, Bereiche umschalten, Tasten
web/neu/app-kopf.js     Kopfzeile: Projekt › Linie, Anschluss, Dauerbetrieb, „N im Dauerbetrieb“
web/neu/app-statusleiste.js  Statusleiste, Nächster Schritt, Bus-Konsole
web/neu/store.js        Zustand und Benachrichtigung; activeLine, faulty (Regel „gestört“)
web/neu/data.js         Laden vom Server, Linie wechseln, Dauerbetrieb schalten, Fehler melden
web/neu/view.js         Bausteine: Liste mit Schlüssel, Menü, Rückfrage am Ort, Meldung mit
                        Rückgängig, Vorschau → Übernehmen; outputWord, opText, opPossible
web/neu/keys.js         Tastatur: eine Stelle für alle Kürzel
web/neu/buslog.js       Protokoll des Busses für Werkzeuge und Bus-Konsole
web/neu/<bereich>.js    anlage, scan, zaehler, weitergabe, werkzeuge, simulator; je mit .css:
                        Aufbau, Ablauf, Bedienung. Daneben <bereich>-texte.js (Texte) und
                        abgegrenzte Teile als reine Bausteine, die alles als Parameter bekommen:
                        anlage-zustand/-baum/-details, zaehler-ansicht/-verlauf/-listen,
                        weitergabe-ausgaenge, scan-baudraten, werkzeuge-analyse
web/neu/app.css         Gerüst und Bausteine; Farben aus web/css/tokens.css
```

Gemeinsam mit dem Bericht (`report.html`): `web/js/texts.js` (Texte DE/EN), `web/js/base.js`
(`t`, `esc`, `request`, Zugriffsschlüssel, `time24`, `BAUDS`, …), `icons.js`, `scanlogic.js`,
`listlogic.js`, `plot.js`, `theme.js`, `web/css/tokens.css`.

### 5.2 Zustand und Neuzeichnen

- **Ein Zustand** (`store.js`): Status, aktives Projekt/Linie, Listen (Zähler, Treffer,
  Datenpunkte), Auswahl und Filter je Bereich. Teile melden Änderungen; nur betroffene Teile
  zeichnen neu.
- **Liste mit Schlüssel** (`view.js`, `list(container, items, key, row)`): jede Zeile hat einen
  Schlüssel (Zähler-ID, Treffer-Adresse). Beim Nachführen bleibt eine Zeile dasselbe Element,
  wenn sich ihr Inhalt nicht geändert hat; geänderte Zeilen werden einzeln ersetzt, neue
  eingefügt, alte entfernt. Damit bleiben Fokus, Hover und Klick erhalten (Vorgaben 1, 2, 9).
  Etwa 60 Zeilen Code, kein Framework nötig.
- **Kleine Teile** (Kopfzeile, Statusleiste) setzen Text und Attribute gezielt statt
  `innerHTML`.

### 5.3 Daten vom Server

- `/api/status` alle 5 s wie heute (Kopfzeile, Statusleiste, Ausgänge), aber nur geänderte
  Felder zeichnen neu.
- Während eines Auftrags `/api/job` (Fortschritt, Treffer) und Treffer zeilenweise nachführen.
- Werte und Änderungen live über `GET /api/nodered/stream` (Server-Sent Events `value`,
  `changed`): bei `changed` nur den betroffenen Teil nachladen (z.B. eine Zählerzeile), statt
  ganze Listen abzufragen.
- Linie wechseln: `POST /api/lines/{id}/activate`, dann Status, Zählerliste und letzten Scan
  dieser Linie laden; Zustand der Bereiche je Linie im Speicher behalten.

### 5.4 Adresse und Arbeitskontext

- Der Zustand steht in der Adresse: `#/zaehler/15?q=whg&f=err`. Zurück-Taste, Lesezeichen und
  Neuladen landen am selben Ort.
- Auswahl, Filter, Scrollposition und offene Details bleiben je Bereich und Linie erhalten.

### 5.5 Rückfragen, Rückgängig, Meldungen

- **Rückfrage im Kontext** (`ask`): erscheint am Knopf („2 Zähler löschen? [Löschen] [Abbrechen]“),
  Enter bestätigt, Escape bricht ab. Nur für Gefährliches: Löschen, Leeren, Projekt löschen,
  Adresse am Zähler ändern, Dauerbetrieb aus.
- **Rückgängig** statt Rückfrage, wo es geht (Plan geändert, Werte abgewählt, Filter): Meldung
  unten „Plan geändert · Rückgängig“.
- **Eingaben** (neues Projekt, Simulation kopieren) als Feld an Ort und Stelle statt `prompt`.

### 5.6 Einen Bereich bauen (Vorgabe, damit Bereiche unabhängig entstehen können)

- **Eine Datei je Bereich:** `web/neu/<bereich>.js` (`scan`, `zaehler`, `weitergabe`, `anlage`,
  `werkzeuge`, `simulator`) mit `export function mount(pane)`. `app.js` lädt sie selbst, wenn es
  die Datei gibt; an `app.js` ändert ein neuer Bereich nichts. Vorbild: `zaehler.js`.
- **Eigene Stile** in `web/neu/<bereich>.css`, geladen mit `styles("<bereich>")` aus `view.js`,
  alle Regeln beschränkt auf `[data-area=<bereich>]`. Allgemeines (Toolbar, Liste, Details,
  Auswahlleiste, Felder, Chips, Knöpfe) steht in `app.css` und wird benutzt, nicht kopiert.
- **Eigene Texte** am Anfang der Datei mit `texts({…de}, {…en})` aus `view.js`; Schlüssel mit
  dem Präfix des Bereichs (z.B. `an_…` für Anlage). Vorhandene Texte aus `js/texts.js` benutzen,
  wo sie passen.
- **Daten** über `request()` (aus `js/base.js`), Laden in den Zustand über `data.js`
  (`loadStatus`, `loadTree`, `loadMeters`, `reload`, `useLine`, `report`). Eigene Listen eines
  Bereichs darf der Bereich selbst halten.
- **Zeichnen** nur über `html()`, `text()`, `list()` aus `view.js` (nichts ganz neu schreiben,
  was sich nicht geändert hat); Menüs mit `menu()`, Rückfragen mit `ask()`, Meldungen mit
  `toast()` (auch mit `undo`). Kein `confirm`/`prompt`/`alert`.
- **Tasten** mit `bind(taste, funktion, text, () => state.area === "<bereich>")`.
- **Je Linie** Einstellungen des Bereichs (Filter, Auswahl, gewählte Zeile) wie in `zaehler.js`
  in einer `Map` nach Linie halten.
- **Test** `tests/ui/<nr>-neu-<bereich>.test.mjs` (Vorbild `14-neu-zaehler`), ohne
  Browser-Dialoge; zusätzlich muss `13-neu-geruest` grün bleiben (nie seitlich scrollen).
- Screenshots Desktop 1250 px, Handy 390 px, dunkel; nichts darf überlaufen.

## 6. Tastatur

| Taste | Wirkung |
| --- | --- |
| `↑` `↓` | Zeile in der Liste wählen (Details rechts folgen) |
| `Leertaste` | Zeile zur Auswahl hinzu/weg; `Shift`+Klick/`↑↓` für Bereiche |
| `Ctrl`+`A` | alle Zeilen der Liste auswählen (wenn der Fokus in der Liste ist) |
| `Enter` | Hauptaktion der Zeile bzw. Feld bestätigen |
| `Escape` | Feld zurücksetzen → Rückfrage schliessen → Auswahl aufheben → Menü schliessen |
| `/` | Suche des Bereichs |
| `1` … `6` | Bereich wechseln |
| `R` | gewählte Zähler auslesen |
| `S` | Scan starten (im Bereich Scan) |
| `?` | Übersicht der Tasten |

Kürzel stehen im Tooltip der Knöpfe („Auslesen (R)“). Eine Stelle (`keys.js`), keine Kürzel in
Eingabefeldern.

Erklärtexte für Einsteiger (wie früher „Erklärungen anzeigen“): `explain(text)` aus `view.js`
gibt einen Absatz `.explain`, sichtbar nur mit dem Schalter im Menü ⚙ (`body.show-help`,
gespeichert im Browser). Je Bereich einer in der ersten Leiste, bei den Werkzeugen je Werkzeug.
Ohne Schalter bleibt die Oberfläche ruhig; Zustände stehen trotzdem als Wort am Element.

## 7. Server (Rust), klein

1. `/neu/` liefert `web/neu/index.html` (heute nur genaue Pfade; ein Verzeichnis mit `/` am Ende
   soll seine `index.html` liefern).
2. Cache-Header für statische Dateien (`ETag` aus dem Inhalt), damit nach dem ersten Laden nur
   noch geprüft wird. Hilft auf dem Pi über WLAN.
3. Falls die Mehrfachauswahl es braucht: „ausgewählte auslesen“ als ein Auftrag
   (`POST /api/meters/read-all` mit `ids`), damit Fortschritt und Abbrechen wie bei „Alle
   auslesen“ funktionieren. Nur nach Rückfrage.

## 8. Reihenfolge

Jeder Schritt ist für sich nutzbar, mit eigenen UI-Tests (`tests/ui-neu/`, gleicher Rahmen
wie `tests/ui/run.mjs`) und den Messskripten aus dem Audit als Abnahme.

0. **Gerüst:** Route `/neu/`, `tokens.css`, Kopfzeile, Laschen, Statusleiste, Zustand, Liste mit
   Schlüssel, Tastatur, Rückfrage, Rückgängig. Linie wechseln ohne Neuladen.
1. **Zähler:** Liste, Details, Bearbeiten in den Feldern, Mehrfachauswahl, alle/ausgewählte
   auslesen, Verlauf, Werte wählen.
2. **Scan:** Suchzeile, Treffer zeilenweise, „✓ in Liste“, Auffälligkeiten im Detail,
   Beheben, Merken.
3. **Anlage und Dauerbetrieb:** Projektbaum, Formulare im Detail, Zustand der Anlage, Bericht,
   Export/Import, Leeren.
4. **Weitergabe:** Datenpunkte, Ausgänge MQTT/Modbus, Vorlagen, CSV.
5. **Werkzeuge und Simulator.**
6. **Umschalten:** `/` zeigt die neue Oberfläche, die alte bleibt eine Version lang unter
   `/alt/`, dann entfällt sie mit ihren Tests.

## 9. Zusammenarbeit

- Die neue Oberfläche entsteht nur in `web/neu/` (plus `tokens.css` und Ergänzungen in
  `texts.js`). An `web/js` und `web/css` der heutigen Oberfläche nur echte Fehler.
- Server-Änderungen (Abschnitt 7) werden vorher abgesprochen; wer am Server arbeitet, ändert
  die API nicht ohne Eintrag in `api.md`.
- Vor jedem Commit `git status` und `git log`; nur eigene Dateien committen.

## 10. Entschieden (25.10.2026)

Alle Empfehlungen übernommen: kein Framework (JavaScript-Module, eigener Baustein für Listen),
Liste links und Details rechts am Desktop, „Nächster Schritt“ nur als ein Knopf in der
Statusleiste, Tastenkürzel wie Abschnitt 6 (auch Einzeltasten), die neue Oberfläche baut Claude,
die andere KI arbeitet am Server, Server-Punkte 1 und 2 aus Abschnitt 7 gleich mit.

## 11. Stand

**Schritt 0 (Gerüst), umgesetzt:**
- Server: Verzeichnisse liefern ihre `index.html` (`/neu/`), ETag aus dem Inhalt mit
  `Cache-Control: no-cache` (unverändert → 304).
- Gemeinsam mit der heutigen Oberfläche: `css/tokens.css` (Farben, aus `base.css`),
  `js/theme.js` (hell/dunkel vor dem ersten Zeichnen), `slotText`/`parseTime` in `js/base.js`.
- `web/neu/`: `index.html` (`<base href="../">`, damit API, Downloads und gemeinsame Dateien
  ohne Änderung gehen), `store.js` (Zustand, Benachrichtigung nur bei Änderung), `view.js`
  (Liste mit Schlüssel, nur Geändertes schreiben, Menü, Rückfrage am Ort, Meldung mit
  Rückgängig), `keys.js` (Tasten, Escape-Stapel), `app.js` (Kopfzeile, Bereiche, Statusleiste),
  `app.css`.
- Kopfzeile in einer Zeile ab 1000 px: Projekt › Linie (Menü), Anschluss mit genau einer
  Aktion (Verbinden / Ändern / Anschluss festlegen, kleines Formular), Schalter Dauerbetrieb
  (ohne Plan gesperrt mit Grund daneben, Ausschalten mit Rückfrage am Ort), „N im
  Dauerbetrieb“ (orange, wenn nicht als Dienst oder gestört; Liste über alle Projekte), ⚙
  (Sprache, Design, Tastenkürzel, bisherige Oberfläche). Unter 1350 px stehen Gateway und Port
  nur im Menü bzw. in der Statusleiste; unter 1000 px feste Zeilen für das Handy.
- Bereiche als Laschen mit Adresse (`#/zaehler`), Tasten 1–6, `?`. Die Arbeitsfläche zeigt bis
  zum jeweiligen Schritt den Weg in die bisherige Oberfläche.
- Test `tests/ui/13-neu-geruest`: kein Neuladen beim Linienwechsel, Fokus bleibt über die
  Statusabfrage, Escape, Tasten, Rückfrage am Ort, keine Browser-Dialoge, Liste mit Schlüssel.

**Schritt 1 (Zähler), umgesetzt:**
- Liste links (34 px je Zeile: 21 von 30 Zählern sichtbar bei 1250×900, vorher 8), Details rechts;
  Handy: Liste als zweizeilige Karten, Details als eigene Ansicht mit „←“.
- Zustand am Element: Wert mit Zeit oder „noch nicht ausgelesen“, „antwortet nicht“, „gestört“,
  „Uhr +1 h 3 min“, Plan nur wenn einer läuft.
- Suche (`/`), Filter mit Anzahl, Mehrfachauswahl (Kästchen, Leertaste, Shift, Ctrl+A) mit Leiste
  (auslesen, Plan setzen, löschen); Escape hebt die Auswahl auf.
- Details: Name, Ort, Notiz speichern beim Verlassen (Enter), Escape setzt zurück; Plan; letzte
  Auslesung mit allen Werten und Kästchen „als Datenpunkt“; Auslesen (`R`), Löschen mit Rückfrage
  am Ort; „Zähler hinzufügen“ im Detailbereich.
- „Alle N auslesen“ als Auftrag, Zeilen werden einzeln nachgeführt; während des Auftrags „Abbrechen“.
- Gemeinsam: Symbole und `meterBlock` in `js/icons.js` (vorher in `ui.js`).
- Test `tests/ui/14-neu-zaehler`.
- **Noch in der bisherigen Oberfläche:** Verlauf, Zählertausch, Liste importieren, Namen nach Muster,
  CSV-Export. Kommen als Ergänzung zu Schritt 1, bevor umgeschaltet wird.

**Nachgebessert:** Die Seite scrollt bei keiner Breite seitlich (Kopfzeile kürzt lange Texte, der
Grund einer Trennung steht in der Statusleiste, Laschen brechen am Handy um, Listenspalten mit
Mindestbreite für Name, Kennung und Wert); der Gerüst-Test prüft das bei 360–1600 px mit langen
Namen und langer Fehlermeldung.

**Schritt 3 (Anlage), umgesetzt:**
- Projektbaum mit festen Spalten (Name, Anschluss, Zähler, Datenpunkte, Dauerbetrieb · Plan,
  Zustand, Aktionen); jede Spalte fest oder anteilig, damit Projekt-, Gateway- und Linienzeilen
  genau untereinander stehen. Linien eingerückt nur im Namen.
- Zustand der Anlage als Raster oben (nur im Dauerbetrieb): Hintergrund/Dienst, Plan, Uhrzeit,
  Speicher.
- Anlegen von Projekt, Gateway, Linie als Feld an Ort und Stelle (Enter, Escape); Details rechts mit
  Speichern beim Verlassen; Dauerbetrieb-Schalter in der Zeile; Linie wechseln ohne Neuladen;
  Löschen mit Rückfrage am Ort; Bericht, Export, Import.
- Test `tests/ui/18-neu-anlage`.

**Schritt 5 (Werkzeuge), umgesetzt:**
- Ein Ziel für alle Werkzeuge (Suche über Zählerliste oder Adresse, ↑↓ Enter); ohne Ziel gesperrt
  mit Grund. Auslesen (`R`), Primäradresse (freie suchen, Rückfrage am Ort), Baudrate (umstellen,
  suchen), Reset; Telegramm-Log mit Analyse, Rohtelegramm senden.
- Nach „Adresse ändern“ / „Baudrate ändern“ führt der Server die Zählerliste nach
  (`follow_meter_change`), für alte und neue Oberfläche gleich.
- Test `tests/ui/19-neu-werkzeuge`.

**Schritt 2 (Scan), umgesetzt:**
- Suchzeile: Art (PA/ID), Adressbereich bzw. „ID beginnt mit“, Baudraten als Menü mit „Prüfen,
  welche antworten“, Dauer, **[Primärscan starten]** (`S`), während des Scans Fortschritt und
  Abbrechen; Weitersuchen nach einem abgebrochenen Sekundärscan.
- Treffer zeilenweise nachgeführt (Klick während des Scans geht nicht verloren, im Test
  geprüft); 16 Treffer sichtbar bei 1250×900 (vorher 3). Zustand als Wort: Kollision, Keine
  Daten, PA nicht eindeutig, andere Baudrate, ID doppelt, neu, ✓ in Liste; Vergleich mit dem
  letzten Scan als Chips.
- Genau ein Hauptknopf: **N beheben** (Plan mit freien Adressen, Kästchen, Ausführen; danach
  **Zur Kontrolle neu scannen**) → **N neue merken** (`M`) → „✓ Alle in der Zählerliste“.
- Details: Ursache und Abhilfe, Knöpfe je Fall (Sekundärscan, freie PA geben, auf Bus-Baudrate
  umstellen, jeweils mit Rückfrage am Ort), Merken, Auslesen mit Werten, Rohtelegramm.
- Merken über PA/ID umschaltbar (Rückfrage am Ort, wenn Zähler in der Liste sind).
- Ohne Adresse startet die Oberfläche im Scan, wenn die Linie noch keine Zähler hat, sonst bei
  den Zählern.
- Gemeinsame Auswertung `js/scanlogic.js` (auch von der bisherigen Oberfläche benutzt); „in
  Liste“ bei Merken über PA nur, wenn dort nicht ein anderer Zähler (andere ID) steht.
- Test `tests/ui/15-neu-scan`.

**Schritt 4 (Weitergabe), umgesetzt:**
- Oben die Ausgänge (MQTT, Modbus) je in einer Zeile: Schalter, Zustand als Wort (läuft = grün,
  gestört = rot mit Grund, verbindet, aus), Einstellungen im Detailbereich (die ganze gespeicherte
  Einstellung wird zurückgeschickt), Adressen nach Muster mit Vorschau, Liste für den Integrator
  (CSV, JSON). Einschalten ohne Broker öffnet die Einstellungen und schaltet nach dem Speichern ein;
  Ausschalten mit Rückfrage am Ort.
- Datenpunkte als dichte Liste (Nr, Name, Wert, Qualität als Wort, Ausgänge, Sendeart), Suche,
  Linie/Projekt, Filter „Probleme“; Details: Name, Faktor, Verschiebung, Senden, aktiv, je Ausgang
  ein/aus und Adresse, speichern beim Verlassen, Escape setzt zurück.
- Mehrfachauswahl mit Leiste: ein/aus, Senden, je Ausgang ein/aus, löschen (Rückfrage).
- Vorlagen im Menü (löschen mit Rückfrage), CSV-Export und -Import mit Vorschau.
- Gemeinsam: `parseTable` in `js/base.js`; `changeSet`/`previewApply` in `view.js`.
- Test `tests/ui/20-neu-weitergabe`.

**Simulator, umgesetzt:**
- Oben die Simulation (Auswahl, „Vorlage“, „von der aktiven Linie benutzt“), „Für aktive Linie
  verwenden“, Kopieren und Neu mit Namensfeld im Menü (statt prompt), Löschen mit Rückfrage am
  Ort (gesperrt, solange die aktive Linie sie benutzt).
- Links die Zähler (PA, ID, Hersteller, Medium mit Symbol, Baudrate, Verhalten als Chip), rechts
  der gewählte Zähler zum Bearbeiten; Vorlagen nur zum Ansehen.
- Änderungen sammeln sich: „Ungespeicherte Änderungen · Verwerfen · **Speichern**“ (dann der eine
  Hauptknopf); nie still mit Escape verwerfen. Hinweise (doppelte PA/ID, PA 0) als ein Chip.
- Server: Zähler mit fester Struktur (`heat_old`) dürfen ohne Hersteller gespeichert werden;
  vorher liess sich keine Kopie der Vorlage „Praxis“ speichern. Test: jede Vorlage besteht die
  Prüfung.
- Test `tests/ui/21-neu-simulator`.

**Rest der Zähler, umgesetzt:**
- Details: **Verlauf** (Taste `V`) als Diagramm mit Reihe zur Auswahl und CSV; **Zähler getauscht …**
  als Feld mit Rückfrage am Ort (statt prompt/confirm).
- Menü „Liste“: **Liste importieren** (Datei oder aus Excel einfügen, Überschrift und Spalten
  werden erkannt, zuordnen, Vorschau, übernehmen), **Namen erzeugen** (Muster, nur ohne Namen, für
  die Auswahl oder alle, Vorschau), **Als CSV exportieren**.
- Gemeinsam: `js/plot.js` + `css/plot.css` (Diagramm, Reihe, CSV), `js/listlogic.js`
  (Spaltenzuordnung der Kundenliste).
- Server: Verlauf trennt Untereinheiten (Volumen an Eingang 1 und 2 sind eigene Reihen; vorher in
  einer Linie gemischt).

**Feinschliff, umgesetzt:**
- **Nächster Schritt** als ein Knopf rechts in der Statusleiste, aus dem Zustand abgeleitet
  (Anschluss festlegen → Verbinden → Zähler suchen → auslesen → Gestörte zeigen → Bericht; im
  Dauerbetrieb Werte wählen → Ausgang einrichten). Er führt an die Stelle, an der man es selbst
  täte; Erklärung im Tooltip. Am Handy ohne das Wort „Nächster Schritt“.
- **Tastenübersicht** (`?` oder ⚙): Kasten mit allen Tasten, die gerade wirken; Escape schliesst.
- Durchgang über alle Bereiche (Desktop, Handy, dunkel): kein seitliches Scrollen, keine Fehler im
  Browser; Anlage: Spalte „Zustand“ mit Mindestbreite.

**Umgeschaltet (25.10.2026):** die neue Oberfläche ist `web/index.html` unter `/`, `/neu/` leitet
auf `/` weiter.

**Bisherige Oberfläche entfernt (26.10.2026),** samt Dateien, Tests und 255 nur dort benutzten
Texten. Ihre Tests, die etwas prüften, was die neuen nicht abdeckten, sind für die neue Oberfläche
neu geschrieben: Sprachen (alle Bereiche, Englisch ohne Deutsch), Ausleseplan mit Uhrzeit,
Scan-Vergleich und Sekundärscan mit Weitersuchen, Projekt leeren, Import, Modbus, MQTT, Nächster
Schritt bis zum Bericht und Ausgang. Dabei dazugekommen: Zugriffsschlüssel in einem Feld über der
Seite statt Browser-Fenster (`askToken` in `base.js`, gilt auch für den Bericht), Kurzformen der
Uhrzeit im Plan (`time24`), „fehlt“ im Scan zeigt im Tooltip welche, Zähler ohne Plan zeigen
„aus“, Werkzeuge → Simulator. Jeder Browser-Dialog lässt einen Test fehlschlagen (`run.mjs`).

**Nach dem Umschalten (26.10.2026), entschieden:**
- **Laschen in Gruppen:** Anlage (einrichten) │ Scan, Zähler, Weitergabe (Arbeitsablauf, in dieser
  Reihenfolge) │ Werkzeuge, Simulator (Hilfsmittel). Strich zwischen den Gruppen; schmal brechen
  nur ganze Gruppen um. Tasten 1–6 folgen dieser Reihenfolge.
- **Statusleiste zum Anklicken:** der Bus öffnet die **Bus-Konsole** (eigene Zeile über der
  Statusleiste, die Arbeitsfläche wird kürzer statt verdeckt; Telegramme live, ein Telegramm
  anklicken erklärt es in Werkzeuge); die Auffälligkeiten führen in den Scan (Filter „Auffällig“,
  die erste gewählt).
- **Eine Zahl, eine Regel:** was an mehreren Stellen steht, kommt aus einer Funktion.
  Auffälligkeiten = betroffene Treffer (`scanLogic.attention`; Statusleiste, Anlage, Filter;
  nach jedem Beheben neu gespeichert), gestört = `faulty`, Zustand eines Ausgangs = `outputWord`,
  Dauerbetrieb = `opText`/`opPossible`/`switchOperation`. Neue Anzeigen nehmen diese, statt
  selbst zu rechnen.
- **Doppelte PA:** im Primärscan als Kollision sichtbar; Hauptknopf „N doppelte PA auflösen“
  startet den Sekundärscan, danach „N beheben“. Ausführen erst, wenn alle freien Adressen
  gefunden sind.
- **Meldungen des Servers** (Protokoll, Hintergrund) in der Sprache der Oberfläche: Protokoll
  speichert beide Sprachen (`i18n::both`), Arbeit im Hintergrund folgt der zuletzt gemeldeten
  Sprache (`X-Lang`, gespeichert in `meta.ui_lang`).
- **Windows:** Browser auf `127.0.0.1` statt `localhost`; von Windows gesperrter Port → der
  nächste freie. Felder mit Textfarbe aus dem Design (sonst Systemfarbe im dunklen Design).
- **Tests sehen, was der Benutzer sieht:** Playwright klickt auch verdeckte Elemente; wo etwas
  aufklappt, prüft der Test mit `elementFromPoint`, dass es sichtbar ist (Gerüst-Test, Liste der
  Linien).

**Geteilt (26.10.2026),** ohne Verhaltensänderung: anlage 727 → 431, zaehler 716 → 487,
weitergabe 604 → 453, scan 569 → 457, werkzeuge 525 → 464, app 462 → 143 Zeilen. Was in den
Hauptdateien bleibt, ist Ablauf und Bedienung eines Bereichs; weiter zu teilen hiesse, eng
Zusammengehöriges zu trennen.

**Noch offen:** Test mit echten Zählern, sobald eine Anlage da ist.
