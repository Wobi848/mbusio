# Konzept: Dauerbetrieb und Datenweitergabe

Stand: 17.10.2026 · Status: **Phase 1–3 umgesetzt** (Auslesen, Datenpunkte, MQTT); offen ist der Dauertest auf echter Hardware (siehe 5.1), nächste Phase Modbus TCP · gilt für mbusio ab Version 0.5

Dieses Dokument beschreibt, wie mbusio vom Inbetriebnahme-Werkzeug zu einem Dienst wird, der
Zähler dauerhaft und zuverlässig ausliest und die Werte weitergibt. Umgesetzt wird in Phasen;
**Phase 1 ist nur das zyklische Auslesen**. Weitergabe (MQTT, Modbus, BACnet) wird hier
vorgedacht, damit Phase 1 sie später nicht verbaut, aber erst nach eigener Abstimmung gebaut.

---

## 1. Ziel und Abgrenzung

**Ziel:** Auf einem Raspberry Pi oder Rechner im Dauerbetrieb liest mbusio alle Zähler eines
Projekts regelmässig aus, speichert die Werte und gibt ausgewählte Werte an ein Leitsystem weiter.
Ausfälle (Zähler, Linie, Gateway, Netzwerk, Strom) dürfen weder Daten verfälschen noch den Betrieb
anhalten, und jede Störung muss sichtbar sein.

**Rolle:** mbusio ist das **Gateway**. Raspberry Pi (oder Rechner) plus M-Bus-Master auf der einen
Seite, das Leitsystem (GLT) auf der anderen. Das Leitsystem spricht nie direkt mit dem M-Bus,
sondern holt die Werte über MQTT, Modbus oder BACnet von mbusio.

```
 Zähler ── M-Bus ── M-Bus-Master ── mbusio (Pi) ── LAN ── Leitsystem
                                        │
                                        └── Browser: Inbetriebnahme, Service, Bericht
```

**Vorrang:** Die Arbeit vor Ort (Scan, Auslesen, Beheben, Bericht) bleibt die **Hauptaufgabe**.
Der Dauerbetrieb darf sie nie behindern.

**Nicht Ziel:**
- mbusio ersetzt kein Leitsystem (keine Alarmierung per SMS, keine Abrechnung).
- Keine Steuerung von Zählern im Dauerbetrieb: Adressen, Baudraten usw. ändert nur ein Mensch.
- Kein Mehrbenutzer-Rechtesystem (weiterhin ein Zugriffsschlüssel für alles).

## 2. Grundsätze

Diese Regeln gelten für alle Phasen. Jede Entscheidung unten muss sich auf sie zurückführen lassen.

1. **Ein Bus, ein Besitzer.** Pro Linie spricht genau ein Programmteil mit dem Bus. Bedienung und
   geplante Auslesungen stellen sich an, statt sich gegenseitig „Bus belegt“ zu melden.
2. **Kein Wert ist erfunden.** Ein weitergegebener Wert ist immer ein echter, gelesener Wert mit
   Zeitstempel. Fehlt ein neuer Wert, wird der alte als *veraltet* gekennzeichnet, nie still
   weitergereicht oder durch eine Schätzung ersetzt.
3. **Jede Störung ist sichtbar.** Was nicht geklappt hat, steht in der Oberfläche, im Protokoll und
   (Phase 3) beim Empfänger als Qualitätsmerkmal. Keine stillen Fehler.
4. **Ein Fehler bleibt lokal.** Ein defekter Zähler verzögert nicht die anderen; eine gestörte Linie
   nicht die anderen Linien; ein nicht erreichbarer Empfänger nicht das Auslesen.
5. **Neustart-fest.** Nach Absturz, Stromausfall oder Update läuft alles von selbst weiter, ohne
   Eingriff, ohne doppelte und ohne verlorene Daten (soweit physikalisch möglich).
6. **Begrenzter Verbrauch.** Speicher, Datenmenge und Buslast wachsen nicht unbegrenzt; der Pi läuft
   jahrelang mit seiner SD-Karte.
7. **Einstellbar ohne Programmieren.** Alles wird in der Oberfläche festgelegt und ist als Datei
   exportierbar (Projekt-Export), damit eine Anlage reproduzierbar eingerichtet werden kann.

## 3. Ausgangslage (Version 0.4)

| Thema | Heute | Problem im Dauerbetrieb |
| --- | --- | --- |
| Verbindung | genau **eine** Verbindung, zur aktiven Linie | andere Linien werden nie gelesen |
| Buszugriff | wer zuerst kommt; sonst „Bus belegt“ | Plan und Bedienung blockieren sich |
| Speicherung | jede Auslesung vollständig als JSON, Ø 1,8 KB | 60 Zähler × 96/Tag ≈ 10 MB/Tag ≈ 4 GB/Jahr |
| Zeit | Systemzeit beim Auslesen | ohne Zeitsynchronisation (Pi ohne Netz) falsch |
| Wertkennung | Werte nur als Liste eines Telegramms | kein stabiler „Datenpunkt“ für die Weitergabe |

Phase 1 behebt die ersten vier Punkte; Phase 2 den letzten.

## 4. Architektur

```
             Oberfläche / API                Ausleseplan (je Zähler)
                   │  Aufträge                      │ Aufträge
                   ▼                                ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ Linien-Dienst  (einer je Linie, eigener Thread)              │
   │  • besitzt die Verbindung (seriell / TCP) dieser Linie       │
   │  • Warteschlange mit Vorrang: Bedienung > Plan               │
   │  • verbindet neu nach Abbruch, meldet Zustand                 │
   └──────────────┬───────────────────────────────────────────────┘
                  │ Auslesung (Rohtelegramme + Zeit + Ergebnis)
                  ▼
   ┌──────────────────────────────┐      ┌────────────────────────┐
   │ Messwert-Speicher (SQLite)   │─────▶│ Datenpunkte (Phase 2)  │
   │  kompakt, mit Aufbewahrung   │      │ Auswahl, Name, Einheit │
   └──────────────────────────────┘      └───────────┬────────────┘
                                                     │ Wert + Zeit + Qualität
                                ┌────────────────────┼────────────────────┐
                                ▼                    ▼                    ▼
                           MQTT (Ph. 3)        Modbus TCP (Ph. 4)   BACnet/IP (Ph. 5)
```

