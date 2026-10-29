# Änderungen

Was sich für Benutzer ändert, je Version. Neuestes oben.

## Unveröffentlicht

### Neu

- **Sicherung des Geräts** (Konzept `docs/konzept-sicherung.md`): ⚙ → Sicherung → „Herunterladen“
  liefert alles in einer Datei (Projekte, Zähler, Messwerte, Einstellungen samt Passwörtern).
  „Einspielen …“ prüft die Datei, zeigt Projekte, Zähler und Zeitraum und startet mbusio danach
  neu; fällt ein Gerät aus, läuft auf dem neuen damit alles wieder. Der bisherige Stand bleibt als
  `mbusio.db.vorher`.
- **Automatische Sicherung** (Weitergabe → Automatische Sicherung, Konzept `docs/konzept-sicherung.md`
  §5): täglich zu einer Uhrzeit auf einen Ordner (USB-Stick, auch vom Betriebssystem eingebundenes
  SMB-Netzlaufwerk, siehe README) oder per eigenem FTP/FTPS-Client; am Ziel bleiben nur die letzten
  paar eigenen Sicherungen, ältere löscht mbusio dort selbst. Mit einem Sicherungs-Passwort
  verschlüsselt (AES-256-GCM); dasselbe Passwort verschlüsselt dann auch „Sicherung herunterladen“
  von Hand, „Einspielen …“ fragt dafür danach. Ein Fehlschlag erscheint bei den Meldungen.

## v0.5.0 (29.10.2026)

Geprüft mit echter Hardware (Relay MR003 USB, Itron-Wärme-/Kältezähler, echter MQTT-Broker, USB
abziehen im Dauerbetrieb) und mit Daten von v0.4.0 (werden ohne Zutun übernommen). **Noch offen:**
der 7-Tage-Test auf einem Raspberry Pi und die Prüfung des Windows-Dienstes auf einem Windows-PC,
siehe `docs/konzept-dauerbetrieb.md`, Abschnitt 11. Für den Dauerbetrieb beim Kunden darum zuerst
eine Anlage zur Probe betreiben.

### Neu

- **Neue Oberfläche** (Konzept `docs/konzept-oberflaeche-neu.md`): Werkzeug statt Webseite. Eine
  Kopfzeile, Liste und Details nebeneinander (etwa doppelt bis fünfmal so viele Zeilen ohne
  Scrollen), Zustand als Wort am Element, genau ein Hauptknopf und „Nächster Schritt“ in der
  Statusleiste, Mehrfachauswahl, Tastatur, Rückfragen am Knopf statt Browser-Fenster, Wechsel von
  Linie und Projekt ohne Neuladen, Zeilen werden während eines Scans nicht mehr ersetzt (keine
  verlorenen Klicks). Die bisherige Oberfläche ist entfernt; die Oberfläche liegt in `web/app/`,
  der Zwischenstand `/neu/` existiert nicht mehr.
- **Protokoll und Meldungen in der eingestellten Sprache:** Meldungen aus dem Hintergrund
  (Zeitplan, MQTT, Modbus, Uhrzeit, Linie getrennt) waren immer deutsch; jetzt folgen sie der
  Sprache der Oberfläche, Einträge im Protokoll auch nach einem Sprachwechsel. Die Sprache bleibt
  über einen Neustart gespeichert.
- **Abrechnung exportieren** (Anlage → Projekt): Zählerstände zum Stichtag je Zähler und Wert als
  CSV für Excel (Anfang, Ende, Verbrauch) oder als Zeitreihe; Vorgaben Vormonat und Hauptwert,
  Zeitraum, weitere Werte und Dezimalzeichen einstellbar und je Projekt gespeichert, mit Vorschau.
  Rückwirkend aus den gespeicherten Auslesungen; Stichtagswert des Zählers vor der letzten
  Auslesung; Quelle, Fehlendes und unsichere Uhrzeit stehen dabei; Zählertausch verrechnet.
  Der bisherige Knopf „Exportieren“ (Projekt als Datei) heisst „Projekt sichern“.
- **Zählertausch erkennen:** Antwortet unter derselben Primäradresse ein anderer Zähler (neue ID),
  nachdem der alte schon gelesen wurde, trägt mbusio den Tausch selbst ein; Zählerstände laufen
  weiter statt zu springen. Hinweis „Tausch erkannt“ in der Zählerliste, in den Details
  bestätigen. Bisher wurde die neue ID still übernommen und der Zählerstand galt als „gesunken“.
