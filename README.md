# mbusio

M-Bus-Master mit Weboberfläche für Inbetriebnahme und Service: Zähler suchen, Fehler finden und
beheben, auslesen, dokumentieren. Eine einzige Datei für Windows, Linux und Raspberry Pi, ohne
Installation; die Oberfläche läuft im Browser, auch auf dem Handy.

*English below.*

## So arbeitet man damit

Die Oberfläche ist in vier Schritte gegliedert:

1. **Projekt** – Projekt (Kunde, Objekt) → Gateways → Linien. Jede Linie ist ein M-Bus-Strang mit
   eigenem Anschluss und eigener Zählerliste. Linie wählen verbindet sofort.
2. **Scan** – auf welchen Baudraten antworten Geräte („Prüfen“), dann Primär- oder Sekundärscan.
   Die Trefferliste zeigt Auffälligkeiten (doppelte Adressen, fremde Baudrate, Zähler ohne Daten)
   mit **„Warum?“** und behebt sie auf Wunsch mit **„Alle beheben“**.
3. **Zähler** – gefundene Zähler merken, benennen, alle auslesen, Verlauf ansehen, exportieren.
4. **Werkzeuge** – einzelnen Zähler auslesen, Adresse/Baudrate ändern, Reset, Telegramm-Log und
   Telegramm-Analyse.

Unten zeigt eine Statusleiste immer, was der Bus gerade macht und wie viele Auffälligkeiten offen
sind. Das **„?“** oben blendet Erklärungen ein. Am Ende entsteht ein **Bericht** (PDF) fürs Projekt.

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

### Datenpunkte (was weitergegeben wird)

- Je Zähler über **„Werte“** wählen, welche Werte Datenpunkte werden; der Hauptwert ist
  vorgeschlagen. Name, Einheit und Umrechnung (z.B. Wh → kWh mit Faktor 0.001) einstellbar
- **Vorlage:** gleiche Auswahl für alle Zähler desselben Mediums auf der Linie, auf Wunsch auch für
  Zähler, die später dazukommen
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

- Im Tab „Zähler“ unter **MQTT**: Broker, Port, Benutzer/Passwort, TLS (mit eigenem
  CA-Zertifikat oder, für selbst signierte Broker, ohne Prüfung), Client-ID und Basis-Topic
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
- Simulator mit 21 virtuellen Zählern und typischen Fehlern aus der Praxis

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

Als Anschluss `SIMULATOR` eintragen. Belegung:

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
- `src/mbus/master.rs`: Scan, Auslesen, Adressierung, Baudrate, Ursachen von Störungen
- `src/mbus/analyze.rs`: Telegramm-Analyse Byte für Byte
- `src/link.rs`: serielle Schnittstelle und TCP; `src/sim.rs`: Simulator, alle hinter demselben `Link`-Trait
- `src/db.rs`: Datenbank (Projekte, Gateways, Linien, Zähler, Auslesungen, Änderungen)
- `src/line.rs`: Linien-Dienst, eine Verbindung und Warteschlange je Linie (Bedienung vor Plan)
- `src/datapoint.rs`: Datenpunkte, Qualität, Plausibilität, Zählertausch, Vorlagen, CSV
- `src/mqtt.rs`: MQTT-Ausgang (eigener kleiner Client für MQTT 3.1.1, TLS mit rustls),
  Warteschlange in der Datenbank
- `src/plan.rs`, `src/scheduler.rs`: Ausleseplan und Zeitplan; `src/clock.rs`: Uhrzeit und ob sie
  bestätigt ist; `src/storage.rs`: Schutz vor vollem Speicher
- `src/web.rs`: HTTP-API; `web/`: Oberfläche und Bericht (in die Binärdatei eingebettet)
- `src/i18n.rs`: Übersetzungen der Messgrössen; UI-Texte in `web/app.js` und `web/report.js`

Der Decoder ist eine Eigenentwicklung und wird gegen das Referenztelegramm aus der
M-Bus-Dokumentation getestet. Funktionsumfang wie libmbus: Primär-/Sekundärscan, Auslesen mit
mehreren Telegrammen, Primäradresse setzen, Baudrate umstellen, Application Reset,
Rohtelegramme, serielle und TCP-Verbindung, variable und feste Datenstruktur.

---

## English

M-Bus master with a built-in web UI for commissioning and service: find meters, find and fix
problems, read them, document the job. One binary for Windows, Linux and Raspberry Pi, no
installation; the UI runs in the browser, phones included.

- Four steps: **Project** (customer, site → gateways → lines), **Scan** (check baud rates, primary
  and secondary scan, issues with “Why?” and “Fix all”), **Meters** (meter list, read all, history,
  export), **Tools** (read one meter, change address/baud rate, reset, telegram log and analysis)
- Detects collisions, duplicate addresses and IDs, meters on another baud rate and meters without
  usable data; explains the cause and fixes what can be fixed, verified on the meter
- Meter list per line, remembered by secondary or primary address (set per line)
- Read plan per project, line or meter (off, monthly, daily, hourly, every 15 min; off by default),
  each line on its own connection, manual work always first; missed terms caught up once, faulty
  and refusing meters marked; readings flagged “time unsure” without a confirmed clock
- Datapoints: choose per meter which values are passed on (templates per medium, stable keys and
  numbers, quality, “counter decreased” check, meter replacement with continuous counters, CSV
  export/import with preview)
- MQTT output: one retained topic per datapoint with JSON (value, unit, UTC time, quality), QoS 1,
  status topic with last will, persistent queue with in-order redelivery, TLS, user/password,
  optional time from the control system
- History per meter as chart or table, CSV; project report as PDF with changes made and open issues
- Serial port or network gateway (`tcp://host:port`), command line with JSON output
- Everything stored in one SQLite file `mbusio.db`; German and English UI; simulator with 21 meters

```sh
mbusio                                  # http://localhost:8080
mbusio --port SIMULATOR                 # try it without hardware
mbusio --lan --token SECRET             # reachable from the LAN, protected by a token
```

You need an M-Bus level converter / master; a plain USB-serial adapter is not enough. No admin
rights needed on Windows: the .exe runs from any folder. Put a folder `mbusio-data` next to it to
keep all data there (portable, e.g. USB stick).
