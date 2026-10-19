# Änderungen

Was sich für Benutzer ändert, je Version. Neuestes oben.

## Unveröffentlicht (wird v0.5.0)

Erscheint nach den Hardware-Tests (7 Tage Raspberry Pi, Stromausfall, USB-Wandler abziehen, kleine
SD-Karte, MQTT mit echtem Broker), siehe `docs/konzept-dauerbetrieb.md`, Abschnitt 11.

### Neu

- **MQTT-Topic-Muster:** Aufbau der Topics in den MQTT-Einstellungen festlegen; „Alle Topics nach
  Muster setzen …“ mit Vorschau statt jedes einzeln anzupassen.
- **Topic-Liste für den Integrator** als CSV und JSON (volles Topic, Name, Einheit, Zähler, ID,
  PA, Ort; im JSON Status-Topic, QoS, retained, Aufbau der Nachricht).
- **Windows:** die .exe hat das mbusio-Icon (Explorer, Taskleiste).
- **Alle auslesen** sagt am Ende, wie viele Zähler antworten und wie viele nicht.
- **Nächster Schritt** meldet „N Zähler antworten nicht“ statt „Linie in Ordnung“, solange gemerkte
  Zähler gestört sind; der Knopf öffnet die Zählerliste mit dem Filter „Gestört“.
- **Sekundärscan eingrenzen und weitersuchen:** „ID beginnt mit …“ sucht nur IDs mit diesem
  Anfang; nach einem Abbruch macht „Weitersuchen“ dort weiter, die bisherigen Treffer bleiben.
- **Änderungen seit dem letzten Scan:** nach jedem vollständigen Scan Vergleich mit dem letzten
  Scan gleicher Art und gleichen Bereichs auf der Linie: neu, fehlt (graue Zeile), geändert
  (PA, Baudrate, Status als alt → neu), mit Zusammenfassung über der Trefferliste.
- **Geplantes Auslesen (Dauerbetrieb):** Ausleseplan je Projekt, Linie oder Zähler (aus, monatlich,
  täglich, stündlich, alle 15 Minuten; Standard aus). Jede Linie hat ihre eigene Verbindung und
  Warteschlange; Bedienung von Hand geht immer vor. Verpasste Termine werden einmal nachgeholt,
  Zähler mit Störung oder „zu viele Auslesungen“ werden markiert und geschont.
- **Uhrzeit:** Auslesungen ohne bestätigte Uhrzeit werden als „Zeit unsicher“ markiert; Uhrzeit aus
  dem Browser, vom System (NTP) oder vom Leitsystem übernehmen.
- **Speicher:** kompakte Speicherung, Messwerte auf Wunsch ausdünnen, Schutz vor vollem Speicher.
- **Datenpunkte:** je Zähler wählen, welche Werte weitergegeben werden; Vorlagen je Linie und Medium,
  feste Nummern, Qualität (gut, veraltet, gestört, ungültig), Prüfung „Zählerstand gesunken“,
  Zählertausch mit durchlaufendem Zählerstand, CSV-Export und -Import mit Vorschau.
- **MQTT-Ausgang:** ein Topic je Datenpunkt (JSON mit Wert, Einheit, UTC-Zeit, Qualität), QoS 1,
  retained, Status-Topic mit Last Will, Warteschlange in der Datenbank (wird nach einem Ausfall in
  richtiger Reihenfolge nachgeliefert), TLS, Benutzer/Passwort, Uhrzeit vom Leitsystem.
- **Reiter Weitergabe:** was (Datenpunkte) wohin (Ausgänge) und wie oft, an einer Stelle; auch aus
  der Auslesung heraus direkt wählbar („Senden“).
- **Zählerliste schnell füllen:** Liste vom Kunden importieren (CSV oder aus Excel kopiert, Zuordnung
  über die ID, mit Vorschau), Name und Ort direkt in der Tabelle bearbeiten, Namen nach Muster
  erzeugen, Zählerliste als CSV.
- **Simulator-Reiter:** Vorlagen (Praxis mit Fehlern, alles i.O. mit 10 Zählern, Mehrfamilienhaus mit
  30 Zählern, gemischte Baudraten) und eigene, frei zusammengestellte Anlagen.
- **Nächster Schritt:** eine Karte mit genau einem Knopf führt vom leeren Projekt bis „Linie in
  Ordnung“ mit Bericht (Service); auf Wunsch weiter mit Plan, Werten und Ausgang (Dauerbetrieb).
  Die Reiter zeigen den Fortschritt.
- **Service und Dauerbetrieb ohne Modus:** ein Projekt gilt als Dauerbetrieb, sobald ein Plan, ein
  Datenpunkt oder ein Ausgang eingerichtet ist; erst dann wird die Weitergabe Reiter 4 und der
  Zustand der Anlage erscheint.
- **Zustand der Anlage** im Reiter Projekt: Plan, Uhrzeit, Speicher, MQTT auf einen Blick.

### Geändert

