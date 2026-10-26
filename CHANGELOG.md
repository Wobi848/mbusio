# Änderungen

Was sich für Benutzer ändert, je Version. Neuestes oben.

## Unveröffentlicht (wird v0.5.0)

Erscheint nach den Hardware-Tests (7 Tage Raspberry Pi, Stromausfall, USB-Wandler abziehen, kleine
SD-Karte, MQTT mit echtem Broker), siehe `docs/konzept-dauerbetrieb.md`, Abschnitt 11.

### Neu

- **Neue Oberfläche** (Konzept `docs/konzept-oberflaeche-neu.md`): Werkzeug statt Webseite. Eine
  Kopfzeile, Liste und Details nebeneinander (etwa doppelt bis fünfmal so viele Zeilen ohne
  Scrollen), Zustand als Wort am Element, genau ein Hauptknopf und „Nächster Schritt“ in der
  Statusleiste, Mehrfachauswahl, Tastatur, Rückfragen am Knopf statt Browser-Fenster, Wechsel von
  Linie und Projekt ohne Neuladen, Zeilen werden während eines Scans nicht mehr ersetzt (keine
  verlorenen Klicks). Die bisherige Oberfläche ist entfernt; alte Lesezeichen auf `/neu/` führen
  zur Startseite.
- **Protokoll und Meldungen in der eingestellten Sprache:** Meldungen aus dem Hintergrund
  (Zeitplan, MQTT, Modbus, Uhrzeit, Linie getrennt) waren immer deutsch; jetzt folgen sie der
  Sprache der Oberfläche, Einträge im Protokoll auch nach einem Sprachwechsel.
- **Zugriffsschlüssel** (`--token`) wird in einem Feld auf der Seite abgefragt statt in einem
  Browser-Fenster; ein falscher wird als falsch gemeldet.
- **Schnelleres Neuladen:** die Dateien der Oberfläche kommen mit ETag; unverändert antwortet
  mbusio mit 304 (spürbar auf dem Raspberry Pi über WLAN).

- **Modbus TCP:** mbusio ist Modbus-Server für Leitsystem, SPS oder Loxone. Je Datenpunkt das
  Häkchen Modbus; 10 Register ab (Nummer − 1) × 10 mit Wert (float64, float32, int32 × 100),
  Qualität und Alter; ab Register 60000 der Zustand; nur lesen; Registerliste für den Integrator.
  Vorgabe: aus.
- **Schnittstelle für Node-RED** (ohne MQTT-Broker): `/api/nodered/tree` liefert Projekte,
  Gateways, Linien, Zähler und Datenpunkte mit aktuellem Wert, `/api/nodered/stream` die
  laufenden Werte als Server-Sent Events.
- **Node-RED-Nodes** im Ordner `node-red/` (node-red-contrib-mbusio, noch nicht auf npm):
  Projekt, Gateway, Linie, Zähler und Datenpunkte im Editor aus Listen wählen, Werte mit Qualität
  und Status am Node; „mbusio status“ meldet, ob in einem Projekt oder einer Linie alles in
  Ordnung ist (für Alarme und Benachrichtigungen).
- **Home Assistant (MQTT Discovery):** neues Häkchen in den MQTT-Einstellungen. Datenpunkte mit
  MQTT-Häkchen erscheinen in Home Assistant von selbst (je Zähler ein Gerät, Einheit, Typ für das
  Energie-Dashboard); Häkchen weg entfernt den Sensor wieder. Vorgabe: aus.
- **MQTT-Topic-Muster:** Aufbau der Topics in den MQTT-Einstellungen festlegen; „Alle Topics nach
  Muster setzen …“ mit Vorschau statt jedes einzeln anzupassen.
- **Topic-Liste für den Integrator** als CSV und JSON (volles Topic, Name, Einheit, Zähler, ID,
  PA, Ort; im JSON Status-Topic, QoS, retained, Aufbau der Nachricht).
- **Windows:** die .exe hat das mbusio-Icon (Explorer, Taskleiste).
- **Alle auslesen** sagt am Ende, wie viele Zähler antworten und wie viele nicht.
- **Projekt leeren:** „Leeren …“ in den Projekt-Werkzeugen löscht Zähler, Auslesungen, Änderungen
  und den letzten Scan aller Linien; Linien, Anschluss, Plan und Ausgänge bleiben. Für den nächsten
  Einsatz mit demselben Projekt, ohne Reste der vorigen Anlage.
- **Neue Installation startet mit dem Projekt „Schnellscan“** statt „Standard“: verbinden und
  scannen, ohne zuerst ein Projekt anzulegen.
- **Kälteenergie getrennt:** Wärme-/Kältezähler liefern Wärme- und Kälteenergie mit gleicher
  Grösse; die Kälteenergie (VIFE „nur negative Beiträge“) heisst jetzt „Kälteenergie“ und lässt
  sich als eigener Datenpunkt wählen. Bisher war sie nicht wählbar (gefunden an einem Itron).
- **Uhr im Zähler:** weicht sie 5 min oder mehr ab, zeigt die Zählerliste „Uhr +1 h 3 min“ mit
  Erklärung (Stichtagswerte richten sich nach der Zähleruhr).
- **USB-Wandler abgezogen:** die Linie steht nach höchstens 10 s auf „getrennt“, nicht erst beim
  nächsten Termin; Meldungen sagen „nicht vorhanden – USB-Wandler abgezogen?“ statt „I/O error“.