- **Defekte Zähler melden:** Fehler, die ein Zähler selbst meldet (Statusbyte, Fehlerflags,
  Batterie schwach), stehen am Zähler und zählen bei „Fehler“. Die Abrechnung nennt je Zeile auch
  „im Zeitraum nicht gelesen“, „antwortet nicht“, „gestört“ und Fehler des Zählers und zählt die
  Hinweise über der Vorschau zusammen.
- **Abrechnung automatisch:** Schalter „Automatisch“ beim Export (Vormonat oder Vorjahr). Nach
  Ende des Zeitraums und der ersten Auslesung danach (spätestens 2 Tage danach) legt mbusio die
  CSV im Datenordner unter `exporte/<projekt-nr>/` ab, meldet sie im Protokoll und zeigt sie zum
  Herunterladen. War das Gerät aus, werden verpasste Zeiträume nachgeholt (höchstens 12).
- **Abrechnung per E-Mail:** Empfänger je Projekt („E-Mail an“), Mailserver als Ausgang
  **Weitergabe → E-Mail** neben MQTT und Modbus: Schalter, Zustand (nicht eingerichtet, bereit,
  Fehler), Einstellungen (STARTTLS, TLS oder ohne; Passwort bleibt auf dem Gerät; Gerätename für
  den Betreff) und Testmail. Jede Mail steht im Weitergabe-Protokoll. Jede neue Datei
  geht als Anhang raus; je Datei steht „gesendet“ oder „nicht gesendet“ mit Grund und „Senden“.
  Fehlgeschlagene werden 7 Tage lang alle 15 Minuten erneut versucht, ins Protokoll kommt nur der
  erste Fehler, ein neuer Grund und der Erfolg.
- **Speicherschutz dünnt selbst aus:** wird der Speicher knapp, behält mbusio von Auslesungen
  älter als 365 Tage (reicht das nicht: 90 Tage) nur die letzte je Zähler und Tag und gibt den
  Platz der Datenbank frei (neue Datenbanken mit `auto_vacuum`, bestehende werden einmal
  umgestellt, wenn genug Platz frei ist). Einstellbar unter Einstellungen ⚙. Bisher wurde bei
  fast vollem Speicher nur nicht mehr gespeichert.
- **Fehler behoben:** Verlauf ausdünnen behielt je Tag die erste Auslesung; jetzt die letzte. Die
  erste verfälschte den Stand am Tagesende (Abrechnung) um den Verbrauch des Tages.
- **Abrechnung zieht in die Weitergabe** (Weitergabe → Abrechnung →, eigene Ansicht mit
  Einstellungen links, Vorschau und Dateien rechts, Projekt wählbar). Neu: Zeiträume
  **Vorquartal** und **Vorhalbjahr** (auch automatisch), **Vorjahre zum Vergleich** (0–10,
  Vorgabe 1; je Jahr eine Spalte „Verbrauch 09/2025“), **Betreff** mit Platzhaltern und Vorschau,
  Knopf **Per E-Mail senden**, einstellbar wie lange auf eine Auslesung gewartet und wie lange
  erneut gesendet wird. Anlage → Projekt zeigt nur noch eine Zeile („Abrechnung: Vormonat,
  automatisch · an …“) mit Öffnen →. Die CSV hat dadurch standardmässig eine Spalte mehr
  (Vorjahr) vor „Hinweis“.
- **Meldungen und Warnungen per E-Mail:** mbusio erkennt im Hintergrund Probleme (Zähler gestört,
  meldet Fehler, Batterie schwach, Zählertausch, Linie im Dauerbetrieb 30 min nicht verbunden,
  MQTT 60 min nicht verbunden, Speicher knapp, Abrechnung nicht gesendet). Sie stehen als
  **Meldungen N** in der Statusleiste: Liste mit Zeit und Ort, „Zeigen →“ springt hin, „Gesehen“
  beruhigt die Anzeige, behobene verschwinden von selbst. Mit Empfänger (Weitergabe → E-Mail →
  Warnungen) kommen neue Probleme gebündelt per Mail, höchstens eine pro Stunde, Behobenes in der
  nächsten. Arten, Wartezeiten, Abstand, Betreff, Behobenes melden und täglich erinnern sind
  einstellbar. Klappt der Versand nicht, versucht mbusio es alle 15 Minuten, höchstens einen Tag.