- Knöpfe: weitere wichtige Aktionen in der Sekundärfarbe (MQTT-Einstellungen, + Linie/Gateway,
  Benennen, Baudrate suchen, Simulator kopieren); „Alle Auslesungen löschen“ rot; Menü-Einträge
  immer leise (Ziel-Menü in den Werkzeugen war unlesbar).
- Probleme in zwei Farben: orange = Hinweis, Zähler kommt mit; rot = Fehler, bleibt draussen.
  Auch die Zählerliste hebt Zähler hervor, die nicht antworten.
- Datenpunkt-Tabelle: lange Topics von links gekürzt (Ende sichtbar, volles im Tooltip), passt auf Full HD.
- **Oberfläche v2:** Kopfzeile für das Projekt, Linien-Leiste mit Verbindung und Plan, Laschen
  Scan · Zähler · Weitergabe; Zähler-Baustein mit Medium-Symbol überall; Zählerliste mit Suche,
  Filtern und Bearbeiten in der Zeile; Scan in zwei Schritten mit Auffälligkeiten direkt an der
  Zeile; Werkzeuge mit Ziel zuerst; Einstellungen (Erklärungen, Design, Sprache) unter ⚙; weniger
  Rahmen; Handy-Ansicht mit Karten.
- Reiter Simulator nur, solange die aktive Linie eine Simulation benutzt; sonst unter Werkzeuge.
- Nächster Schritt steht in der Leiste unten (immer sichtbar, aufklappbar) statt als Karte oben.
- Nach einem Scan mit behebbaren Auffälligkeiten heisst der nächste Schritt „Auffälligkeiten beheben“
  (übernehmen geht trotzdem), danach „Zur Kontrolle neu scannen“.
- Trefferliste: Zeilen mit Problem sind hervorgehoben; „Keine Daten“ als Warnung statt grau.
- Neue Linien merken über PA (vorher SA); leere bestehende Linien werden einmal umgestellt.
- „Neues Projekt“ steht neben der Projektauswahl.
- Uhrzeiten immer 24 h (HH:MM).
- Die Oberfläche nutzt die ganze Fensterbreite.
- Knöpfe: Hauptknöpfe in der Akzentfarbe, gesperrte grau mit Grund im Tooltip; Felder etwas grösser.
- Zählerliste mit Menü „Liste ▾“ (Importieren, Exportieren, Leeren).

### Entfernt

- Live-Auslesen (widerspricht dem Grundsatz, M-Bus-Zähler sparsam auszulesen; batteriebetriebene
  Zähler).

### Behoben

- Nach „Verbinden“ zeigte die Linie im Reiter Projekt weiter „Anschluss noch nicht festgelegt“.
- Die Statusleiste zeigte bei einem laufenden Ausleseplan „Behebe …“ statt „Plan … liest“.
- „klein“ gestaltete Links und Datei-Knöpfe waren grösser als die übrigen kleinen Knöpfe.

### Intern

- Code nach Bereichen aufgeteilt (Server, Datenbank, Datenpunkte, Simulator, M-Bus-Master, MQTT,
  Oberfläche, Stile), Doppeltes zusammengelegt; gemeinsame Bausteine für die Oberfläche (`ui.js`).
- Browser-Tests der Oberfläche (`tests/ui`), laufen in der CI zusammen mit Format- und
  Rust-Tests.

## v0.4.0 (17.10.2026)

- Projekte, Gateways und Linien, gespeichert in einer SQLite-Datenbank (`mbusio.db`)
- Oberfläche nach Arbeitsschritten: Reiter statt einer langen Seite
- Zählerliste je Linie, gemerkt über SA oder PA (je Linie einstellbar, keine gemischten Listen)
- Verlauf je Zähler (Diagramm, Tabelle), sortierbare Trefferliste, Gateways und Linien sortierbar
- Auffälligkeiten je Linie gemerkt (Projekt-Karte, Bericht); im Bericht ausblendbar

## v0.3.0 (17.10.2026)

- Neu gestaltete Oberfläche, hell und dunkel; Statusleiste unten mit Zähler der Auffälligkeiten
- Scan über mehrere Baudraten (Busse mit gemischten Baudraten), Prüfen, wo Geräte antworten,
  Dauer-Schätzung
- Auffälligkeiten erkennen: doppelte Primäradressen und IDs, Kollisionen, fremde Baudrate
- „Warum?“: Ursache in einfachen Worten; „Alle beheben“: Plan zum Abhaken
- Telegramm-Analyse Byte für Byte, auch für gestörte Telegramme
- Bericht als PDF, „Alle merken“, Zählerliste leeren, „Neue Anlage“
- Auslesen über eine Primäradresse bricht ab, wenn mehrere Zähler antworten
- Windows ohne Adminrechte, portabler Datenordner

## v0.2.0 (17.10.2026)

- M-Bus-Master mit Weboberfläche und Simulator
- Funktionsumfang wie libmbus: Primär- und Sekundärscan, Auslesen, Primäradresse setzen, Baudrate
  suchen und umstellen, Application Reset, Rohtelegramme
- Zählerliste, Deutsch und Englisch, Schutz für den Zugriff aus dem LAN, Builds für Windows, Linux
  und Raspberry Pi
