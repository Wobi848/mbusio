# mbusio

M-Bus-Master mit Weboberfläche für Inbetriebnahme und Service: Zähler suchen, Fehler finden und
beheben, auslesen, dokumentieren. Eine einzige Datei für Windows, Linux und Raspberry Pi, ohne
Installation; die Oberfläche läuft im Browser, auch auf dem Handy.

*English below.*

## So arbeitet man damit

Die Oberfläche hat drei Ebenen; jeder Ablauf hat genau eine Stelle
(siehe [docs/konzept-bedienung.md](docs/konzept-bedienung.md)):

- **Kopfzeile = Projekt:** Projekt wählen oder anlegen, **Anlage** (Gateways und Linien, Bericht,
  Export/Import, Zustand der Anlage), rechts **Werkzeuge**, **Simulator** und ⚙ (Erklärungen,
  Design, Sprache).
- **Linien-Leiste = Linie:** welche Linie (Name und Gateway), Verbindung mit genau einem Knopf für
  den nächsten Schritt (Anschluss festlegen, Verbinden, Anschluss ändern, Abbrechen), Ausleseplan.
  Das Verbindungsformular klappt darunter auf.
- **Laschen der Linie:**
  1. **Scan** – Suche einstellen (Primär mit Adressbereich oder Sekundär mit „ID beginnt mit …“,
     Baudraten; abgebrochenen Sekundärscan weitersuchen), dann das Ergebnis:
     jede Auffälligkeit steht als Streifen direkt unter ihrer Zeile, mit Erklärung und dem Knopf,
     der hilft; „N beheben …“ und „N Zähler übernehmen“; Vergleich mit dem letzten Scan.
  2. **Zähler** – Zählerliste mit Suche und Filtern (gestört, ohne Namen), benennen, Liste
     importieren, alle auslesen, Verlauf, Plan je Zähler; Bearbeiten klappt in der Zeile auf.
  3. **Weitergabe** – was (Datenpunkte je Zähler) wohin (Ausgänge: MQTT, später Modbus) und wie oft.

Jeder Zähler erscheint überall gleich: Symbol und Farbe des Mediums, Name (oder „ohne Namen“),
darunter ID · PA · Hersteller.

Neue Benutzer führt die Leiste unten mit **„Nächster Schritt“** und genau einem Knopf: suchen →
merken → auslesen → **Linie in Ordnung** (Bericht); Anschluss und Verbinden zeigt die
Linien-Leiste. Wer danach regelmässig auslesen und weitergeben will, drückt **„Dauerbetrieb
einrichten“**: Plan festlegen → Werte wählen → Ausgang (MQTT) einrichten. Einen Modus gibt es
nicht: Sobald ein Plan, ein Datenpunkt oder ein Ausgang eingerichtet ist, gilt das Projekt als
Dauerbetrieb und oben erscheint der Zustand der Anlage. Die Laschen zeigen den Fortschritt
(✓ erledigt, Anzahl Zähler bzw. Datenpunkte, Punkt = als Nächstes).

Unten zeigt eine Statusleiste immer, was der Bus gerade macht und wie viele Auffälligkeiten offen
sind. Unter ⚙ blendet **„Erklärungen anzeigen“** Hilfetexte ein. Am Ende entsteht ein **Bericht**
(PDF) fürs Projekt. **Werkzeuge:** zuerst das Ziel (Zähler aus Liste oder Scan, oder eine Adresse),
darunter Primäradresse, Baudrate und Reset; dazu einzelnen Zähler roh auslesen, Telegramm-Log und
-Analyse.

## Funktionen

### Verbindung und Suche

- Serielle Schnittstelle (COM-Port, `/dev/ttyUSB0`, …) oder M-Bus-Gateway im Netzwerk
  (`tcp://192.168.1.50:10001`), Baudrate 300–38400, 8E1
- Primärscan (Bereich, Wiederholungen, geschätzte Höchstdauer) und Sekundärscan (Suche über die ID)
- Busse mit gemischten Baudraten: prüfen, wo Geräte antworten, und nur dort scannen
- Trefferliste sortierbar, Sekundäradresse lesbar zerlegt oder als 16 Hex-Zeichen

### Fehler finden und beheben

- Erkennt Kollisionen, doppelte Primäradressen, doppelte IDs (Hinweis, wenn nur die Hersteller
  verschieden sind), Zähler auf fremder Baudrate und Zähler ohne brauchbare Daten