- **Bericht per E-Mail** (Weitergabe → E-Mail → Bericht): kurze Übersicht seit dem letzten Bericht,
  je Linie gelesen x von y Zählern, Weitergabe je Ausgang (gesendet, verworfen, nicht gesendet),
  offene Probleme, abgelegte und gesendete Abrechnungen, freier Speicher und wie lange er reicht.
  Vorgabe täglich 07:00; Häufigkeit (aus, täglich, wöchentlich), Uhrzeit, Wochentag, Betreff und
  jeder Teil einstellbar. Geht nur raus, wenn „Bericht an“ ausgefüllt ist; der erste kommt am
  nächsten Termin, nicht sofort. War das Gerät aus, kommt danach einer, nicht mehrere. Klappt der
  Versand nicht, versucht mbusio es jede Stunde neu. **Bericht jetzt senden** schickt ihn sofort
  (seit dem letzten Bericht, sonst seit gestern), ohne den Zeitplan zu ändern.
- **Betreff mit Vorschau** bei Warnungen und Bericht („→ mbusio Keller-Pi: Bericht 27.10.2026“),
  wie bei der Abrechnung. Die Testmail ist mit „Warnungen an“ vorbelegt.
- **Ausgewählte Zeile** in den Listen mit Crimson-Strich links, auch beim Arbeiten mit der Tastatur.
- **Abrechnung: gestört im Zeitraum und Linien wählen.** mbusio hält fest, von wann bis wann ein
  Zähler gestört war; die Abrechnung nennt das („gestört 05.09.2026 08:00 – 07.09.2026 09:00“),
  auch wenn er heute wieder antwortet. Bisher stand nur „zurzeit gestört“. Bei Projekten mit
  mehreren Linien lassen sich einzelne Linien wählen.
- **Speicher-Vorschau:** im Zustand der Anlage, wie lange der Platz beim heutigen Plan reicht
  („81 % frei · reicht ca. 12 Jahre“, Zuwachs je Tag im Tooltip); unter 90 Tagen als Hinweis.
- **Weitergabe-Protokoll:** was an MQTT ging, was wartet und was verworfen wurde (Broker zu
  lange weg, Warteschlange voll), je Wert mit Zeit, Topic und Qualität; Filter nach Zeitraum,
  Ergebnis, Datenpunkt und Suche; CSV; 30 Tage. Modbus: welches Leitsystem zuletzt abgefragt hat.
- **Windows: Port gesperrt** (os error 10013, reservierte Ports von Hyper-V, WSL oder Docker):
  mbusio nimmt den nächsten freien Port (8081, 8082 …) und nennt ihn, statt abzubrechen.
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

- **Zählerliste importieren** ist bei grossen Listen viel schneller (2000 Zähler: 0,5 s statt
  8,5 s): die Prüfung auf doppelte Adressen las für jeden neuen Zähler die ganze Liste.

- **Nächster Schritt „Zähler auslesen“** bleibt, bis jeder Zähler einmal ausgelesen wurde; vorher
  ging es schon zum Bericht, sobald irgendein Zähler gelesen war. Der Projektbaum liefert dafür
  `unread` je Linie.

- **Sicherheit: fremde Webseiten können mbusio nicht mehr steuern.** Eine Webseite, die man im
  Browser offen hatte, konnte unbemerkt ändernde Anfragen an mbusio auf demselben Rechner schicken
  (z.B. Projekt leeren, auslesen, Adresse ändern). Solche Anfragen mit fremdem `Origin` lehnt
  mbusio jetzt ab (403). Die eigene Oberfläche und Programme wie Node-RED oder curl sind nicht
  betroffen.

- **mbusio antwortet nicht:** ist der Server beendet oder der Pi neu gestartet, zeigen Kopf und
  Statusleiste „mbusio antwortet nicht“ statt weiter „Verbunden“; danach geht es von selbst weiter.
- **Messwerte als CSV/JSON** heissen ehrlich „letzte 5000 Auslesungen“ (hiess „alle“); für
  Zeiträume gibt es die Abrechnung.
- Scheitert das Protokoll einer Adress- oder Baudratenänderung, steht es im Log statt still zu
  verschwinden.

- **Verlauf eines Zählers über den ganzen Zeitraum:** bisher kamen nur die letzten 2000
  Auslesungen (bei stündlichem Plan knapp 3 Monate, bei 15 min 3 Wochen); „Erste“, Diagramm und
  CSV zeigten nur diesen Ausschnitt. Jetzt alle Auslesungen, bei mehr als 2000 die letzte je Tag.
- **Zählerliste zieht nach:** liest der Plan im Hintergrund (Dauerbetrieb) oder ein anderes
  Fenster, zeigt die offene Liste die neuen Werte, statt bei „noch nicht ausgelesen“ zu bleiben.

