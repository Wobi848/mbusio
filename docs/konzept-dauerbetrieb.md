# Konzept: Dauerbetrieb und Datenweitergabe

Stand: 17.10.2026 · Status: **Entwurf zur Abstimmung** · gilt für mbusio ab Version 0.5

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

> **Wichtig für Netzwerk-Gateways:** Viele M-Bus-Gateways erlauben nur **einen** TCP-Client
> gleichzeitig. Fragt auch ein Leitsystem dasselbe Gateway ab, können nicht beide verbunden sein.
> Das muss je Anlage geklärt werden (siehe offene Fragen).

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

- **Die Linie hat einen Standardplan** (z.B. täglich 00:05). **Jeder Zähler** übernimmt ihn
  oder bekommt einen eigenen Plan. So genügt bei den meisten Anlagen eine Einstellung pro Linie.
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
- **Schutz vor zu häufigem Auslesen:**
  - Für Wärme, Kälte, Wasser und Heizkostenverteiler zeigt die Oberfläche bei Plänen häufiger als
    täglich den Hinweis „vermutlich Batteriezähler – Herstellerangaben zur zulässigen Anzahl
    Auslesungen prüfen“. Ob ein Zähler eine Batterie hat, verrät M-Bus nicht sicher; deshalb
    Hinweis, kein Verbot.
  - Meldet ein Zähler „zu viele Auslesungen“ (09) oder „beschäftigt“ (08), wird er für den Rest
    des Tages ausgesetzt (erneuter Versuch beim ersten Termin des nächsten Tages) und als
    Auffälligkeit gemeldet.
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
- **Aufbewahrung (einstellbar, Vorschlag):** Rohtelegramme 90 Tage, danach nur noch eines pro Tag
  (Tageswert, z.B. 00:00); Tageswerte unbegrenzt.
- **Abschätzung:** Im Normalfall (täglich) 250 Zähler × ~200 Byte ≈ 50 KB/Tag ≈ 18 MB/Jahr.
  Im Extremfall (60 Zähler alle 15 min) ≈ 1,2 MB/Tag; mit 90 Tagen Aufbewahrung ≈ 100 MB, danach
  ~4 MB/Jahr. Beides verträgt jede SD-Karte.
- **Schreibverhalten:** Auslesungen eines Termins werden gesammelt in einem Schreibvorgang
  (Transaktion) gespeichert; SQLite im WAL-Modus. Das schont die SD-Karte und übersteht
  Stromausfall (Transaktion ganz oder gar nicht).
- Das bisherige JSON je Auslesung entfällt für geplante Auslesungen; manuelle Auslesungen
  speichern weiterhin vollständig (wenige, für Fehlersuche wertvoll).

### 4.6 Zeitstempel

- Massgeblich ist die **Zeit der Antwort** (Systemzeit), in UTC gespeichert.
- Zusätzlich wird die **Uhrzeit des Zählers** (falls im Telegramm) gespeichert. Weicht sie mehr als
  15 Minuten ab, ist das eine Auffälligkeit („Uhr des Zählers falsch“) – wichtig bei
  Stichtagswerten.
- Ohne gültige Systemzeit (Pi ohne Netz und ohne Echtzeituhr startet z.B. im Jahr 1970) liest
  mbusio **nicht zyklisch**, sondern meldet „Systemzeit ungültig“. Lieber keine Werte als Werte mit
  falscher Zeit (Grundsatz 2).

## 5. Phase 1: Zyklisches Auslesen – Umfang

**Einstellungen:** je Linie ein Standardplan (Aus · Monatlich · Täglich · Stündlich · 15 min, mit
Uhrzeit bzw. Tag); je Zähler „wie Linie“ oder eigener Plan. Neue Linien: Plan „Aus“, damit nichts
ungewollt startet.

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
7. mbusio über Nacht aus (über den Termin 00:05): nach dem Start wird jeder Zähler einmal
   nachgelesen, mit echtem Zeitstempel.
8. Uhr springt (Zeit von Hand verstellen): Protokolleintrag, Termine richten sich neu aus,
   keine doppelten Zeitstempel.

## 6. Phase 2: Datenpunkte (was weitergegeben wird)

Weitergegeben werden nicht „Zähler“, sondern **Datenpunkte**: ein bestimmter Wert eines bestimmten
Zählers, z.B. „Whg. 3 Wärme – Energie aktuell (kWh)“.

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

## 10. Offene Fragen (vor Phase 1 zu klären)

1. **Standardplan:** täglich um 00:05 für alle neuen Linien, oder lieber „Aus“ bis man ihn
   bewusst einschaltet? (Vorschlag: neue Linien „Aus“, beim Einschalten „täglich 00:05“ vorgeschlagen)
2. **Wie viele Linien und Zähler** maximal pro Pi? (Planung: bis 10 Linien, bis 250 Zähler)
3. **Teilen sich mbusio und ein Leitsystem ein Gateway?** Wenn ja, welches Gateway-Modell
   (mehrere TCP-Clients möglich?)
4. **Hat der Pi Internet bzw. einen Zeitserver**, oder braucht es eine Echtzeituhr (RTC-Modul)?
5. **Aufbewahrung:** Bei häufigen Plänen 90 Tage alle Werte, danach Tageswerte – passt das?
6. Sollen **manuelle Auslesungen** während eines geplanten Laufs erlaubt sein (Vorschlag: ja,
   mit Vorrang)?
7. Kennst du **Herstellergrenzen** für Auslesungen bei deinen Zählern? Damit liessen sich die
   Hinweise genauer machen (z.B. je Hersteller eine Empfehlung).

## 11. Phasenplan

| Phase | Inhalt | Voraussetzung |
| --- | --- | --- |
| 1 | Linien-Dienst, Ausleseplan je Zähler, kompakte Speicherung, Aufbewahrung | Abstimmung dieses Konzepts |
| 2 | Datenpunkte, Qualität, Plausibilität, Zählertausch | Phase 1 im Dauertest bestanden |
| 3 | MQTT | Phase 2 |
| 4 | Modbus TCP | Phase 2, Bedarf bestätigt |
| 5 | BACnet/IP | Bedarf bestätigt, Bibliothek geprüft |
