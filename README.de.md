# mbusio

[English](README.md) · **Deutsch**

M-Bus-Master mit Weboberfläche für Inbetriebnahme und Service: Zähler suchen, Fehler finden und
beheben, auslesen, dokumentieren. Eine einzige Datei für Windows, Linux und Raspberry Pi, ohne
Installation; die Oberfläche läuft im Browser, auch auf dem Handy.

## So arbeitet man damit

Die Oberfläche ist ein Werkzeug: eine Kopfzeile, Bereiche als Laschen, darunter Liste links und
Details rechts. Was ein Element ist und in welchem Zustand, steht am Element selbst; der nächste
Schritt ist immer genau ein Knopf (siehe [docs/konzept-oberflaeche-neu.md](docs/konzept-oberflaeche-neu.md)).

- **Kopfzeile:** Projekt › Linie, Anschluss mit genau der passenden Aktion (Anschluss festlegen,
  Verbinden, Ändern), Schalter **Dauerbetrieb** der Linie mit Plan, „N im Dauerbetrieb“ über alle
  Projekte, ⚙ (Sprache, Design, Tastenkürzel).
- **Bereiche** (Tasten 1–6) in drei Gruppen, getrennt durch einen Strich:
  - einrichten: **Anlage** (Projekte, Gateways und Linien, Zustand der Anlage, Bericht,
    Export/Import)
  - der Arbeitsablauf: **Scan** (Suchzeile, Treffer mit Zustand als Wort, doppelte PA auflösen
    und beheben, neue merken), **Zähler** (Liste, Bearbeiten in den Feldern, Mehrfachauswahl,
    auslesen, Verlauf, Tausch, Liste importieren, Namen nach Muster), **Weitergabe** (Ausgänge
    MQTT und Modbus mit Schalter, Datenpunkte)
  - Hilfsmittel: **Werkzeuge** (ein Ziel für Auslesen, Adresse, Baudrate, Reset, Telegramme),
    **Simulator**
- **Statusleiste:** was der Bus macht, Auffälligkeiten, Ausgänge. Den **nächsten Schritt** zeigt der Knopf rechts neben den Laschen.
  Klick auf den Bus öffnet die **Bus-Konsole** (Telegramme live, ein Telegramm anklicken erklärt es
  in Werkzeuge), Klick auf die Auffälligkeiten zeigt sie im Scan.
- **Tastatur:** ↑↓ in Listen, Leertaste wählt aus, Ctrl+A alle, Enter, Escape, `/` sucht, `R`
  liest aus, `S` scannt, `?` zeigt alle Tasten. Keine Browser-Fenster für Rückfragen: sie stehen am
  Knopf.

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
  **Standard ist überall Aus**; gelesen wird erst, wenn der **Dauerbetrieb der Linie** eingeschaltet
  ist (Schalter in der Kopfzeile oder in der Anlage)
- Termine im Raster der Uhr; jede Linie hat ihre eigene Verbindung, Linien laufen gleichzeitig.
  Bedienung (Scan, Auslesen, Adresse ändern) hat immer Vorrang, kein „Bus belegt“
- Verpasste Termine (mbusio aus, Linie getrennt) werden einmal nachgeholt, mit echtem Zeitstempel
- Fehler wird einmal wiederholt; nach 3 verpassten Terminen gilt ein Zähler als **gestört**.
  Lehnt ein Zähler ab (beschäftigt, zu viele Auslesungen), wird er bis zum nächsten Tag ausgesetzt
- Warnung, wenn die geplanten Auslesungen einer Linie nicht mehr in die Buszeit passen
- **Uhrzeit:** ohne Zeitserver (NTP) oder Uhr-Modul gilt sie als unsicher; Werte werden dann als
  „Zeit unsicher“ gekennzeichnet. „Uhrzeit übernehmen“ im Zustand der Anlage übernimmt die
  Uhrzeit des Laptops oder Handys