- „Warum?“ nennt die Ursache in einfachen Worten (Prüfsumme, abgeschnitten, falsche Adresse,
  Fehlermeldung des Zählers …) mit Tipp, was zu tun ist
- „PA ändern“ sucht eine freie Adresse auf dem Bus; „Baudrate ändern“ stellt auf die Bus-Baudrate um
- „Alle beheben“: Plan zum Abhaken, jede Änderung wird am Zähler kontrolliert und protokolliert
- Sicherheitsprüfungen: keine Adressänderung auf belegte Adressen oder wenn mehrere Zähler
  antworten; Auslesen über eine Primäradresse mit mehreren Zählern wird abgelehnt

### Zähler, Auslesen, Verlauf

- Auslesen aller Werte mit Einheit, Speicher, Tarif; Antworten über mehrere Telegramme;
  variable und feste Datenstruktur (alte Zähler), Fehlermeldungen des Zählers
- Zählerliste je Linie, gemerkt **über SA oder PA** (einstellbar je Linie, keine gemischten Listen)
- „Alle auslesen“, auch mit unterschiedlichen Baudraten auf einem Bus
- Verlauf je Zähler als Diagramm oder Tabelle mit Differenz und Verbrauch pro Tag, als CSV
- Baudrate suchen und umstellen, Application Reset, Telegramme senden (Experten)

### Zählerliste schnell füllen

- **Liste importieren:** Liste vom Kunden oder Installateur als CSV-Datei, oder in Excel die
  Zellen kopieren und einfügen. Spalten werden erkannt (ID/Zählernummer, Name/Bezeichnung,
  Ort/Wohnung, Notiz, PA) und lassen sich zuordnen. Zuordnung über die **ID auf dem
  Typenschild**: fehlende Zähler werden angelegt, vorhandene benannt; leere Zellen ändern nichts.
  Vorher zeigt mbusio, was neu ist und was sich ändert, und warnt bei IDs, die im letzten Scan
  nicht gefunden wurden oder schon auf einer anderen Linie stehen
- **Exportieren:** Zählerliste als CSV für Excel (ID, PA, Hersteller, Medium, Name, Ort, Notiz,
  Sekundäradresse, letzter Wert), z.B. zum Ausfüllen lassen und wieder Einlesen
- **Benennen:** Name, Ort und Notiz direkt in der Tabelle; Enter springt zum nächsten Zähler,
  gespeichert wird sofort. Ortsvorschläge und „wie oben“
- **Namen erzeugen** nach Muster, z.B. `{ort} {medium}` → „Whg 3 Kaltwasser“ (Platzhalter
  `{ort}`, `{medium}`, `{id}`, `{pa}`, `{nr}`, `{name}`), für alle oder nur Zähler ohne Namen

### Geplantes Auslesen (Dauerbetrieb)

- **Ausleseplan** im Projekt, je Linie oder je Zähler: Aus, monatlich, täglich, stündlich, alle
  15 min. Linie und Zähler übernehmen den Plan von oben, solange sie keinen eigenen haben.
  **Standard ist überall Aus**; geplant gelesen wird nur, was man bewusst einschaltet
- Termine im Raster der Uhr; jede Linie hat ihre eigene Verbindung, Linien laufen gleichzeitig.
  Bedienung (Scan, Auslesen, Adresse ändern) hat immer Vorrang, kein „Bus belegt“
- Verpasste Termine (mbusio aus, Linie getrennt) werden einmal nachgeholt, mit echtem Zeitstempel
- Fehler wird einmal wiederholt; nach 3 verpassten Terminen gilt ein Zähler als **gestört**.
  Lehnt ein Zähler ab (beschäftigt, zu viele Auslesungen), wird er bis zum nächsten Tag ausgesetzt
- Warnung, wenn die geplanten Auslesungen einer Linie nicht mehr in die Buszeit passen
- **Uhrzeit:** ohne Zeitserver (NTP) oder Uhr-Modul gilt sie als unsicher; Werte werden dann als
  „Zeit unsicher“ gekennzeichnet. „Uhrzeit nicht bestätigt“ unten in der Leiste übernimmt die
  Uhrzeit des Laptops oder Handys