### 4.1 Linien-Dienst

- Für **jede Linie mit Anschluss**, die einen aktiven Plan hat oder gerade bedient wird, läuft ein
  eigener Linien-Dienst. Linien sind physisch getrennte Busse und dürfen daher **gleichzeitig**
  gelesen werden.
- Der Dienst ist der **einzige** Besitzer der Verbindung. Alle anderen geben ihm Aufträge
  (Auslesen, Scan, Adresse ändern …) und warten auf das Ergebnis.
- **Vorrang:** Aufträge aus der Bedienung kommen vor geplanten Auslesungen. Ein laufender Plan
  wird zwischen zwei Zählern unterbrochen, nicht mitten in einem Telegramm; danach geht er weiter.
- **Wiederverbinden:** Bricht die Verbindung ab (USB gezogen, Gateway neu gestartet), versucht der
  Dienst es mit wachsendem Abstand erneut (5 s, 10 s, 30 s, dann jede Minute) und meldet den
  Zustand „getrennt seit …“.
- **„Aktive Linie“ in der Oberfläche** bleibt, bedeutet aber nur noch „diese Linie sehe ich gerade
  an“, nicht mehr „nur diese ist verbunden“.

> **Randfall fremde Netzwerk-Gateways:** Hängt mbusio über ein fremdes M-Bus-TCP-Gateway am Bus
> und fragt das Leitsystem dasselbe Gateway direkt ab, geht das oft nicht: viele dieser Gateways
> erlauben nur einen TCP-Client. Im Normalfall (mbusio ist das Gateway) tritt das nicht auf.

**Mengen:** mbusio setzt **keine künstlichen Grenzen** für Linien, Zähler oder Datenpunkte. Die
echten Grenzen setzt die Physik: ein M-Bus-Master versorgt je nach Modell 3–250 Zähler, und eine
Linie kann pro Tag nur so viele Auslesungen schaffen, wie Buszeit da ist (siehe Buslast-Prüfung).
Mehrere Linien laufen parallel. Getestet wird gegen ein Referenzszenario von **20 Linien mit
zusammen 2000 Zählern** auf einem Raspberry Pi 4; darüber hinaus soll es weiter funktionieren,
nur ungetestet.

### 4.2 Wie oft auslesen?

Bei M-Bus ist „so oft wie möglich“ fast immer falsch:

- **Batteriezähler** (Wärme, Kälte, Wasser, Heizkostenverteiler) verbrauchen bei jeder Auslesung
  Strom. Viele Hersteller begrenzen die Zahl der Auslesungen pro Tag oder Monat; wird sie
  überschritten, antwortet der Zähler mit „zu viele Auslesungen“ (Fehlercode 09) oder gar nicht.
- **Die Werte ändern sich selten.** Viele Zähler aktualisieren ihre Werte intern nur alle paar
  Minuten bis Stunden; häufigeres Abfragen liefert denselben Wert.
- **Buszeit:** Eine Auslesung dauert bei 2400 Bd etwa 0,5–1 s, bei 300 Bd mehrere Sekunden.
- **Netzbetriebene Zähler** (Elektrizität, manche grosse Wärmezähler) vertragen häufiges Auslesen.

Daraus folgt: **Standard ist 1× pro Tag.** Häufiger nur dort, wo es einen Grund gibt (Lastgang,
Leistung, Überwachung) und der Zähler es verträgt.

| Plan | Typischer Einsatz |
| --- | --- |
| **Täglich** um eine Uhrzeit (Standard 00:05) | Zählerstände, Verbrauch pro Tag, Abrechnung |
| **Monatlich** am 1. um eine Uhrzeit | reine Stichtagsablesung |
| **Stündlich** | Überwachung, netzbetriebene Zähler |
| **Alle 15 min** | Lastgang bei Elektrizität, nur netzbetriebene Zähler |
| **Aus** | Zähler nur von Hand auslesen |

**Pro Datenpunkt** lässt sich nicht verschieden oft *auslesen*: ein Zähler liefert bei jeder
Auslesung immer alle Werte auf einmal. Ein eigenes Intervall pro Datenpunkt gibt es deshalb nur
fürs *Weitergeben* (Phase 2/3), höchstens so oft, wie der Zähler gelesen wird.

### 4.3 Ausleseplan

- **Vererbung Projekt → Linie → Zähler:** Das Projekt hat einen Standardplan. Jede Linie
  übernimmt ihn oder hat einen eigenen; jeder Zähler übernimmt den seiner Linie oder hat einen
  eigenen. So genügt bei den meisten Anlagen eine einzige Einstellung.
- **Standard ist immer „Aus“** (neue Projekte, Linien, Zähler). Geplantes Auslesen wird nur
  bewusst eingeschaltet; beim Einschalten schlägt die Oberfläche „täglich 00:05“ vor.
- **Termine im Raster der Uhr** (15 min: :00, :15, :30, :45; stündlich: :00; täglich: feste Zeit),
  damit Werte verschiedener Zähler und Tage vergleichbar sind.
- Der Linien-Dienst prüft jede Minute, welche Zähler **fällig** sind, und liest sie nacheinander
  aus. Sind zur selben Zeit viele Zähler fällig (typisch: alle täglich um 00:05), dauert das ein
  paar Minuten; der Zeitstempel ist immer der echte Zeitpunkt der Antwort.
- **Nie doppelt gleichzeitig:** Ein Zähler ist höchstens einmal in der Warteschlange.
- **Verpasster Termin** (mbusio aus, Stromausfall, Linie getrennt): sobald möglich wird der Zähler
  **einmal** nachgelesen, mit echtem Zeitstempel, danach geht es im Raster weiter. Bei Plänen
  „alle 15 min“ wird nicht jeder einzelne verpasste Termin nachgeholt, sondern nur einmal gelesen;
  die Lücke bleibt sichtbar.
- **Wie oft ein Zähler gelesen werden darf, weiss der Benutzer** (Herstellerangaben); mbusio
  gibt dazu keine Vermutungen ab.
- **Schutz, wenn der Zähler selbst ablehnt:** Meldet ein Zähler „zu viele Auslesungen“ (09) oder
  „beschäftigt“ (08), wird er für den Rest des Tages ausgesetzt (erneuter Versuch beim ersten
  Termin des nächsten Tages) und als Auffälligkeit gemeldet.