- **Speicherung:** geplante Auslesungen nur als Rohtelegramm (rund 100–250 Byte statt ~2 KB),
  decodiert wird beim Anzeigen; Zeitstempel zusätzlich in UTC. Messwerte werden nie von selbst
  gelöscht; optional je Projekt „Verlauf: nach 30/90/365 Tagen nur 1 Wert pro Tag“
- **Speicherschutz** (Einstellungen ⚙, Vorgabe an): wird der Platz knapp, dünnt mbusio selbst aus,
  zuerst Auslesungen älter als 365 Tage, wenn nötig älter als 90 Tage, auf eine je Zähler und Tag
  (die letzte, also der Stand am Tagesende, den die Abrechnung braucht), und gibt den Platz der
  Datenbank frei. Ist er trotzdem fast voll, werden keine neuen Auslesungen mehr gespeichert
  (Zählerliste und Anzeige laufen weiter), damit Datenbank und Gerät intakt bleiben
- Die Datenbank übersteht Absturz und Stromausfall (SQLite WAL, Transaktionen); nach dem Neustart
  geht es von selbst weiter
- Konzept und nächste Schritte (MQTT, Modbus):
  [docs/konzept-dauerbetrieb.md](docs/konzept-dauerbetrieb.md); Ausgänge allgemein:
  [docs/konzept-ausgaenge.md](docs/konzept-ausgaenge.md)
- Node-RED-Nodes (Auswahl Projekt › … › Datenpunkt, ohne Broker): Ordner [node-red/](node-red/)
- HTTP-API (alles, was die Oberfläche kann, auch für eigene Programme): [docs/api.md](docs/api.md)

### Weitergabe: Datenpunkte und Ausgänge

Alles im Reiter **Weitergabe**: oben die **Ausgänge** (heute MQTT) mit Zustand und Einstellungen,
darunter die **Datenpunkte** als Liste (aktive Linie oder ganzes Projekt) mit Details rechts und
Mehrfachauswahl. Werte wählt man im Bereich Zähler beim Zähler an.

- **Protokoll** (Knopf bei den Ausgängen, oder „Protokoll dieses Datenpunkts“): was an MQTT ging,
  was wartet und was verworfen wurde (mit Grund), mit Filtern nach Zeitraum, Ergebnis, Datenpunkt
  und Suche, als CSV. 30 Tage. Bei Modbus steht an der Zeile des Ausgangs, welches Leitsystem
  zuletzt abgefragt hat.

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
- **Tausch erkannt:** antwortet unter derselben Primäradresse ein Zähler mit anderer ID, nachdem
  mbusio den alten schon gelesen hat, wird der Tausch selbst eingetragen (Stände laufen weiter) und
  in der Liste als „Tausch erkannt“ zum Bestätigen gezeigt. Stimmt nur die importierte Liste nicht,
  übernimmt mbusio die ID mit Meldung im Protokoll. Meldet ein Zähler selbst einen Fehler
  (Statusbyte, Fehlerflags, Batterie schwach), steht das am Zähler
- Mehrere markieren und gemeinsam aktiv/inaktiv schalten, Weitergabe setzen oder löschen
- **CSV** exportieren, in Excel bearbeiten und wieder importieren; mbusio zeigt vorher, was sich
  ändert, und übernimmt erst nach Bestätigung. Die Liste dient auch als Unterlage für den
  Integrator; im Bericht gibt es eine Seite „Datenpunkte“
- Weitergabe je Datenpunkt: bei jeder Auslesung, nur bei Änderung oder 1× täglich

### MQTT

- Reiter Weitergabe → Ausgänge → MQTT → **Einstellungen**: Broker, Port, Benutzer/Passwort, TLS
  (mit eigenem CA-Zertifikat oder, für selbst signierte Broker, ohne Prüfung), Client-ID und
  Basis-Topic