- **Fehler im Zählerstatus:** meldet der Zähler einen Fehler (Statusbyte), gelten nur Messwerte
  (Temperatur, Leistung, Durchfluss) als ungültig; Zählerstände (Energie, Volumen) bleiben gültig.
  Werte, die der Zähler selbst als Fehlerwert kennzeichnet, sind sofort ungültig statt nach drei
  Abständen veraltet. Geprüft an einem Itron-Wärmezähler ohne Durchfluss.
- **Dauerbetrieb je Linie einschalten:** automatisch ausgelesen und weitergegeben wird nur, wo
  der Dauerbetrieb der Linie eingeschaltet ist (Linien-Leiste, „Nächster Schritt“). Ein Plan allein
  liest nicht mehr, Auslesen von Hand in anderen Projekten sendet nichts an MQTT. Die Kopfzeile
  zeigt „Dauerbetrieb: N Linien“ mit Liste über alle Projekte. Beim Update eines Gateways (läuft
  als Dienst) bleiben Linien mit Plan eingeschaltet; im Fenster ist alles aus.
- **Windows-Dienst:** `mbusio-dienst-einrichten.cmd` (oder `--install-service` als Administrator)
  richtet mbusio als Dienst ein: läuft ohne Anmeldung und ohne Fenster, startet mit Windows und nach
  einem Absturz neu, PC schläft nicht ein, solange ein Plan läuft. Daten in `C:\ProgramData\mbusio`
  (bisherige Datenbank wird kopiert). `--uninstall-service` entfernt ihn.
- **Hinweis im Dauerbetrieb:** läuft mbusio nur im Fenster (nicht als Dienst bzw. unter systemd),
  zeigt der Zustand der Anlage „Läuft nur, solange mbusio geöffnet ist“ mit Anleitung.
- **Sauber beenden unter systemd:** SIGTERM beendet mbusio geordnet wie Ctrl+C.
- **USB-Wandler weg im Dauerbetrieb:** die Werte der Linie gehen beim ersten verpassten Termin als
  „gestört“ (`disturbed`) an MQTT, nicht erst nach drei Abständen als „veraltet“.
- **Fester Name für USB-Wandler unter Linux:** die Port-Auswahl speichert `/dev/serial/by-id/…`
  statt `/dev/ttyUSB0`; so findet mbusio den Wandler auch, wenn Linux ihm nach dem
  Wiedereinstecken eine andere Nummer gibt. Bestehende Linien: Port einmal neu wählen.
- **Ein Anschluss, eine Linie:** benutzt schon eine Linie (auch eine Plan-Linie im Hintergrund)
  einen Port oder ein Gateway, wird eine zweite abgelehnt mit „… wird schon von Linie „…“ im
  Projekt „…“ benutzt“, statt „Device or resource busy“ oder, bei Gateways im Netzwerk, zwei
  Verbindungen auf denselben Bus. Gilt auch als Grund für „gestört“ im Ausleseplan.
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

- **CSV der Datenpunkte:** zwei neue Spalten am Ende (Modbus, Register). Ältere Dateien ohne diese
  Spalten lassen sich weiter einlesen.
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

- **Verlauf:** Werte desselben Zählers an verschiedenen Untereinheiten (z.B. Volumen am Zähler und an
  Impulseingang 1 und 2) waren im Diagramm zu einer Linie vermischt; jetzt eigene Reihen.
- **Simulator:** eine Kopie der Vorlage „Praxis“ liess sich nicht speichern („Hersteller: 3
  Grossbuchstaben“), weil der Zähler mit fester Datenstruktur keinen Hersteller hat.
- **Node-RED bekam neue Namen nicht mit:** Umbenennen oder Löschen von Projekt, Linie oder Zähler
  meldete sich nicht bei Node-RED (erst beim nächsten Plan-Durchgang). Jetzt meldet jede
  erfolgreiche Änderung über die API den neuen Stand.
- **API: „nicht gefunden“ ist 404** statt 502 (502 bleibt für Fehler am Bus). Damit zeigt der
  Node-RED-Node bei einem gelöschten Projekt „Projekt nicht gefunden“ statt „nicht erreichbar“.
- **Projekt-Import:** Bei einer unlesbaren Datei hiess es „Datei enthält keine Zählerliste“; jetzt
  „Datei ist keine Projektdatei von mbusio“.
- **MQTT-Einstellungen:** Wer TLS ausschaltete und speicherte, verlor das eigene CA-Zertifikat und
  „Zertifikat nicht prüfen“; wer Home Assistant ausschaltete, das eigene Präfix. Ausgeblendete
  Felder behalten jetzt ihren Wert.
- **Doppelte Adressen:** Ein Topic oder Modbus-Register, das schon ein anderer Datenpunkt hat, wird
  beim Bearbeiten und beim CSV-Import abgelehnt (bei Modbus hätte sonst ein Wert den anderen
  überschrieben). Zwei Adressen per CSV tauschen geht weiterhin.
- **Simulator:** Nach einem Neustart von mbusio begannen die simulierten Zählerstände wieder beim
  Anfangswert; mbusio meldete dann „Zählerstand gesunken“ (ungültig), auch an MQTT, Home
  Assistant und Node-RED. Die Zählerstände laufen jetzt ab dem 1.10.2026 nach der Systemuhr und
  steigen über einen Neustart weiter; sie sind dadurch grösser als bisher.
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