- **Buslast-Prüfung:** mbusio misst die Dauer jeder Auslesung. Passt die geplante Arbeit einer
  Linie nicht mehr in die Zeit (z.B. 250 Zähler alle 15 min bei 300 Bd), warnt die Oberfläche
  schon beim Einstellen.
- Springt die Uhr (Zeitsynchronisation nach dem Start, Sommerzeit), richten sich die Termine neu
  aus; der Sprung wird protokolliert. Intern wird in UTC gespeichert, angezeigt in Ortszeit.

### 4.4 Ablauf einer Auslesung im Plan

1. Fälligen Zähler auslesen wie heute („Alle auslesen“), mit eigener Baudrate falls nötig.
2. Schlägt es fehl: **einmal** wiederholen (nicht bei 08/09, siehe oben). Schlägt auch das fehl:
   als Fehler mit Ursache („Warum?“) speichern, nächster Zähler.
3. Ein Zähler gilt als **gestört**, wenn er **3 Termine nacheinander** nicht gelesen werden
   konnte; er erscheint dann in den Auffälligkeiten und wird trotzdem weiter versucht.
4. Eine Linie gilt als **gestört**, wenn bei einem Termin mit mehreren fälligen Zählern kein
   einziger antwortete (Leitung, Wandler, Gateway).

### 4.5 Speicherung und Aufbewahrung

Heute wird jede Auslesung vollständig als JSON abgelegt. Für den Dauerbetrieb:

- **Gespeichert wird das Rohtelegramm** (Hex, typ. 50–250 Byte) plus Zeit und Ergebnis.
  Die Werte werden beim Anzeigen aus dem Telegramm decodiert. Vorteil: klein, verlustfrei, und
  ein später verbesserter Decoder gilt auch für alte Daten.
- Zusätzlich eine **schlanke Wertetabelle** nur für die Werte, die für Verlauf und Weitergabe
  gebraucht werden (Phase 2: die gewählten Datenpunkte; Phase 1: der Hauptwert je Zähler).
- **Aufbewahrung: alles wird behalten.** mbusio löscht keine Messwerte von sich aus.
- **Abschätzung:** Normalfall (täglich) 250 Zähler × ~200 Byte ≈ 50 KB/Tag ≈ 18 MB/Jahr.
  Extremfall (60 Zähler alle 15 min) ≈ 1,2 MB/Tag ≈ 440 MB/Jahr.
- **Optional pro Projekt „Verlauf ausdünnen“** (bewusst einzuschalten, nie automatisch): z.B. nach
  90 Tagen nur noch einen Wert pro Tag behalten. Sinnvoll nur bei häufigen Plänen.
- **Schutz vor voller Speicherkarte:** mbusio überwacht den freien Speicher. Unter 20 % frei:
  Warnung in Oberfläche und Statusleiste. Unter 5 % frei: keine neuen Rohdaten mehr speichern
  (Werte werden weiter gelesen, angezeigt und weitergegeben), Auffälligkeit „Speicher voll“.
  So bleibt die Datenbank intakt und das Gerät bedienbar, statt abzustürzen.
- **Speicherschutz (26.10.2026, ersetzt „nie automatisch“; umgesetzt in `db/thinning.rs`,
  stündlich in `scheduler/upkeep.rs`, Einstellungen ⚙):** Droht der Speicher voll zu laufen,
  dünnt mbusio selbst aus, statt nur das Speichern einzustellen. Einstellbar, Vorgaben:
  - eingeschaltet;
  - Stufe „knapp“ (Warnung): Auslesungen und Werte **älter als 365 Tage** auf **eine je Zähler
    und Tag**; reicht das nicht, **älter als 90 Tage**; nie jünger als 90 Tage;
  - behalten wird je Tag die **letzte** Auslesung (Stand am Tagesende, genau das, was die
    Abrechnung braucht; Stichtagswerte des Zählers stecken in jeder Auslesung). Auch das
    Ausdünnen je Projekt behält ab jetzt die letzte statt der ersten (Fehler: die erste
    verfälschte den Stand am Tagesende um den Verbrauch des Tages);
  - danach **verkleinern**: Datenbank mit `auto_vacuum = INCREMENTAL` (neue Datenbanken von
    Anfang an, bestehende einmalig umgestellt, wenn genug Platz für eine Kopie frei ist), freie
    Seiten gehen mit `PRAGMA incremental_vacuum` an das Dateisystem zurück, ohne die ganze Datei
    neu zu schreiben;
  - Meldung im Protokoll und als Warnung: „Speicher knapp: Auslesungen vor dem … auf eine je Tag
    ausgedünnt, 120 MB frei geworden“;
  - „fast voll“ (unter 5 %) bleibt die letzte Stufe: keine neuen Rohdaten mehr.
- **Schreibverhalten:** Auslesungen eines Termins werden gesammelt in einem Schreibvorgang
  (Transaktion) gespeichert; SQLite im WAL-Modus. Das schont die SD-Karte und übersteht
  Stromausfall (Transaktion ganz oder gar nicht).
- Das bisherige JSON je Auslesung entfällt für geplante Auslesungen; manuelle Auslesungen
  speichern weiterhin vollständig (wenige, für Fehlersuche wertvoll).

### 4.6 Zeit

**Das Problem:** Ein Raspberry Pi hat keine eigene Uhr mit Batterie. Nach einem Stromausfall
startet er mit der zuletzt gespeicherten Zeit weiter. Diese Zeit ist **plausibel, aber falsch**
(um die Dauer des Ausfalls zu spät) – gefährlicher als eine offensichtlich falsche Zeit, weil man
es den Werten nicht ansieht.

**Zeitquellen** (eine oder mehrere, in dieser Reihenfolge bevorzugt):

| Quelle | Wie | Bemerkung |
| --- | --- | --- |
| Zeitserver (NTP) im Anlagennetz | Leitsystem, Router oder Server liefert NTP | übliche Lösung in GLT-Netzen, kein Internet nötig |
| **Leitsystem über das Protokoll** | BACnet TimeSynchronization/UTCTimeSynchronization; Modbus: Leitsystem schreibt Zeit in ein Register; MQTT: Zeit-Topic | kommt mit Phase 3–5 |
| Echtzeituhr-Modul am Pi | RTC-Modul mit Knopfzelle (wenige Franken) | empfohlen, wenn keine andere Quelle zuverlässig ist |
| Browser | „Uhrzeit vom Gerät übernehmen“ in der Oberfläche (Laptop/Handy des Technikers) | für Inbetriebnahme und Service |