- Je Datenpunkt das Häkchen **MQTT**; das Topic entsteht beim Einschalten aus dem **Topic-Muster**
  der Einstellungen (Standard `{projekt}/{gateway}-{linie}/{zaehler}/{wert}`, z.B. `…/whg-1/energy`;
  Platzhalter auch `{id}`, `{pa}`, `{ort}`, `{medium}`, `{nr}`) und bleibt danach gleich, auch wenn
  Namen sich ändern; einzeln überschreibbar. **„Alle Topics nach Muster setzen …“** rechnet alle
  Topics des Projekts nach dem Muster neu, mit Vorschau
- **Topic-Liste für den Integrator** als CSV (Excel) oder JSON: je Datenpunkt das volle Topic,
  Name, Einheit, Zähler, ID, PA, Ort, Linie; im JSON dazu Status-Topic, QoS, retained und der
  Aufbau der Nachricht
- Nachricht JSON, retained, QoS 1:
  `{"value": 123456, "unit": "kWh", "time": "2026-10-17T19:03:12Z", "quality": "good", "name": "Whg 1 · Energie", "no": 1}`,
  bei unsicherer Uhrzeit zusätzlich `"time_unsure": true`. Ändert sich nur die Qualität (Zähler
  antwortet nicht mehr, Wert veraltet), wird das ebenfalls gemeldet
- `mbusio/status` = `online`, bei Absturz oder Stromausfall `offline` (Last Will)
- **Node-RED** (ohne Broker): `GET /api/nodered/tree` (Projekt › Gateway › Linie › Zähler ›
  Datenpunkte mit Wert) und `GET /api/nodered/stream?project=1` (Server-Sent Events: zuerst der
  Stand, dann neue Werte und geänderte Qualität); nur lesend, mit `X-Token` wie die übrige API
  (siehe [docs/konzept-node-red.md](docs/konzept-node-red.md))
- **Home Assistant:** Häkchen „Home Assistant (MQTT Discovery)“ in den Einstellungen: alle
  Datenpunkte mit MQTT-Häkchen erscheinen in Home Assistant von selbst, je Zähler ein Gerät, mit
  Einheit und passendem Typ (Energie, Wasser, Gas, Temperatur …, fürs Energie-Dashboard).
  Gestörte Werte zeigt Home Assistant als „nicht verfügbar“. Client-ID danach nicht mehr ändern
  (siehe [docs/konzept-home-assistant.md](docs/konzept-home-assistant.md))
- **Zwischenspeicher:** ist der Broker nicht erreichbar, sammelt mbusio die Nachrichten (bis 7 Tage,
  auch über einen Neustart) und liefert sie danach in richtiger Reihenfolge nach; gelöscht wird
  eine Nachricht erst, wenn der Broker sie bestätigt hat
- Optional die **Uhrzeit vom Leitsystem**: Nachricht auf `mbusio/time` (Sekunden oder
  Millisekunden seit 1970 oder `2026-10-17T12:00:00Z`)
- Das Passwort erscheint nie in der Oberfläche, im Protokoll oder im Projekt-Export

### Modbus TCP

- mbusio ist Modbus-TCP-Server: Leitsystem, SPS oder Loxone lesen die Werte als Register (nur
  lesen, Funktion 03 und 04 liefern dieselben Register, jede Unit-ID)
- Reiter Weitergabe → Ausgänge → Modbus → **Einstellungen**: einschalten, Adresse der
  Netzwerkschnittstelle (Modbus hat keine Zugangskontrolle), Port (502; unter Linux braucht ein
  Port unter 1024 Rechte, sonst z.B. 1502), Projekt, Wortreihenfolge (ABCD oder CDAB)
- Je Datenpunkt das Häkchen **Modbus** (mehrere: auswählen → **Modbus ein**); Startregister =
  (Nummer − 1) × 10, bleibt immer gleich
- Je Datenpunkt 10 Register: Wert als float64 (+0), float32 (+4), int32 × 100 (+6), Qualität (+8:
  0 gut, 1 veraltet, 2 gestört, 3 ungültig, 4 kein Wert), Alter in Minuten (+9); ab 60000 der
  Zustand (Lebenszeichen, Anzahl gestört, Anzahl veraltet)
