# Übergabe: Oberfläche v2 – „Was gehört wozu“

**Status: umgesetzt am 18.10.2026, abgelöst am 26.10.2026** durch die neue Oberfläche
([konzept-oberflaeche-neu.md](konzept-oberflaeche-neu.md)). Nur noch zur Geschichte.

Entwurf (Seite „v2 · Was gehört wozu“): https://claude.ai/artifact/YWf4vE2MMjHVDWHVMmP8mF
Drei Artboards: **Zählerliste** (klickbar), **Scan**, **Bausteine**. Farben, Abstände, Knöpfe sind die
heutigen aus `web/css/base.css` – es ändert sich die **Gliederung**, nicht der Stil.

Ziel: Man sieht immer, *für welche Linie* etwas gilt, *welcher Zähler* gemeint ist und *welche Aktion
was betrifft*. Kein Funktionsverlust; alle Browser-Tests (`node tests/ui/run.mjs`) bleiben grün
(Selektoren anpassen, wo sich Aufbau ändert; neue Tests für die neuen Teile).

---

## 1. Drei Ebenen statt einer Reiterleiste

```
Kopfzeile (Projekt)   mbusio │ PROJEKT Wohnanlage Seeweg ▾ │ Anlage        Werkzeuge  Simulator  ⚙
Linien-Leiste (grau)  LINIE [Haus A ▾ Gateway Keller]  ● Verbunden SIMULATOR·2400 Bd  [Anschluss ändern]   Plan …
                      ┌ Scan ✓ ┐┌ Zähler 5 ┐┌ Weitergabe 2 ┐   ← Laschen, gehen in die Inhaltskarte über
Inhalt                └──────── Karte des Reiters ─────────────────────────────────────┘
```

- **Kopfzeile = Projekt-Ebene:** Projekt-Auswahl (heute `#project-select`), „Anlage“ (heutiger Reiter
  Projekt: Gateways, Linien, Bericht, Export/Import), rechts abgesetzt Werkzeuge und Simulator
  (gelten für den Bus bzw. global), dann ein **Zahnrad-Menü** mit Hilfe (heute `#help-toggle`),
  Design (Theme) und Sprache. Der Pfad-Chip `#crumb` entfällt (steckt in der Linien-Leiste).
- **Linien-Leiste = Linien-Ebene** (neu, unter der Kopfzeile, Hintergrund `--sunken`):
  Linie wählen (Name + Gateway darunter, öffnet Auswahl der Linien im Projekt), Verbindungsstatus,
  Anschluss, Ausleseplan. **Die Verbindung zieht aus dem Reiter Scan hierher** (`#sec-conn` fällt dort
  weg; „Anschluss ändern“ klappt das heutige Verbindungsformular unter der Leiste auf).
  Zustände (Artboard Bausteine): kein Anschluss → Knopf „Anschluss festlegen“ (primär);
  getrennt → „Verbinden“ (primär); verbunden → „Anschluss ändern“ (leise); Scan/Auslesen läuft →
  Fortschritt + „Abbrechen“. Der rechte Knopf ist immer der nächste Schritt.
- **Reiter der Linie:** Scan, Zähler, Weitergabe als Laschen, die nahtlos in die Inhaltskarte
  übergehen (aktive Lasche weiss, Karte ohne oberen Rand). Zahl = Anzahl Zähler bzw. Datenpunkte;
  ✓ = erledigt, Punkt = nächster Schritt (Logik aus `web/js/guide.js` bleibt).
- Die Karte „Nächster Schritt“ (`#guide`) bleibt, sitzt aber **in** der Inhaltskarte oben bzw. entfällt
  für die Schritte, die die Linien-Leiste schon zeigt (Anschluss, Verbinden).

## 2. Ein Zähler-Baustein überall

Neu in `web/js/ui.js`: `ui.meter(m, { size: "row" | "short" })`
- **row:** Medium-Symbol (34 px, abgerundetes Quadrat, Farbe je Medium) + Name fett
  (fehlt → *„ohne Namen“* kursiv grau) + darunter mono klein „ID 12345678 · PA 1 · KAM“.
  Optional rechts ein Status-Chip (Keine Antwort, noch nicht in der Liste …), roter Punkt am Symbol bei Störung.
- **short:** runde Pille mit kleinem Symbol + Name (oder ID), als Link zum Zähler – für Texte,
  Toasts, Weitergabe-Kopfzeilen, Bericht.
- Medium → Farbe/Symbol (hell; im Dunkelmodus Fläche ~15 % Deckkraft, Symbol heller):
  Wärme `#c2410c`/`#fdeee6` Flamme · Kälte `#0369a1`/`#e6f3fb` Stern · Kaltwasser `#1d4ed8`/`#e8efff`
  Tropfen · Warmwasser `#be123c`/`#fde8ee` Tropfen mit Welle · Strom `#a16207`/`#fdf4d6` Blitz ·
  Gas `#6d28d9`/`#f0eaff` · Rest grau. Inline-SVG, Strich 2 px (Pfade im Artboard Bausteine).
  Zuordnung über den Medium-Schlüssel vom Server (`medium_heat_outlet` → Wärme usw.).