**Regeln:**
- mbusio merkt sich, **woher und wann** die Uhrzeit zuletzt bestätigt wurde. Nach jedem Neustart
  gilt die Zeit als **unsicher**, bis eine Quelle sie bestätigt (NTP, Leitsystem, RTC-Modul).
- Geplante Auslesungen laufen auch bei unsicherer Zeit (die Zählerstände selbst sind richtig), aber
  jeder Wert trägt dann das Merkmal **„Zeit unsicher“**, sichtbar in Oberfläche, Bericht und
  Weitergabe. Wird die Zeit später bestätigt und lag sie daneben, werden die betroffenen Werte
  nicht umgeschrieben, sondern bleiben markiert (Grundsatz 2).
- Ohne jede Zeitquelle zeigt die Oberfläche dauerhaft „Uhrzeit nicht bestätigt“ mit den Optionen
  oben.
- Gespeichert wird in UTC, angezeigt in Ortszeit.
- Zusätzlich wird die **Uhrzeit des Zählers** (falls im Telegramm) gespeichert. Weicht sie mehr als
  15 Minuten von einer bestätigten Systemzeit ab, ist das eine Auffälligkeit („Uhr des Zählers
  falsch“) – wichtig für Stichtagswerte. Umgekehrt kann eine stark abweichende Zählerzeit ein
  Hinweis auf eine falsche Systemzeit sein.

## 5. Phase 1: Zyklisches Auslesen – Umfang

**Zeit in Phase 1:** Erkennen, ob die Systemzeit von NTP/RTC bestätigt ist, „Uhrzeit vom Gerät
übernehmen“ im Browser, Kennzeichnung „Zeit unsicher“. Zeit vom Leitsystem folgt mit dem
jeweiligen Protokoll (Phase 3–5).

**Einstellungen:** Plan im Projekt (Vorgabe), je Linie „wie Projekt“ oder eigener Plan, je Zähler
„wie Linie“ oder eigener Plan. Pläne: Aus · Monatlich · Täglich · Stündlich · 15 min, mit Uhrzeit
bzw. Tag. Standard überall „Aus“. Optional pro Projekt: Verlauf ausdünnen.

**Anzeige:**
- Projekt-Karte: je Linie „täglich 00:05 · nächster Termin morgen 00:05 · zuletzt ✓ 58/60“.
- Zählerliste: Plan je Zähler, nächster Termin, Ergebnis des letzten Termins; gestörte und
  ausgesetzte Zähler hervorgehoben.
- Statusleiste: „Plan Haus A liest · 12/60“, ohne die Bedienung zu blockieren.
- Verlauf: füllt sich automatisch; Lücken (ausgelassen, gestört) sind sichtbar.

**Abnahmekriterien Phase 1** (alle müssen erfüllt sein):
1. 3 Linien mit je 20 Simulator-Zählern laufen 7 Tage auf einem Raspberry Pi, gemischt mit
   Plänen täglich, stündlich und 15 min; keine Lücke ausser absichtlich erzeugten,
   Datenbank < 20 MB.
2. Während geplante Auslesungen laufen, funktionieren Scan, Auslesen und Adresse ändern in der
   Oberfläche ohne „Bus belegt“.
3. Stromausfall (Stecker ziehen) während geplanter Auslesungen: nach dem Neustart keine kaputte
   Datenbank, die nächsten Termine kommen pünktlich.
4. USB-Wandler abziehen und wieder einstecken bzw. Gateway neu starten: Linie meldet sich als
   getrennt und läuft ohne Eingriff weiter.
5. Defekter Zähler (Simulator: Prüfsumme, Wackelkontakt): die anderen Zähler werden pünktlich
   gelesen, der Defekte erscheint nach 3 Terminen als gestört.
6. Zähler mit „zu viele Auslesungen“ (Simulator, neu): wird für den Rest des Tages ausgesetzt,
   am nächsten Tag wieder gelesen, Auffälligkeit gemeldet.
11. Speicher fast voll (Test mit kleinem Datenträger): Warnung ab 20 %, ab 5 % keine Rohdaten
    mehr, Datenbank bleibt lesbar, Oberfläche bedienbar.
7. mbusio über Nacht aus (über den Termin 00:05): nach dem Start wird jeder Zähler einmal
   nachgelesen, mit echtem Zeitstempel.
8. Uhr springt (Zeit von Hand verstellen): Protokolleintrag, Termine richten sich neu aus,
   keine doppelten Zeitstempel.
9. Neustart ohne Zeitquelle: Werte werden gelesen und als „Zeit unsicher“ gekennzeichnet; nach
   „Uhrzeit vom Gerät übernehmen“ sind neue Werte wieder unmarkiert.
10. Referenzszenario 20 Linien / 2000 Zähler (Simulator) auf einem Raspberry Pi 4: Oberfläche
    bleibt bedienbar, alle täglichen Termine werden innerhalb einer Stunde erledigt.

### 5.1 Stand der Abnahme (17.10.2026)