- **Registerliste für den Integrator** als CSV oder JSON, wie bei MQTT
  (siehe [docs/konzept-modbus.md](docs/konzept-modbus.md))

### Dokumentation

- Bericht je Projekt mit Kunde, Objekt, Bearbeiter, allen Linien und Zählern, optional mit allen
  Messwerten, offenen Auffälligkeiten (ausblendbar) und durchgeführten Änderungen; drucken oder
  als PDF speichern
- **Abrechnung exportieren** (Weitergabe → Abrechnung; Vormonat, Vorquartal, Vorhalbjahr,
  Vorjahr oder von–bis, Vergleich mit bis zu 10 Vorjahren, von Hand oder per E-Mail): Zählerstände zum Stichtag je Zähler als CSV für
  Excel, eine Zeile je Zähler und Wert mit Stand Anfang, Stand Ende und Verbrauch, oder als
  Zeitreihe (Stand je Tag oder Monatsende). Vorgaben: Vormonat, Hauptwert je Zähler; Zeitraum,
  weitere Werte und Dezimalzeichen einstellbar, je Projekt gespeichert, mit Vorschau. Die Werte
  kommen aus den gespeicherten Auslesungen, also auch rückwirkend für Werte, die nie als Datenpunkt
  gewählt waren. Stand am Stichtag: der Stichtagswert, den der Zähler selbst gespeichert hat, sonst
  die letzte Auslesung davor; die Quelle steht daneben, Fehlendes und unsichere Uhrzeit als Hinweis.
  Ein Zählertausch im Zeitraum wird verrechnet. Hinweise auch für erkannten, nicht bestätigten
  Tausch, im Zeitraum nicht gelesen, antwortet nicht, gestört im Zeitraum (von–bis) und Fehler
  des Zählers; einzelne Linien wählbar; über der
  Vorschau zusammengezählt („2 × antwortet nicht · 1 × Zählertausch“). **Automatisch:** nach Ende
  des Zeitraums (am Monatsanfang bzw. im Januar, nach der ersten Auslesung) legt mbusio die Datei
  im Datenordner unter `exporte/<projekt-nr>/` ab; verpasste Zeiträume (Gerät aus) werden
  nachgeholt, die Dateien stehen zum Herunterladen beim Export. **Per E-Mail:** mit Empfängern je
  Projekt geht jede neue Datei als Anhang raus (eigener kleiner SMTP-Client, STARTTLS/TLS,
  Mailserver einmal je Gerät, Testmail-Knopf); klappt es nicht, steht der Grund an der Datei und
  mbusio versucht es 7 Tage lang erneut
- Export der Messwerte als CSV (Semikolon, Excel) oder JSON; Projekte als Datei sichern
  („Projekt sichern“) und importieren (z.B. vom PC auf den Raspberry Pi)
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

### Windows als Dienst

Für den Dauerbetrieb (regelmässig auslesen und weitergeben) muss mbusio auch laufen, wenn niemand
angemeldet ist. Im Fenster gestartet, steht es still, sobald das Fenster zu ist; der Zustand der
Anlage zeigt das dann orange an.

1. `mbusio-dienst-einrichten.cmd` aus dem Release neben die .exe legen und doppelklicken. Windows
   fragt nach Adminrechten; danach wählen, ob die Oberfläche aus dem Netzwerk erreichbar sein soll
   (dann mit Zugriffsschlüssel). Ohne die Datei, in einer Eingabeaufforderung als Administrator:
   `mbusio-windows-x86_64.exe --install-service [--lan --token GEHEIM]`
2. Danach läuft mbusio als Dienst „mbusio“: startet mit Windows, ohne Anmeldung und ohne Fenster,
   nach einem Absturz neu. Ein Doppelklick auf die .exe öffnet nur noch den Browser.