- **Speicherung:** geplante Auslesungen nur als Rohtelegramm (rund 100–250 Byte statt ~2 KB),
  decodiert wird beim Anzeigen; Zeitstempel zusätzlich in UTC. Messwerte werden nie von selbst
  gelöscht; optional je Projekt „Verlauf: nach 30/90/365 Tagen nur 1 Wert pro Tag“
- **Speicherschutz:** wird der Platz knapp, warnt die Leiste unten; ist er fast voll, werden keine
  neuen Auslesungen mehr gespeichert (Zählerliste und Anzeige laufen weiter), damit Datenbank und
  Gerät bedienbar bleiben
- Die Datenbank übersteht Absturz und Stromausfall (SQLite WAL, Transaktionen); nach dem Neustart
  geht es von selbst weiter
- Konzept und nächste Schritte (MQTT, Modbus):
  [docs/konzept-dauerbetrieb.md](docs/konzept-dauerbetrieb.md)

### Weitergabe: Datenpunkte und Ausgänge

Alles im Reiter **Weitergabe**: oben die **Ausgänge** (heute MQTT) mit Zustand und Einstellungen,
darunter die **Datenpunkte nach Zählern gruppiert** (aktive Linie oder ganzes Projekt). Aus der
Zählerliste („Weitergabe“) und aus einer Auslesung führt ein Link direkt zum Zähler.

- Je Zähler **„Werte wählen“**: alle Werte mit aktuellem Wert anhaken; der Hauptwert ist
  vorgeschlagen. Jeder gewählte Wert ist ein Datenpunkt; je Ausgang ein Häkchen. Name, Einheit und
  Umrechnung (z.B. Wh → kWh mit Faktor 0.001) einstellbar
- **Vorlage:** gleiche Auswahl für alle Zähler desselben Mediums auf der Linie, auf Wunsch auch für
  Zähler, die später dazukommen, und auf Wunsch gleich per MQTT
- **Stabile Kennung** aus der Bedeutung des Werts (Grösse, Einheit, Speicher, Tarif, Funktion),
  nicht aus der Position im Telegramm; **stabile Nummer** je Projekt, wird nie neu vergeben
- **Qualität** je Datenpunkt: gut, veraltet (älter als 3 Termine), gestört (Zähler antwortet
  nicht), ungültig (Zähler meldet Fehler, oder Zählerstand gesunken)
- **Zähler getauscht…** (beim Bearbeiten eines Zählers): neuer Zähler wird ausgelesen, die
  Zählerstände laufen nahtlos weiter (alter Endstand → neuer Anfangsstand), steht im Bericht
- Mehrere markieren und gemeinsam aktiv/inaktiv schalten, Weitergabe setzen oder löschen
- **CSV** exportieren, in Excel bearbeiten und wieder importieren; mbusio zeigt vorher, was sich
  ändert, und übernimmt erst nach Bestätigung. Die Liste dient auch als Unterlage für den
  Integrator; im Bericht gibt es eine Seite „Datenpunkte“
- Weitergabe je Datenpunkt: bei jeder Auslesung, nur bei Änderung oder 1× täglich

### MQTT

- Reiter Weitergabe → Ausgänge → MQTT → **Einstellungen**: Broker, Port, Benutzer/Passwort, TLS
  (mit eigenem CA-Zertifikat oder, für selbst signierte Broker, ohne Prüfung), Client-ID und
  Basis-Topic
- Je Datenpunkt das Häkchen **MQTT**; das Topic wird beim Einschalten vorgeschlagen
  (`mbusio/<projekt>/<gateway>-<linie>/<zähler>/<wert>`, z.B. `…/whg-1/energy`) und bleibt danach
  gleich, auch wenn Namen sich ändern; überschreibbar
- Nachricht JSON, retained, QoS 1:
  `{"value": 123456, "unit": "kWh", "time": "2026-10-17T19:03:12Z", "quality": "good", "name": "Whg 1 · Energie", "no": 1}`,
  bei unsicherer Uhrzeit zusätzlich `"time_unsure": true`. Ändert sich nur die Qualität (Zähler
  antwortet nicht mehr, Wert veraltet), wird das ebenfalls gemeldet
- `mbusio/status` = `online`, bei Absturz oder Stromausfall `offline` (Last Will)
- **Zwischenspeicher:** ist der Broker nicht erreichbar, sammelt mbusio die Nachrichten (bis 7 Tage,
  auch über einen Neustart) und liefert sie danach in richtiger Reihenfolge nach; gelöscht wird
  eine Nachricht erst, wenn der Broker sie bestätigt hat