| Nr. | Kriterium | Stand | Wie geprüft |
| --- | --- | --- | --- |
| 1 | 3 Linien, gemischte Pläne, keine Lücken, DB < 20 MB | ✓ im Zeitraffer, **Pi offen** | Zeitraffer 48× über 2 Tage, 3 Linien (täglich/stündlich/15 min) mit 14 Zählern, davon 2 absichtlich fehlerhaft: 790 Auslesungen, 0 Lücken, 0 doppelt, `integrity_check` ok, Datenbank 147 KB (Rohtelegramm Ø 54 Byte im Simulator, echte Zähler 100–250). Hochgerechnet auf 60 Zähler und 7 Tage etwa 3–6 MB |
| 2 | Bedienung während geplanter Auslesungen, kein „Bus belegt“ | ✓ | Test Linien-Dienst (Bedienung vor Plan), Browser: Auslesen während Scan wartet und klappt |
| 3 | Stromausfall | ✓ Datenbank, **echter Stecker offen** | 8× `kill -9` mitten im Schreiben: `integrity_check` ok, keine doppelten Zeitstempel, Neustart läuft weiter |
| 4 | USB abziehen / Gateway neu starten | ✓ USB am echten Gerät (20.10.2026), **Gateway offen** | Relay MR003 mit Itron-Wärmezähler, Plan 15 min, über `/dev/serial/by-id/…`: abgezogen 20:42 → Termin 20:45 „nicht verbunden, Zähler bleiben fällig“, Werte sofort `disturbed` → eingesteckt 20:47:28 → nach 6 s verbunden und Termin nachgeholt, Werte wieder `good`, ohne Eingriff. Noch offen: Einstecken, bevor mbusio den Ausfall bemerkt (Linux vergibt dann `ttyUSB1`) |
| 5 | Defekter Zähler gestört nach 3 Terminen | ✓ | Test und Zeitraffer (Prüfsummen-Zähler, andere pünktlich gelesen) |
| 6 | „Zu viele Auslesungen“ ausgesetzt bis nächster Tag | ✓ | neuer Simulator-Zähler 19 (2 Auslesungen, dann 09); Test und Zeitraffer |
| 7 | Über Nacht aus: einmal nachlesen | ✓ | Test (3 Tage verpasst → genau eine Auslesung mit echtem Zeitstempel) |
| 8 | Uhr springt | ✓ | Tests: Sommerzeit-Ende ohne doppelte Termine, Sprung um einen Tag zurück richtet neu aus; Sprung wird protokolliert |
| 9 | Ohne Zeitquelle „Zeit unsicher“, nach Übernahme unmarkiert | ✓ | `--untrusted-clock`: Auslesung markiert, nach „Uhrzeit vom Gerät übernehmen“ unmarkiert, im Verlauf sichtbar |
| 10 | 20 Linien / 2000 Zähler | ✓ Rechenzeit, **Pi offen** | Test: eine Prüfung aller 2000 Zähler 0,1–0,2 s, Projektbaum 0,14 s (Debug-Build, PC) |
| 11 | Speicher fast voll | ✓ Logik, **kleiner Datenträger offen** | Tests: Stufen 20 %/5 % (auf grossen Platten mit absoluter Grenze), bei „voll“ wird gelesen und angezeigt, aber nicht gespeichert |

**Noch auf echter Hardware zu prüfen, bevor ein Release als „dauerbetriebstauglich“ gilt:**
7 Tage auf einem Raspberry Pi 4 (Kriterien 1 und 10), Stecker ziehen (3), USB-Wandler abziehen (4),
kleiner Datenträger (11). Der Zeitraffer (`--time-scale 48`, ein Tag in 30 min) ersetzt das nicht,
macht es aber planbar.

**Umsetzung Speicherung** (Schritt 3):
- Geplante Auslesungen: nur Rohtelegramme (BLOB), Zeitstempel als Ortszeit und zusätzlich UTC;
  von Hand gelesene weiterhin zusätzlich als JSON. Beim Anzeigen wird decodiert.
- Je Zähler eine Transaktion (Auslesung, Zählerliste, Termin zusammen) statt einer je Termin:
  so geht bei einem Abbruch höchstens der gerade gelesene Zähler verloren und wird nachgeholt.
  SQLite WAL mit `synchronous=NORMAL` schont die SD-Karte.
- „Verlauf ausdünnen“ je Projekt: 30, 90 oder 365 Tage, danach der erste Wert je Zähler und Tag.
- Speicherschutz: „knapp“ unter 20 % frei (und weniger als 10 GB), „voll“ unter 5 % (und weniger
  als 1 GB) oder unter 200 MB. „Knapp“ zeigt die Oberfläche nur, wenn ein Plan läuft.

## 6. Phase 2: Datenpunkte (was weitergegeben wird)

Weitergegeben werden nicht „Zähler“, sondern **Datenpunkte**: ein bestimmter Wert eines bestimmten
Zählers, z.B. „Whg. 3 Wärme – Energie aktuell (kWh)“. Was weitergegeben wird und wohin, muss
**frei wählbar** sein – pro Datenpunkt, ohne Programmieren, und für grosse Anlagen schnell.

### 6.1 Zuordnung (was geht wohin)

Kern ist eine **Zuordnungstabelle**: Zeilen sind Datenpunkte, Spalten die Ausgänge.

| Datenpunkt | Einheit | MQTT | Modbus | BACnet |
| --- | --- | --- | --- | --- |
| Whg 1 Wärme · Energie | kWh | ✓ | ✓ Reg. 100 | ✓ AI 1 |
| Whg 1 Wärme · Vorlauftemperatur | °C | ✓ | – | ✓ AI 2 |
| Allgemeinstrom · Energie Tarif 1 | kWh | – | ✓ Reg. 104 | – |

- Ein Datenpunkt kann an **keinen, einen oder mehrere** Ausgänge gehen.
- **Vorlagen** für schnelles Arbeiten: „bei allen Wärmezählern dieser Linie: Energie, Volumen,
  Vorlauf, Rücklauf weitergeben“ – angewendet auf alle passenden Zähler, auch auf später
  hinzukommende (einstellbar).
- **Massenbearbeitung:** mehrere Datenpunkte markieren und gemeinsam einem Ausgang zuordnen.
- **Als CSV exportieren und wieder importieren:** Für grosse Anlagen wird die Zuordnung in Excel
  bearbeitet und zurückgespielt. Beim Import zeigt mbusio vorher, was sich ändert, und übernimmt
  erst nach Bestätigung.
- Adressen in den Protokollen (Modbus-Register, BACnet-Objektnummer, MQTT-Topic) vergibt mbusio
  automatisch und **stabil**; sie können überschrieben werden.
- Die Zuordnung gehört zum Projekt und wird mit exportiert.

### 6.2 Eigenschaften eines Datenpunkts

- **Auswahl** je Zähler aus seinen Werten (Liste wie in der Verlaufsansicht), mit Name und
  optionaler Umrechnung (z.B. Wh → kWh). Vorlage: Hauptwert automatisch vorgeschlagen.