- Einsetzen in: Trefferliste (Scan), Zählerliste, Weitergabe (Kopfzeile je Zähler statt nur ID),
  Werkzeuge (Ziel), Verlauf, Bericht.

## 3. Aktionen direkt beim Ding

**Zählerliste** (Artboard Zählerliste):
- Kopf der Karte: Titel + „5 Zähler auf Linie Haus A · zuletzt ausgelesen …“; rechts nur Seltenes
  (Liste importieren, Bericht).
- **Leiste direkt an der Tabelle** (grau): Suche (Name, Ort, ID), Filter Alle / Gestört / Ohne Namen
  (mit Anzahl), rechts „+ Zähler“ und „Alle N auslesen“ (primär).
- Spalten: Zähler (Baustein) · Ort · Letzter Wert (Wert fett + Grösse·Zeit; Fehler als roter Chip) ·
  Plan („wie Linie“ grau, Abweichung schwarz) · Aktionen: Auslesen, Weitergabe + Anzahl Datenpunkte, ✎.
- **Bearbeiten klappt in der Zeile auf** (Zeile hell akzentuiert, Formular direkt darunter, gleiche
  Einrückung): Name, Ort, Baudrate, Ausleseplan, Notiz; Speichern / Abbrechen; rechts Zählertausch …,
  Aus Liste entfernen. Ersetzt `#meter-form` über der Tabelle. Gleiches Prinzip im Projektbaum
  (Gateway/Linie bearbeiten unter der Zeile statt `#tree-form` unter dem Baum).

**Scan** (Artboard Scan):
- Zwei nummerierte Blöcke in einer Karte: **1 Suche einstellen** (eine Zeile: Art Primär/Sekundär
  als Umschalter, Baudraten-Chips + „Prüfen, welche antworten“, Von/Bis/Wiederholung, Start) und
  **2 Ergebnis** (Kopf: Art, Zeit, Anzahl; Filter Alle / Eindeutig / Auffällig).
- Darunter ein Satz mit der Hauptaktion: „9 Zähler sind eindeutig … [6 beheben …] [9 Zähler übernehmen →]“.
- **Auffälligkeit an ihrer Zeile:** heller Warn-Streifen direkt unter der betroffenen Zeile mit
  Erklärung (fett: was, normal: was tun) und dem Knopf, der es behebt (Sekundärscan starten,
  Erneut lesen, Telegramm ansehen, Beide über PA merken …). Ersetzt den Sammelkasten `#hits-warn`
  über der Tabelle. Betrifft eine Auffälligkeit mehrere Zeilen (gleiche ID): Klammer links über die
  Zeilen, gemeinsamer Wert markiert, Erklärung einmal darunter.
- „Übernehmen“ je eindeutiger Zeile; nicht eindeutige Zeilen haben keinen Übernehmen-Knopf.

**Werkzeuge** (Artboard Bausteine): oben ein Ziel-Kopf „Ziel: [Zähler-Baustein] [Anderen Zähler
wählen]“; Primäradresse / Baudrate / Reset hängen sichtbar darunter (eingerückt mit Linie links),
zeigen „alt → neu“ und sind gesperrt, solange kein Ziel gewählt ist.

## 4. Weniger Rahmen

Nur noch **eine** Rahmen-Ebene (die Karte). Innen gliedern Abstand, kleine nummerierte
Zwischenüberschriften und Trennlinien – keine Kacheln mit eigenem Rand in Karten (`.tile`) mehr.

## 5. Reihenfolge (je ein Commit, Tests grün)

1. `ui.meter` + Medium-Symbole; in Trefferliste, Zählerliste, Weitergabe einsetzen.
2. Kopfzeile + Linien-Leiste + Laschen; Verbindung aus dem Scan in die Leiste; Zahnrad-Menü.
3. Zählerliste: Tabellen-Leiste, Filter, Bearbeiten in der Zeile (auch Projektbaum).
4. Scan: zwei Blöcke, Auffälligkeiten an der Zeile.
5. Werkzeuge: Ziel-Kopf. Rahmen aufräumen.
6. Handy (≤ 640 px): Linien-Leiste in zwei Zeilen, Laschen scrollbar, Zähler-Zeilen als Karten,
   Bearbeiten in der Karte. Dark Mode prüfen. `docs/konzept-bedienung.md` ergänzen.

Animationen wie bisher nur über `transform`/`opacity` (Aufklappen der Zeile, Warn-Streifen
einblenden), aus bei `prefers-reduced-motion`.