- **Programm** liegt dann in `C:\Program Files\mbusio`, **Daten** in `C:\ProgramData\mbusio`. Beim
  ersten Einrichten wird die bisherige Datenbank dorthin kopiert (das Original bleibt).
- **Aktualisieren:** die neue .exe genauso einrichten; Daten bleiben.
- **Entfernen:** `mbusio-dienst-entfernen.cmd` bzw. `--uninstall-service`; Programm und Daten bleiben.
- Startet der Dienst nicht: `C:\ProgramData\mbusio\mbusio.log`.
- Der Dienst läuft als `LocalService` (wenig Rechte). Solange ein Plan läuft, schläft der PC nicht ein.
- **USB-Energiesparen ausschalten**, sonst schaltet Windows den M-Bus-Wandler ab: Energieoptionen →
  Erweiterte Einstellungen → USB-Einstellungen → „Selektives USB-Energiesparen“ deaktivieren, und im
  Geräte-Manager beim USB-Wandler „Computer kann das Gerät ausschalten“ abhaken.

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

**Reicht der Pi?** Vor dem Einsatz `mbusio lasttest` laufen lassen: erzeugt in einem eigenen
Ordner eine Anlage mit 60 Zählern und 3 Jahren Messwerten (Plan alle 15 min, im letzten Jahr jede
Auslesung, davor eine je Tag wie beim Speicherschutz), startet mbusio darauf und zeigt, wie lange
Status, Zählerliste, Verlauf, Bericht, Abrechnung, Export und Sicherung brauchen. Danach wird der
Ordner gelöscht; echte Daten fasst der Lasttest nie an. Grösse wählbar:
`mbusio lasttest --meters 120 --years 5 --minutes 15`. Zum Vergleich auf einem PC: 480 MB
Datenbank, Status 1 ms, Verlauf 60 ms, Abrechnung 30 ms, Sicherung herunterladen 1 s.

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

Alle Projekte, Zählerlisten, Auslesungen (mit Verlauf), letzten Scans, Änderungen und
Einstellungen (MQTT, Modbus, E-Mail samt Passwörtern) liegen in **einer Datei** `mbusio.db` (SQLite).

**Sicherung des Geräts:** Einstellungen (⚙) → Sicherung → „Herunterladen“ liefert diese Datei als
stimmige Kopie, auch während mbusio ausliest. Fällt das Gerät aus: auf dem neuen mbusio starten,
⚙ → Sicherung → „Einspielen …“, die Rückfrage zeigt Projekte, Zähler und Zeitraum; danach startet
mbusio neu und alles läuft wieder (Linien verbinden sich, Plan und Ausgänge wie vorher). Der
bisherige Stand bleibt als `mbusio.db.vorher` liegen. Eine Sicherung einer neueren mbusio-Version
lehnt eine ältere ab. Die Datei enthält die Passwörter: sicher aufbewahren. Einzelne Projekte gehen
weiter über „Projekt sichern“ als JSON-Datei (ohne Geräte-Einstellungen). Eine `meters.json` aus Version 0.3 wird beim
ersten Start ins Projekt „Standard“ übernommen.

**Automatische Sicherung** (Weitergabe → Automatische Sicherung): täglich zu einer einstellbaren
Uhrzeit auf ein **anderes** Gerät, damit ein Ausfall des Geräts selbst nicht auch die Sicherung
mitreisst. Ziel: ein Ordner (USB-Stick oder ein vom Betriebssystem eingebundenes Netzlaufwerk,
siehe unten) oder ein eigener kleiner FTP/FTPS-Client. Am Ziel bleiben nur die letzten paar
eigenen Sicherungen dieses Geräts (einstellbar); ältere löscht mbusio dort selbst, nichts anderes
im Ordner. Mit einem **Sicherungs-Passwort** verschlüsselt mbusio die Datei (AES-256-GCM,
Schlüssel aus PBKDF2-SHA256); dasselbe Passwort verschlüsselt dann auch „Sicherung herunterladen“
von Hand. Ohne das Passwort ist die Datei nicht mehr lesbar – wie einen Schlüssel aufbewahren.
Scheitert eine Sicherung, meldet mbusio das über die Meldungen (Statusleiste, E-Mail-Warnung).

