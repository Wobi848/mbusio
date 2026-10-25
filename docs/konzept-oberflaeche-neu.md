# Konzept: neue Oberfläche (Werkzeug statt Webseite)

Stand: 25.10.2026 · Status: **in Arbeit** – Schritt 0 (Gerüst) umgesetzt, Entscheidungen in Abschnitt 10

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
web/neu/index.html      Gerüst: Kopfzeile, Laschen, Arbeitsfläche, Statusleiste
web/neu/app.js          Start, Bereiche umschalten, Adresse (#/zaehler/15) ↔ Zustand
web/neu/store.js        Zustand und Benachrichtigung (ein Objekt, kleine Abonnements)
web/neu/api.js          fetch, Zugriffsschlüssel, Sprache, Fehler; Abfragen und Strom
web/neu/view.js         Bausteine: Liste mit Schlüssel, Auswahl, Rückfrage, Rückgängig, Felder
web/neu/keys.js         Tastatur: eine Stelle für alle Kürzel
web/neu/<bereich>.js    scan.js, meters.js, transfer.js, plant.js, tools.js, sim.js
web/neu/app.css         Gerüst und Bausteine; Farben aus den gemeinsamen Variablen
```

Gemeinsam mit der heutigen Oberfläche (nichts doppelt): `web/js/texts.js` (Texte DE/EN, wird
erweitert), `web/js/base.js` (`t`, `label`, `esc`, `portText`, …) und die Farbvariablen. Diese
kommen aus `base.css` in eine eigene `web/css/tokens.css`, die beide Oberflächen laden.

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

**Nächster Schritt:** 1 (Zähler).