- **Weitergabe je Datenpunkt:** bei jeder Auslesung, nur bei Änderung, oder seltener als gelesen
  (z.B. Leistung bei jeder Auslesung, Zählerstand nur 1×/Tag). Nie häufiger als der Zähler
  gelesen wird (Grundsatz 2).
- **Stabile Kennung:** Ein Datenpunkt wird über die Bedeutung des Wertes erkannt
  (Grösse, Einheit, Speicher, Tarif, Funktion – also DIF/VIF), **nicht** über seine Position im
  Telegramm. Ändert ein Zähler die Reihenfolge seiner Werte (Firmware-Update), bleibt die Zuordnung
  richtig.
- **Qualität** je Datenpunkt: *gut* · *veraltet* (letzter Wert älter als 3 Intervalle) · *gestört*
  (Zähler antwortet nicht) · *ungültig* (Wert vom Zähler als Fehler markiert).
- **Plausibilität:** Zählerstände dürfen nicht sinken. Sinkt einer, wird der Wert als *ungültig*
  gekennzeichnet und eine Auffälligkeit gemeldet („Zählerstand gesunken – Zählertausch?“).
- **Zählertausch:** eigene Aktion „Zähler getauscht“: alter Endstand und neuer Anfangsstand werden
  festgehalten, der Datenpunkt bleibt derselbe (für Leitsystem und Verlauf durchgehend).
- Export der Datenpunkt-Liste als CSV (für den Integrator: Name, Einheit, Kennung, Adresse im
  jeweiligen Protokoll).

### 6.3 Umsetzung (17.10.2026)

- **Kennung** = `Funktion|Speicher|Tarif|Untereinheit|Grösse|Einheit` (aus DIF/VIF decodiert),
  z.B. `instantaneous|0|0|0|energy|kWh`.
- **Nummer** je Projekt fortlaufend, nie wieder vergeben (auch nicht nach Löschen); Grundlage für
  Modbus-Register und BACnet-Objekte. Bleibt beim Projekt-Export/-Import erhalten.
- **Werte** je Datenpunkt zusätzlich in einer schlanken Tabelle (Zeit UTC, Wert, gültig) für
  Verlauf und Weitergabe; „Verlauf ausdünnen“ gilt auch für sie.
- **Ungültig** wenn das Statusbyte des Zählers einen Fehler meldet (0x08/0x10) oder ein
  Zählerstand (Energie, Volumen, Masse, HKV; aktuell, Speicher 0) sinkt. Verglichen wird mit dem
  letzten plausiblen Wert; Meldung im Protokoll, Zähler in der Projekt-Karte markiert.
- **Veraltet** nur mit eingeschaltetem Ausleseplan (älter als 3 Intervalle). **Gestört**: mit Plan
  nach 3 verpassten Terminen, ohne Plan wenn die letzte Auslesung fehlschlug.
- **Zählertausch:** der neue Zähler wird an der neuen Adresse ausgelesen; je Zählerstand wird die
  Differenz (alter Endstand − neuer Anfangsstand) dauerhaft addiert. Ins Änderungsprotokoll
  (Bericht) mit beiden Ständen.
- **Vorlagen** je Linie und Medium; „auch für neue Zähler“ wirkt einmal je Zähler nach seiner
  ersten Auslesung (ein bewusst gelöschter Datenpunkt kommt nicht wieder).
- **CSV:** Semikolon, UTF-8 mit BOM (Excel), feste Spaltenreihenfolge
  `Nr;Linie;Zähler;Adresse;Kennung;Name;Einheit;Faktor;Weitergabe;Aktiv`. Zuordnung über die Nr;
  Zeilen ohne Nr legen neue Datenpunkte an; fehlende Zeilen löschen nichts. Bei einem Fehler wird
  nichts übernommen.
- Die Zuordnung zu Ausgängen (Spalten MQTT, Modbus …) kommt mit den Ausgängen selbst.

## 7. Phasen 3–5: Ausgänge (Skizze)

Alle Ausgänge lesen dieselben Datenpunkte mit Wert, Zeit und Qualität. Ein Ausgang, der nicht
funktioniert (Broker weg, Netzwerk weg), beeinflusst weder das Auslesen noch die anderen Ausgänge.

### Phase 3: MQTT
- Ein Topic je Datenpunkt, z.B. `mbusio/<projekt>/<linie>/<zähler>/<wert>`, Nutzlast JSON
  `{"value": 1234.5, "unit": "kWh", "time": "…Z", "quality": "good"}`; retained; QoS 1.
- Verfügbarkeit über „Last Will“ (`…/status` = `offline`), damit der Empfänger einen Ausfall
  von mbusio selbst erkennt.
- **Zwischenspeicher:** Ist der Broker nicht erreichbar, werden Werte (begrenzt, z.B. 7 Tage)
  gesammelt und in richtiger Reihenfolge nachgeliefert.
- Benutzername/Passwort und TLS.

**Umsetzung (17.10.2026):**
- Eigener schlanker Client für MQTT 3.1.1 (CONNECT, PUBLISH QoS 1, PUBACK, SUBSCRIBE, PING) statt
  einer grossen Bibliothek: jede Nachricht wird erst nach der Bestätigung des Brokers aus der
  Warteschlange (Tabelle `mqtt_queue`) gelöscht. TLS mit rustls (ring), Zertifikate von Mozilla
  oder eigenes CA-Zertifikat; „nicht prüfen“ für selbst signierte Broker.
- Ein Broker für die ganze Installation (nicht je Projekt), weil die Zugangsdaten nicht in den
  Projekt-Export gehören. Die Zuordnung (MQTT je Datenpunkt, Topic) gehört zum Projekt.
- Topic relativ zum Basis-Topic gespeichert; wird beim Einschalten aus Projekt, Gateway, Linie,
  Zähler und Grösse (sprachunabhängig, z.B. `energy-t1`) erzeugt und danach nicht mehr verändert.
- Grenzen der Warteschlange: 7 Tage oder 200 000 Nachrichten (älteste zuerst verworfen, mit
  Meldung). Gesammelt wird nur, solange MQTT eingeschaltet ist.
- Uhrzeit vom Leitsystem über `<basis>/time`; eine Systemquelle (NTP, Uhr-Modul) hat Vorrang.
- Getestet gegen einen echten Broker: retained, Nachlieferung in Reihenfolge nach Broker-Ausfall,
  Last Will bei `kill -9`, Uhrzeit per MQTT, Passwort nirgends sichtbar.