- Optional die **Uhrzeit vom Leitsystem**: Nachricht auf `mbusio/time` (Sekunden oder
  Millisekunden seit 1970 oder `2026-10-17T12:00:00Z`)
- Das Passwort erscheint nie in der Oberfläche, im Protokoll oder im Projekt-Export

### Dokumentation

- Bericht je Projekt mit Kunde, Objekt, Bearbeiter, allen Linien und Zählern, optional mit allen
  Messwerten, offenen Auffälligkeiten (ausblendbar) und durchgeführten Änderungen; drucken oder
  als PDF speichern
- Export der Messwerte als CSV (Semikolon, Excel) oder JSON; Projekte als Datei exportieren und
  importieren (z.B. vom PC auf den Raspberry Pi)
- Telegramm-Log; jede TX/RX-Zeile anklicken erklärt das Telegramm Byte für Byte

### Allgemein

- Alles wird gespeichert (SQLite, eine Datei `mbusio.db`)
- Deutsch und Englisch, hell/dunkel, fürs Handy angepasst
- Kommandozeile für Skripte mit JSON-Ausgabe
- Simulator mit Vorlagen (Praxis mit Fehlern, alles i.O., Mehrfamilienhaus, gemischte
  Baudraten) und eigenen, frei zusammengestellten Anlagen

## Hardware

Ein normaler USB-Seriell-Adapter genügt nicht: M-Bus braucht einen **Pegelwandler / M-Bus-Master**
(z.B. USB-M-Bus-Master für 3–60 Zähler), der die Busspannung liefert. mbusio spricht diesen
Wandler über seine serielle Schnittstelle an, oder ein M-Bus-Gateway über TCP.

## Starten