**Netzlaufwerk (SMB) als Ziel einbinden:** mbusio hat dafür keinen eigenen Client, das kann das
Betriebssystem.
- **Windows:** die Freigabe (z. B. `\\server\freigabe`) direkt als Ordner-Pfad eintragen.
- **Raspberry Pi / Linux:** einmal in `/etc/fstab` einbinden, danach den Mountpunkt als Ordner
  eintragen:
  ```
  //server/freigabe /mnt/sicherung cifs credentials=/etc/mbusio-smb-credentials,uid=pi,iocharset=utf8,x-systemd.automount 0 0
  ```
  dazu `/etc/mbusio-smb-credentials` (nur root lesbar: `sudo chmod 600 /etc/mbusio-smb-credentials`):
  ```
  username=...
  password=...
  ```
  Dann `sudo mount -a` und `/mnt/sicherung` in den Einstellungen als Ordner eintragen.

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
sonst öffnet man ihn unter **Werkzeuge → Simulator**. Eingebaute Vorlagen:

| Simulation | Anschluss | Inhalt |
| --- | --- | --- |
| Praxis mit typischen Fehlern | `SIMULATOR` | 20 Zähler mit Fehlern aus der Praxis (Tabelle unten) |
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

Die simulierten Zählerstände (Energie, Volumen, Wasser, Strom) steigen mit der Zeit seit dem
1.10.2026 nach der Systemuhr, also auch über einen Neustart von mbusio hinweg, wie bei echten
Zählern.

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
- `src/mbus/decode/`, `tables.rs`: Datensätze decodieren (EN 13757-3): variable Struktur (mod),
  feste Struktur und Fehlermeldungen des Zählers (fixed), Rohbytes in Werte (values)
- `src/mbus/master/`: Verbindung, Auslesen, Ursachen von Störungen (mod), Typen (types),
  Protokoll (log), Primär- und Sekundärscan, Baudraten am Bus (scan), Adresse, Baudrate, Reset
  (config)
- `src/mbus/analyze/`: Telegramm-Analyse Byte für Byte, Verbindungsschicht (mod) und
  Anwendungsschicht (application)
- Tests liegen in `tests.rs` bzw. `tests/` neben dem Modul, grosse nach Thema geteilt
- `src/link.rs`: serielle Schnittstelle und TCP; `src/sim/`: Simulator (bus: der Bus, meter: ein simulierter Zähler, scenarios: Vorlagen und eigene Anlagen, store: Speichern), alle hinter demselben `Link`-Trait
- `src/db/`: Datenbank, nach Bereichen (types: Datentypen und Tabellenzeilen, schema mit Migrationen,
  projects, lines, meters, readings, schedule, report)
- `src/line.rs`: Linien-Dienst, eine Verbindung und Warteschlange je Linie (Bedienung vor Plan)
- `src/datapoint/`: Datenpunkte: mod (Datenpunkt, Schema, lesen, anlegen), key (Kennung eines
  Werts), quality (Qualität, Plausibilität), values (neue Werte aus einer Auslesung), select (Werte
  wählen), edit (ändern, mehrere auf einmal), list (Listen je Linie/Projekt), live (Stand für
  Node-RED), outputs (Ausgänge allgemein mit Schnittstelle `Output`, je Ausgang ein Modul: mqtt,
  home_assistant, modbus), templates (Vorlagen), exchange (Zählertausch), csv (Export/Import),
  project_file (aus einem Projekt-Export übernehmen)