**Ergänzung (18.10.2026): Topic-Muster und Liste für den Integrator**
- **Topic-Muster** in den MQTT-Einstellungen, z.B. `{projekt}/{gateway}-{linie}/{zaehler}/{wert}`
  (der bisherige feste Aufbau). Platzhalter: `{projekt}`, `{gateway}`, `{linie}`, `{zaehler}` (Name,
  sonst ID), `{id}` (ID vom Typenschild), `{pa}`, `{ort}`, `{medium}` (sprachunabhängig, z.B.
  `heat-outlet`), `{wert}` (Grösse, z.B. `energy-t1`), `{nr}` (Nummer des Datenpunkts). Jeder Teil
  wird für Topics lesbar gemacht (Kleinbuchstaben, Umlaute ausgeschrieben); leere Teile fallen weg.
- Neue Datenpunkte bekommen ihr Topic nach dem Muster. **„Alle Topics nach Muster setzen …“**
  rechnet alle Topics des Projekts neu, zeigt zuerst die Änderungen (Vorschau → Übernehmen wie bei
  den Importen) und sendet danach den aktuellen Wert unter dem neuen Topic. Doppelte Topics werden
  mit der Nummer des Datenpunkts eindeutig gemacht. Von Hand geänderte Topics bleiben möglich.
- **Liste für den Integrator** je Ausgang, bei MQTT im Kasten des Ausgangs: CSV (Excel) und JSON.
  Je Datenpunkt das volle Topic mit Basis, Nr., Name, Einheit, Faktor, Weitergabe, Zähler, ID, PA,
  Ort, Medium, Linie. Das JSON enthält dazu Broker-unabhängige Angaben: Basis-Topic, Status-Topic
  mit Last Will (online/offline), QoS 1, retained und den Aufbau der Nutzlast. Allgemein für alle
  Ausgänge gebaut (Modbus bekommt später dieselbe Liste mit Registern).
- In der Tabelle der Datenpunkte wird das Topic von links gekürzt (sichtbar bleibt das Ende, z.B.
  `…/whg-1/energy`), das volle Topic steht im Tooltip.

### Phase 4: Modbus TCP (Server)
- mbusio stellt die Datenpunkte als **Eingangsregister** bereit (nur lesen).
- Je Datenpunkt fester Registerblock: Wert (float32 oder int32 mit Skalierung), Zeit seit letzter
  Aktualisierung, Qualität. Wortreihenfolge einstellbar (Leitsysteme sind sich da uneins).
- **Registeradressen bleiben stabil**, auch wenn Datenpunkte dazukommen oder wegfallen
  (freie Lücken statt Verschieben). Die Registertabelle wird als CSV/PDF für den Integrator
  ausgegeben.
- Modbus hat keine Zugangskontrolle: nur auf einer gewählten Netzwerkschnittstelle anbieten.

### Phase 5: BACnet/IP
- Je Datenpunkt ein Objekt *Analog Input* mit Einheit (BACnet engineering units), Status_Flags
  aus der Qualität, COV-Benachrichtigung.
- Vorher zu klären: Gerätenummer (Device Instance) eindeutig im Netz, ob BBMD/Foreign Device
  nötig ist, und welche **Rust-Bibliothek** zuverlässig genug ist. BACnet ist der aufwendigste
  und riskanteste Teil; Entscheidung erst nach Phase 3.

## 8. Sicherheit

- Weboberfläche weiterhin standardmässig nur lokal; im Netzwerk nur mit Zugriffsschlüssel.
- Zugangsdaten (MQTT) in der Datenbank, nie im Projekt-Export und nie im Protokoll.
- Ausgänge sind einzeln abschaltbar; ohne Konfiguration ist keiner aktiv.

## 9. Teststrategie

- **Simulator mit Zeitraffer:** Ein simulierter Tag in Minuten, um Zeitplan, Aufbewahrung und
  Lücken automatisch zu testen.
- **Fehler-Einspeisung:** vorhandene Fehlerzähler plus „Linie fällt aus“, „Gateway trennt“,
  „Uhr springt“.
- **Dauertest** auf einem echten Raspberry Pi (7 Tage) vor jedem Release mit Dauerbetrieb.
- **Stromausfall-Test** von Hand (Abnahmekriterium 3).
- Automatische Tests für jede Regel aus Abschnitt 4 (Überlappen, Nachholen, Wiederholen,
  „gestört nach 3“, Aufbewahrung).

## 10. Entscheidungen und offene Fragen

**Entschieden** (17.10.2026):
- mbusio ist das Gateway; das Leitsystem holt die Werte bei mbusio.
- Keine künstlichen Mengengrenzen; Referenztest 20 Linien / 2000 Zähler.
- Pi ohne Internet und Zeitserver muss funktionieren; Zeit bevorzugt vom Leitsystem bzw. NTP im
  Anlagennetz, sonst RTC-Modul oder Browser; unsichere Zeit wird gekennzeichnet.
- Manuelle Arbeit hat immer Vorrang vor geplanten Auslesungen.
- Weitergabe frei wählbar pro Datenpunkt und Ausgang, mit Vorlagen und CSV-Bearbeitung.
- Ausleseplan mit Vererbung Projekt → Linie → Zähler; Standard überall „Aus“.
- Zulässige Anzahl Auslesungen ist Sache des Benutzers; mbusio reagiert nur auf Ablehnung durch
  den Zähler (08/09).
- Messwerte werden standardmässig nie gelöscht; Ausdünnen nur auf Wunsch; Schutz vor voller Karte.
- Reihenfolge der Ausgänge: **MQTT, dann Modbus TCP**; BACnet später nach Bedarf.

**Offen:** keine Punkte, die Phase 1 blockieren.

**Umsetzungsnotizen** (Phase 1, Schritt 2):
- Termine liegen im Raster der Ortszeit (00:05 soll 00:05 bleiben, auch nach der Zeitumstellung).
  Jede Auslesung trägt den Zeitstempel in Ortszeit und zusätzlich in UTC (eindeutig auch beim
  Ende der Sommerzeit). Rücksprünge bis 2 h werden ausgesessen, damit kein Termin doppelt kommt.