Fertige Programme gibt es bei den [Releases](https://github.com/Wobi848/mbusio/releases):
`mbusio-windows-x86_64.exe`, `mbusio-linux-x86_64`, `mbusio-raspberrypi-arm64` (Raspberry Pi OS
64-Bit) und `mbusio-raspberrypi-armv7` (32-Bit). Beim Start öffnet sich der Browser mit
`http://localhost:8080`; die zuletzt aktive Linie wird automatisch wieder verbunden.

### Windows ohne Adminrechte

Keine Installation nötig: die .exe in einen beliebigen Ordner legen und doppelklicken. Bei der
SmartScreen-Warnung „Weitere Informationen → Trotzdem ausführen“. Ein zweiter Doppelklick öffnet
nur wieder den Browser.

- **Portabel** (z.B. USB-Stick): Ordner `mbusio-data` neben die .exe legen, dann liegen alle
  Projekte dort statt in `%APPDATA%\mbusio`.
- Der **Treiber des USB-M-Bus-Masters** braucht beim Installieren Adminrechte. Viele Adapter
  (FTDI) erkennt Windows von selbst; im Geräte-Manager muss ein COM-Port erscheinen.
- Ohne `--lan` starten, sonst verlangt die Windows-Firewall eine Freigabe (Adminrechte).

### Raspberry Pi als Dienst

```sh
sudo usermod -aG dialout pi        # Zugriff auf /dev/ttyUSB0, danach neu anmelden
sudo install -m 755 mbusio-raspberrypi-arm64 /usr/local/bin/mbusio
```

```ini
# /etc/systemd/system/mbusio.service
[Unit]
Description=mbusio
After=network.target

[Service]
ExecStart=/usr/local/bin/mbusio --lan --token GEHEIM --no-browser --data-dir /home/pi/mbusio-data
User=pi
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl enable --now mbusio
```

Dann im Handy- oder Laptop-Browser `http://<IP des Pi>:8080` öffnen.

### Optionen

| Option | Bedeutung |
| --- | --- |
| `--http-port 8080` | Port der Weboberfläche |
| `--lan` | auch aus dem Netzwerk erreichbar (sonst nur `127.0.0.1`) |
| `--token GEHEIM` | Zugriffsschlüssel, empfohlen mit `--lan` (auch per `MBUSIO_TOKEN`) |
| `--port`, `--baud` | beim Start mit diesem Anschluss verbinden statt mit der zuletzt aktiven Linie |
| `--timeout-ms` | Antwort-Timeout, `0` = automatisch aus der Baudrate |
| `--data-dir` | Ordner für `mbusio.db` (Standard: `mbusio-data` neben dem Programm, falls vorhanden, sonst `~/.local/share/mbusio` bzw. `%APPDATA%\mbusio`) |
| `--no-browser` | Browser beim Start nicht öffnen |
| `--list-ports` | serielle Schnittstellen auflisten |
| `--untrusted-clock` | Systemuhr nie als bestätigt ansehen (alle Werte „Zeit unsicher“, bis die Uhrzeit im Browser übernommen wird) |

### Daten und Sicherung

Alle Projekte, Zählerlisten, Auslesungen (mit Verlauf), letzten Scans und Änderungen liegen in
**einer Datei** `mbusio.db` (SQLite). Zum Sichern mbusio beenden und die Datei kopieren, oder ein
Projekt über „Exportieren“ als JSON-Datei sichern. Eine `meters.json` aus Version 0.3 wird beim
ersten Start ins Projekt „Standard“ übernommen.

### Kommandozeile

Wie die Tools von libmbus, aber ein Programm. Ergebnis als JSON auf stdout, Telegramm-Log auf stderr.

```sh
mbusio --port /dev/ttyUSB0 scan --from 1 --to 50
mbusio --port /dev/ttyUSB0 scan-secondary
mbusio --port /dev/ttyUSB0 bus-bauds              # auf welchen Baudraten antworten Geräte?
mbusio --port /dev/ttyUSB0 scan --all-bauds       # Scan auf allen diesen Baudraten
mbusio --port /dev/ttyUSB0 scan --bauds 2400,9600 # nur auf diesen Baudraten
mbusio --port /dev/ttyUSB0 read 12345678          # Primäradresse, ID oder Sekundäradresse
mbusio --port /dev/ttyUSB0 set-address 12345678 5
mbusio --port /dev/ttyUSB0 detect-baud 5
mbusio --port /dev/ttyUSB0 switch-baud 5 9600
mbusio --port /dev/ttyUSB0 reset 5
mbusio --port tcp://192.168.1.50:10001 raw "10 40 05 45 16"
```

## Simulator

Im Reiter **Simulator** lassen sich virtuelle Anlagen zusammenstellen, ohne Hardware: zum Üben,
Testen und Vorführen. Der Reiter erscheint nur, solange die aktive Linie eine Simulation benutzt;
sonst öffnet man ihn unter **Werkzeuge → Simulator öffnen**. Eingebaute Vorlagen:

| Simulation | Anschluss | Inhalt |
| --- | --- | --- |
| Praxis mit typischen Fehlern | `SIMULATOR` | 21 Zähler mit Fehlern aus der Praxis (Tabelle unten) |
| Alles in Ordnung – 10 Zähler | `SIMULATOR:ok10` | Wärme, Kalt- und Warmwasser, Strom, Gas, PA 1–10 |
| Mehrfamilienhaus – 30 Zähler i.O. | `SIMULATOR:mfh30` | 10 Wohnungen mit Wärme, Kalt- und Warmwasser |
| Gemischte Baudraten | `SIMULATOR:bauds` | 8 Zähler auf 2400, 9600 und 300 Bd, sonst i.O. |

Eine Vorlage **kopieren**, dann Zähler frei anlegen (einzeln oder „+ mehrere“), mit Primäradresse,
ID, Hersteller, Medium, eigener Baudrate und, wenn gewünscht, einem Fehler: Prüfsumme falsch,
Telegramm abgeschnitten, „beschäftigt“, falsche Adresse, Wackelkontakt, „zu viele Auslesungen“.
Doppelte Adressen oder IDs werden beim Bearbeiten angezeigt (manchmal sind sie ja gewollt).
**„Für aktive Linie verwenden“** verbindet die aktive Linie mit der Simulation
(`SIMULATOR:<name>`); verschiedene Linien können verschiedene Anlagen simulieren. Änderungen
gelten nach dem Speichern sofort.

Belegung von `SIMULATOR` (Praxis mit Fehlern):

| Adresse | Zähler | Besonderheit |
| --- | --- | --- |
| 1 | Wärmezähler, ID 12345678 | Antwort in zwei Telegrammen |
| 2 | Elektrizitätszähler, ID 87654321 | Tarife, Spannung, Strom, Firmware-Text |
| 3 | Wasserzähler, ID 30000001 | |
| 0 | zwei Wasserzähler, ID 12345699 und 55500011 | Kollision, nur per Sekundäradresse erreichbar |
| 4 | Wasserzähler, ID 40000004 | falsche Prüfsumme |
| 5 | Wasserzähler, ID 50000005 | abgeschnittenes Telegramm |
| 6 | Wasserzähler, ID 60000006 | läuft mit 9600 Bd (bzw. 2400 Bd, wenn mit 9600 verbunden) |
| 7 | alter Wärmezähler, ID 70000007 | feste Datenstruktur (CI 73) |
| 9 | Wasserzähler, ID 90000009 | uralt, läuft mit 300 Bd |
| 10 | zwei Wärmezähler, ID 10000010 und 10000011 | gleiche Primäradresse |
| 11 | Wasserzähler, ID 11000011 | meldet „beschäftigt“ (Fehlertelegramm CI 70) |
| 12 | Wasserzähler, ID 12000012 | antwortet mit falscher Adresse (13) |
| 14, 15 | zwei Wasserzähler mit gleicher ID 77777777 | verschiedene Hersteller |
| 16 | Wasserzähler, ID 16000016 | Wackelkontakt: nur jedes zweite Telegramm kommt an |
| 17, 18 | zwei Wasserzähler mit gleicher ID 88888888 | gleicher Hersteller: echter Fehler |
| 19 | Wasserzähler, ID 19000019 | nach 2 Auslesungen „zu viele Auslesungen“ (Fehlertelegramm 09) |

Baudratenwechsel und Application Reset werden wie bei echten Zählern quittiert.

## Erster Test mit echter Hardware

1. Linie anlegen, Anschluss eintragen (z.B. `COM3` oder `/dev/ttyUSB0`), Baudrate 2400
2. Im Scan „Prüfen“: auf welchen Baudraten antworten Geräte?
3. Einen bekannten Zähler unter „Werkzeuge“ über seine Primäradresse auslesen
4. Bei Problemen: Telegramm-Log aufklappen, Zeilen anklicken (Analyse) und die TX/RX-Zeilen mitschicken

## Bekannte Grenzen

- Antworten zwei Zähler exakt gleichzeitig mit einer Quittung (`E5`), sieht das manchmal wie eine
  einzige Antwort aus. Der Sekundärscan ist deshalb der sichere Weg, alle Zähler zu finden.
- Telegramme mit umgekehrter Byte-Reihenfolge (CI 76/77) werden nicht unterstützt.
- Die ID eines Zählers lässt sich nicht ändern; das kann nur die Software des Herstellers.

## Entwicklung

```sh
cargo test                     # Frames, Decoder, Analyse, Scans, Adressierung, Datenbank, Zeitplan
cargo run -- --port SIMULATOR
cargo run -- --time-scale 48   # Zeitraffer für Dauertests: ein Tag in 30 Minuten
```

Aufbau:

- `src/mbus/frame.rs`: Telegramme bauen und prüfen (EN 13757-2)
- `src/mbus/decode.rs`, `tables.rs`: Datensätze decodieren (EN 13757-3)
- `src/mbus/master/`: Verbindung, Auslesen, Ursachen von Störungen (mod), Primär- und
  Sekundärscan, Baudraten am Bus (scan), Adresse, Baudrate, Reset (config)
- `src/mbus/analyze.rs`: Telegramm-Analyse Byte für Byte
- `src/link.rs`: serielle Schnittstelle und TCP; `src/sim/`: Simulator (bus: Verhalten der Zähler, scenarios: Vorlagen und eigene Anlagen, store: Speichern), alle hinter demselben `Link`-Trait
- `src/db/`: Datenbank, nach Bereichen (schema mit Migrationen, projects, meters, readings, schedule, report)
- `src/line.rs`: Linien-Dienst, eine Verbindung und Warteschlange je Linie (Bedienung vor Plan)
- `src/datapoint/`: Datenpunkte (Kern: Kennung, Qualität, Plausibilität; outputs: Ausgänge,
  allgemein für MQTT und später Modbus; templates: Vorlagen; exchange: Zählertausch; csv: Export/Import)
- `src/changeset.rs`: Änderungssatz für „Vorschau → Übernehmen“ (Importe, Vorlagen)
- `src/meterlist.rs`: Zählerliste importieren (über die ID), Namen nach Muster, CSV-Export
- `src/mqtt/`: MQTT-Ausgang: Einstellungen, Warteschlange in der Datenbank, Dienst (mod);
  eigener kleiner Client für MQTT 3.1.1 mit TLS über rustls (client)
- `src/plan.rs`, `src/scheduler.rs`: Ausleseplan und Zeitplan; `src/clock.rs`: Uhrzeit und ob sie
  bestätigt ist; `src/storage.rs`: Schutz vor vollem Speicher
- `src/web/`: HTTP-API, nach Bereichen (bus, meters, projects, datapoints, outputs, simulator); `web/`: Oberfläche und Bericht (in die Binärdatei eingebettet)
- `src/i18n.rs`: Übersetzungen der Messgrössen, Sprache je Request; alle Texte der Oberfläche und
  des Berichts in `web/js/texts.js`
- `web/js/`: Oberfläche, normale Skripte ohne Build-Schritt, Reihenfolge in `index.html`:
  `base.js` (Speicher, Sprache, API; auch für den Bericht), `core.js`, `ui.js` (gemeinsame Bausteine:
  Chip, Symbol-Knopf, Tabelle, Eingabefeld, Formular, Aufklapp-Bereich, Vorschau → Übernehmen,
  CSV zerlegen), dann je Bereich eine Datei (busbar, scan, fix, tools, meters, chart, meterlist,
  transfer, sim, project, guide); `main.js` startet alles
- `web/css/`: `base.css` (Farben, Elemente, Bausteine: Kopfzeile, Linien-Leiste, Laschen, Zähler-Baustein,
  Tabellen-Leiste, Statusleiste), dann je Reiter eine Datei
- `tests/ui/`: Browser-Tests der Oberfläche (siehe `tests/ui/README.md`)

Der Decoder ist eine Eigenentwicklung und wird gegen das Referenztelegramm aus der
M-Bus-Dokumentation getestet. Funktionsumfang wie libmbus: Primär-/Sekundärscan, Auslesen mit
mehreren Telegrammen, Primäradresse setzen, Baudrate umstellen, Application Reset,
Rohtelegramme, serielle und TCP-Verbindung, variable und feste Datenstruktur.

---

## English

M-Bus master with a built-in web UI for commissioning and service: find meters, find and fix
problems, read them, document the job. One binary for Windows, Linux and Raspberry Pi, no
installation; the UI runs in the browser, phones included.

- Four steps: **Project** (customer, site → gateways → lines, plant status), **Scan** (check baud
  rates, primary and secondary scan, issues with “Why?” and “Fix all”), **Meters** (meter list,
  names, import list, read all, history, plan, meter replacement), **Pass on** (datapoints per meter,
  outputs such as MQTT); plus **Tools** (raw reading, address/baud rate, reset, telegram log) and
  the **Simulator**; a **“Next step”** card leads new users from an empty project to passing on values
- Detects collisions, duplicate addresses and IDs, meters on another baud rate and meters without
  usable data; explains the cause and fixes what can be fixed, verified on the meter
- Meter list per line, remembered by secondary or primary address (set per line)
- Read plan per project, line or meter (off, monthly, daily, hourly, every 15 min; off by default),
  each line on its own connection, manual work always first; missed terms caught up once, faulty
  and refusing meters marked; readings flagged “time unsure” without a confirmed clock
- Datapoints: choose per meter which values are passed on (templates per medium, stable keys and
  numbers, quality, “counter decreased” check, meter replacement with continuous counters, CSV
  export/import with preview)
- Fill the meter list fast: import the customer's list (CSV or pasted from Excel, matched by the ID
  on the name plate, with preview), edit names and locations directly in the table, generate names
  from a pattern, export as CSV
- MQTT output: one retained topic per datapoint with JSON (value, unit, UTC time, quality), QoS 1,
  status topic with last will, persistent queue with in-order redelivery, TLS, user/password,
  optional time from the control system
- History per meter as chart or table, CSV; project report as PDF with changes made and open issues
- Serial port or network gateway (`tcp://host:port`), command line with JSON output
- Everything stored in one SQLite file `mbusio.db`; German and English UI; simulator with templates and your own virtual installations

```sh
mbusio                                  # http://localhost:8080
mbusio --port SIMULATOR                 # try it without hardware
mbusio --lan --token SECRET             # reachable from the LAN, protected by a token
```

You need an M-Bus level converter / master; a plain USB-serial adapter is not enough. No admin
rights needed on Windows: the .exe runs from any folder. Put a folder `mbusio-data` next to it to
keep all data there (portable, e.g. USB stick).