- `src/main.rs`: Start, Webserver und Dienste; `src/cli.rs`: Befehle der Kommandozeile (JSON)
- `src/changeset.rs`: Änderungssatz für „Vorschau → Übernehmen“ (Importe, Vorlagen)
- `src/meterlist.rs`: Zählerliste importieren (über die ID), Namen nach Muster, CSV-Export
- `src/modbus/`: Modbus-TCP-Server: Einstellungen und Dienst (mod), Registerbild (image),
  Anfragen und Antworten (protocol)
- `src/mqtt/`: MQTT-Ausgang: Dienst (mod), Einstellungen und Topics (config), Einstellungen und
  Warteschlange in der Datenbank (store);
  eigener kleiner Client für MQTT 3.1.1 mit TLS über rustls (client)
- `src/plan.rs`, `src/scheduler/`: Ausleseplan und Zeitplan (mod: was fällig ist; reading: geplant lesen
  und Durchgang abschliessen; upkeep: Speicher, Ausdünnen, Uhrzeit); `src/clock.rs`: Uhrzeit und ob sie
  bestätigt ist; `src/storage.rs`: Schutz vor vollem Speicher
- `src/service/`: im Hintergrund laufen: erkennen (Dienst, systemd), Daten übernehmen, Log (mod);
  Windows-Dienst einrichten, entfernen, laufen (win, nur Windows)
- `src/web/`: HTTP-API: mod (Router, Zugriffsschlüssel, Dateien), state (gemeinsamer Zustand,
  Linien-Dienste), api (Fehler, Downloads, Arbeit am Bus), dann nach Bereichen (bus: Verbindung und
  Auftrag, scan, tools, meters, projects, schedule: Plan/Dauerbetrieb/Uhrzeit, datapoints, outputs,
  nodered, simulator); `web/`: Oberfläche und Bericht (in die Binärdatei eingebettet)
- `src/i18n.rs`: Übersetzungen der Messgrössen, Sprache je Request; Arbeit im Hintergrund (Zeitplan,
  Ausgänge, Linien) folgt der Sprache der Oberfläche (letzte Anfrage mit `X-Lang`); Meldungen fürs
  Protokoll entstehen mit `both` in beiden Sprachen und erscheinen in der Sprache, die fragt; alle
  Texte der Oberfläche und des Berichts in `web/js/texts.js` bzw. beim Bereich
- `web/index.html` und `web/app/`: Oberfläche als ES-Module ohne Build-Schritt (Konzept
  `docs/konzept-oberflaeche-neu.md`): `app.js` (Start, Laschen, Einstellungen),
  `app-kopf.js` und `app-statusleiste.js`, `store.js` (Zustand), `view.js` (Bausteine: Liste mit
  Schlüssel, Rückfrage am Ort, Rückgängig, Vorschau → Übernehmen), `keys.js` (Tastatur), `data.js`
  (Laden), `buslog.js` (Protokoll), dann je Bereich `<bereich>.js` mit `<bereich>.css` (scan,
  zaehler, weitergabe, anlage, werkzeuge, simulator), daneben `<bereich>-texte.js` und abgegrenzte
  Teile (z.B. `zaehler-verlauf.js`)
- `web/js/`: gemeinsam für Oberfläche und Bericht (`report.html`): `base.js` (Speicher, Sprache, API,
  Zugriffsschlüssel), `texts.js`, `icons.js`, `theme.js`, `scanlogic.js`, `listlogic.js`, `plot.js`;
  `web/css/`: `tokens.css` (Farben, Schrift), `meter.css`, `plot.css`
- `tests/ui/`: Browser-Tests der Oberfläche (siehe `tests/ui/README.md`)

Der Decoder ist eine Eigenentwicklung und wird gegen das Referenztelegramm aus der
M-Bus-Dokumentation getestet. Funktionsumfang wie libmbus: Primär-/Sekundärscan, Auslesen mit
mehreren Telegrammen, Primäradresse setzen, Baudrate umstellen, Application Reset,
Rohtelegramme, serielle und TCP-Verbindung, variable und feste Datenstruktur.


## Lizenz

MIT, siehe [LICENSE](LICENSE).