- „Uhrzeit vom Gerät übernehmen“ stellt nicht die Systemuhr (bräuchte Root-Rechte), sondern
  mbusio rechnet mit der Abweichung weiter. Bestätigt danach NTP oder ein Uhr-Modul die Zeit,
  gilt wieder die Systemuhr.
- Linie „gestört“ (kein Zähler hat geantwortet) und gestörte/ausgesetzte Zähler erscheinen in der
  Projekt-Karte, der Zählerliste und im Protokoll; in die Scan-Auffälligkeiten (Statusleiste)
  fliessen sie nicht ein, weil diese aus dem letzten Scan berechnet werden.

## 11. Roadmap und Stand (18.10.2026)

| Phase | Inhalt | Stand |
| --- | --- | --- |
| 1 | Linien-Dienst, Ausleseplan, Uhrzeit, kompakte Speicherung, Speicherschutz | ✓ umgesetzt, Hardware-Tests offen (5.1) |
| 2 | Datenpunkte, Qualität, Plausibilität, Zählertausch, Vorlagen, CSV | ✓ umgesetzt |
| 3 | MQTT | ✓ umgesetzt, gegen Test-Broker geprüft |
| 4 | Modbus TCP (Server, Registertabelle für den Integrator) | ✓ umgesetzt 25.10.2026, siehe konzept-modbus.md |
| 5 | BACnet/IP | offen, nur bei Bedarf und nach Prüfung einer Bibliothek |

**Offen, bevor ein Release als dauerbetriebstauglich gilt:**
- 7 Tage auf einem Raspberry Pi 4 (Kriterien 1 und 10), Stecker ziehen (3), USB-Wandler abziehen
  bzw. Gateway neu starten (4), kleine SD-Karte (11). Hilfsmittel: `--time-scale 48`.
- ~~MQTT mit einem echten Broker prüfen~~ – erledigt am 19.10.2026 (vom Benutzer mit einem echten
  Broker getestet).
- ~~Zwei Linien am selben Port~~ – erledigt am 19.10.2026: Serielle Ports waren schon exklusiv
  (`serialport` öffnet mit TIOCEXCL und flock), nur die Meldung war „Device or resource busy“;
  TCP-Gateways waren gar nicht geschützt. Jetzt belegt jeder Linien-Dienst seinen Anschluss vor dem
  Öffnen in einer gemeinsamen Liste (`line::Ports`) und gibt ihn mit der Verbindung wieder frei.
  Besetzt heisst: eine andere Linie hat gerade eine offene Verbindung, auch eine Plan-Linie im
  Hintergrund. Meldung: „<Anschluss> wird schon von Linie „…“ im Projekt „…“ benutzt“, beim
  Verbinden von Hand wie als Grund für „gestört“ bei geplanten Auslesungen (gleiche Stelle).
  Verglichen wird über `link::port_key` (`tcp://Host:Port` = `host:port`, `COM3` = `com3`);
  Simulationen sind ausgenommen.
- **USB abziehen im Dauerbetrieb** (Prüfstand 19.10.2026: virtueller USB-Port mit Fake-Zähler,
  Plan 15 min im Zeitraffer, MQTT an lokalen Broker): Linie meldet „nicht verbunden, Zähler bleiben
  fällig“, nach dem Einstecken liest sie ohne Eingriff weiter und holt den Termin einmal nach.
  Zwei Lücken geschlossen:
  - Werte einer Linie, die beim letzten Termin nicht erreichbar war (oder deren Zähler alle nicht
    antworteten), gehen sofort als `disturbed` an die Ausgänge, nicht erst nach drei Abständen als
    `stale` (bei „täglich“ wären das 3 Tage gewesen).
  - Linux vergibt nach dem Wiedereinstecken oft `/dev/ttyUSB1`, solange der alte Anschluss noch
    offen ist. Die Port-Auswahl liefert für USB-Wandler darum den festen Namen aus
    `/dev/serial/by-id/`; angezeigt wird er lesbar (`portText`), `port_key` löst ihn auf das Gerät
    auf. Bestehende Linien mit `/dev/ttyUSB0` einmal neu wählen.
- **Windows im Hintergrund:** als Windows-Dienst umgesetzt ([konzept-hintergrund.md](konzept-hintergrund.md));
  Prüfung auf einem Windows-PC nach der Checkliste dort (Abschnitt 9) offen.
- Danach Release v0.5.0 (bisher letztes Release: v0.4.0).

**Ausserhalb der Phasen umgesetzt:** Simulator-Reiter mit Vorlagen (Praxis mit Fehlern, alles
i.O. mit 10 Zählern, Mehrfamilienhaus mit 30 Zählern, gemischte Baudraten) und eigenen Anlagen.
Zählerliste schnell füllen: Liste vom Kunden importieren (über die ID, mit Vorschau), Benennen
direkt in der Tabelle, Namen nach Muster, CSV-Export; Vorlagen auf Wunsch gleich
mit MQTT. Bedienkonzept umgesetzt (Reiter Weitergabe, Bausteine), dazu die Karte „Nächster
Schritt“ (PR #2). Code aufgeräumt: nichts doppelt, grosse Dateien nach Bereichen aufgeteilt.

**Oberfläche v2** (Entwurf, Übergabe in [uebergabe-design-v2.md](uebergabe-design-v2.md)): Kopfzeile
für das Projekt, Linien-Leiste mit Verbindung, Laschen je Linie, Zähler-Baustein mit Medium-Symbol,
Bearbeiten in der Zeile, Auffälligkeiten an ihrer Zeile. Umsetzung schrittweise, je ein Commit.

**Nächste Ideen: Scan** (noch nicht umgesetzt; Konzept in [konzept-scan.md](konzept-scan.md)):
Änderungen seit dem letzten Scan erkennen und hervorheben (neu, fehlt, geändert), Sekundärscan über
ein ID-Präfix eingrenzen und nach Abbruch weitersuchen, Schnellprüfung der bekannten Zähler.

**Bekannte Grenzen** (siehe auch README): Antworten zweier Zähler exakt gleichzeitig mit `E5`
können wie eine Antwort aussehen; CI 76/77 nicht unterstützt; die ID eines Zählers lässt sich
nicht ändern.