- **Funktionen der bisherigen Oberfläche, die gefehlt haben**, sind zurück:
  - Zähler → „Liste ▾“: **Benennen in der Liste** (Name und Ort direkt in den Zeilen, Enter zum
    nächsten Zähler, „↓“ übernimmt den Ort von oben), **Messwerte als CSV oder JSON** (alle
    Auslesungen) und **Alle Auslesungen löschen**.
  - Scan: **Wiederholungen** je Adresse einstellbar (0–3, Vorgabe 1); die geschätzte Dauer folgt.
  - Anlage → Linie: **Timeout ms** (0 = automatisch nach Baudrate).
  - Zähler → Verlauf: **Tabelle** neben dem Diagramm (neueste zuerst, Δ zur vorherigen
    Auslesung, „Zeit unsicher“ markiert) und darüber **Erste, Letzte, Differenz und pro Tag**.
  - Scan: **Treffer sortieren** per Klick auf die Spalte (auf, ab, wie gescannt; bleibt gespeichert).
  - Zählerliste: **„ausgesetzt bis …“**, wenn ein Zähler das Auslesen abgelehnt hat (zu viele
    Auslesungen, beschäftigt), und Anzahl verpasster Termine beim Hinweis „gestört“.
  - Einstellungen (⚙): **Erklärungen anzeigen** blendet in jedem Bereich und bei den Werkzeugen
    einen kurzen Text ein, was hier geht.
- **Verbinden über die Kopfzeile setzte den Timeout der Linie auf 0 zurück**; jetzt bleibt er.
- Weitergabe: Wer aus der Abrechnung das Protokoll öffnete, sah beide übereinander; jetzt ist
  immer nur eine Ansicht offen. Auf dem Handy fehlten in der Abrechnung die Einstellungen ganz, und
  die Filter im Protokoll waren abgeschnitten.
- Abrechnung: Zwei Felder schnell hintereinander geändert (z.B. „Von“ und „Bis“) – die zweite
  Änderung konnte die erste mit dem alten Stand überschreiben. Und eine Antwort des Servers
  ersetzte das Feld, in dem man gerade tippte, samt dem Getippten. Beides behoben.
- Lässt sich ein Bereich der Oberfläche nicht laden (z.B. WLAN kurz weg), kommt eine Meldung statt
  einer leeren Fläche; beim nächsten Öffnen wird er neu geladen.
- Handy: Scan-Treffer lassen sich auch schmal sortieren; „Merken über“ bleibt bei seinen Knöpfen;
  die Statusleiste zeigt den Zustand des Busses in einer eigenen Zeile statt nur als Punkt.
- Aussehen: Einstellungsmenü bündig, Abrechnung und Anlage nutzen die ganze Höhe, Hinweise der
  Abrechnung stehen unter dem Text statt daneben, „Keine Einträge“ steht oben in der Liste.
- Abrechnung, Vorschau: Zähler ohne Namen zeigen ID und PA statt einer leeren Zelle; der Hinweis
  bricht um statt abgeschnitten zu werden.
- Weitergabe: In Abrechnung und Protokoll ist keine Ausgangskarte mehr als gewählt markiert. Die
  E-Mail zeigt ohne Server den Schalter aus (vorher ein, mit „nicht eingerichtet“).
- Aussehen: Häkchen in Formularen so gross wie in Listen und links; am Handy die Reiter in einer
  seitlich laufenden Zeile statt drei; beim Zähler „Löschen“ in derselben Zeile wie „Auslesen“
  („Zähler getauscht …“ heisst jetzt „Getauscht …“); Simulator „+ mehrere“ vor Anzahl und Medium;
  Werkzeuge → Baudrate steht auf der des Busses statt 300; Ausleseplan in der Anlage so breit wie
  die anderen Felder.
- **Gleiche Auswahl für alle Zähler eines Mediums** fehlte in der neuen Oberfläche: beim Zähler unter
  „Letzte Auslesung“ wieder „Gleiche Auswahl für alle Zähler „Wärme“ dieser Linie“, auf Wunsch
  auch für Zähler, die später dazukommen. Abwählen entfernt die Vorlage, Datenpunkte bleiben.
- **Nächster Schritt nach dem Scan:** Gefundene, aber noch nicht gemerkte Zähler führten wieder zu
  „Zähler suchen“; jetzt heisst der Schritt „Gefundene merken“ und führt zum Knopf.

- Weitergabe → Einstellungen eines Ausgangs: aufgeklappte Abschnitte (Warnungen, Bericht) klappten
  nach ein paar Sekunden wieder zu, weil das Formular bei jeder Statusabfrage neu gebaut wurde.

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
