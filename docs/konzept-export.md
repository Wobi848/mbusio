# Konzept: Weitergabe-Protokoll und Export für die Abrechnung

Stand: 26.10.2026 · Status: **Schritt 1 umgesetzt** (Weitergabe-Protokoll); Vorgaben aus Abschnitt 6
entschieden: 30 Tage fest, eine Zeile je Zähler und Wert, Ablage zum Herunterladen bis zur E-Mail

Zwei Wünsche aus dem Betrieb:

1. **Sehen, was rausgegangen ist und was nicht:** je Wert, wann er an welchen Ausgang ging, was
   verworfen wurde und warum, mit Filtern.
2. **Werte für die Abrechnung in Excel:** die Zählerstände zum Stichtag je Zähler, einstellbar,
   von Hand und automatisch, später auch per E-Mail.

Verwandt: [konzept-ausgaenge.md](konzept-ausgaenge.md) (Ausgänge, Warteschlange),
[konzept-dauerbetrieb.md](konzept-dauerbetrieb.md) (Plan, Speicherung, Ausdünnen).

**Grundsatz (26.10.2026):** mbusio funktioniert allein, ohne andere Software (kein
Datenbankserver, kein Home Assistant, keine InfluxDB). Die Werte bleiben in SQLite im Datenordner;
Langzeitspeicher, Export und Sicherung gehören deshalb in mbusio selbst. MQTT und Modbus sind
Angebote an andere Systeme, keine Voraussetzung.

**Langzeitspeicher** (gemessen: ~340 Byte je Auslesung eines Zählers samt Index, ~70 Byte je Wert
eines Datenpunkts; 100 Zähler stündlich ≈ 360 MB im Jahr):

1. **Vorschau** im Zustand der Anlage: wie lange der Speicher beim heutigen Plan reicht.
   *Umgesetzt (26.10.2026):* `Db::growth_per_day` aus Plan, Datenpunkten und Ausgängen der Linien
   im Dauerbetrieb, `DiskState::with_growth` im Status (frisch bei jeder Abfrage, der Platz wird
   jede Minute gemessen); unter 90 Tagen als Hinweis.
2. **Jahresarchive** `archiv-JJJJ.db` (gleiches Format) für grosse Anlagen: ältere Auslesungen
   wandern dorthin, Verlauf und Export lesen über alle; erst bauen, wenn eine Anlage es braucht.
3. **Sicherung vom Gerät weg** über den automatischen Export (Teil B, später E-Mail).

---

## 1. Heutiger Stand

**Gespeichert wird rückwirkend schon viel:**

- **Jede Auslesung vollständig** (`readings`): alle Werte des Zählers, geplante als Rohtelegramm.
  Nichts wird von selbst gelöscht; je Projekt optional „nach 30/90/365 Tagen 1 Wert pro Tag“.
- **Werte der gewählten Datenpunkte** (`dp_values`), aber erst ab der Wahl.
- **Stichtagswerte im Zähler:** viele Wärmezähler liefern „Energie · Speicher 1“ mit dem Datum des
  Stichtags (Datensätze mit `storage` > 0, das Datum im Datensatz desselben Speichers). Sie kommen
  mit jeder Auslesung, stehen also auch in `readings`.

Folge: ein Export kann **auch Werte liefern, die damals niemand als Datenpunkt gewählt hatte** – er
liest sie aus den gespeicherten Auslesungen.

**Was fehlt:**

- **Kein Protokoll der Weitergabe.** MQTT löscht eine Nachricht, sobald der Broker sie bestätigt;
  übrig bleiben die Zähler „gesendet / wartet“ und eine Meldung, wenn Nachrichten verworfen werden.
  Modbus holt ab; wer wann abgefragt hat, steht nirgends.
- **Export nur von Hand und nur roh:** CSV der Zählerliste, der Datenpunkte, des Verlaufs je
  Zähler, Projekt als JSON, Bericht als PDF. `/api/export` (alle Auslesungen einer Linie) hat in
  der neuen Oberfläche keinen Knopf. Keine Abrechnungstabelle, nichts automatisch, keine E-Mail.

---

## 2. Teil A: Weitergabe-Protokoll

### 2.1 Was festgehalten wird

Neue Tabelle `out_log`, eine Zeile je Ereignis:

| Feld | Inhalt |
|---|---|
| `utc`, `time` | wann (UTC und lokale Zeit wie bei den Auslesungen) |
| `output` | `mqtt`, `modbus` … |
| `dp` | Datenpunkt (leer bei Status- und Home-Assistant-Nachrichten) |
| `address` | Topic bzw. Register |
| `value`, `quality` | was weitergegeben wurde |
| `result` | `sent` · `dropped` |
| `reason` | bei `dropped`: Broker zu lange nicht erreichbar, Warteschlange voll, Ausgang ausgeschaltet |

- **MQTT:** `sent`, sobald der Broker bestätigt (heute die Stelle, an der die Nachricht aus der
  Warteschlange gelöscht wird); `dropped` beim Begrenzen der Warteschlange. Was gerade **wartet**,
  steht in der Warteschlange selbst und wird in der Ansicht dazugezeigt, nicht doppelt gespeichert.
  Dazu bekommt `mqtt_queue` die Spalte `dp`, damit auch wartende Nachrichten ihrem Datenpunkt
  zugeordnet sind.
- **Modbus** wird abgeholt, nicht gesendet. Festgehalten wird je Leitsystem (IP) die letzte Abfrage
  und die Anzahl Abfragen heute, angezeigt an der Zeile des Ausgangs. Jede Abfrage einzeln zu
  speichern wäre bei Abfragen im Sekundentakt zu viel für einen Raspberry Pi.
- **Nicht** festgehalten: dass ein Wert wegen „nur bei Änderung“ nicht gesendet wurde. Das ist
  gewollt und würde das Protokoll füllen. Ob eine Linie überhaupt weitergibt (Dauerbetrieb), zeigt
  die Weitergabe schon heute am Datenpunkt.

### 2.2 Ansicht

Weitergabe → Knopf **Protokoll** neben den Ausgängen. Es öffnet sich eine Liste über die ganze
Breite (wie die Datenpunkte), neueste oben:

- **Filter:** Ausgang · Datenpunkt (Suche wie überall mit `/`) · Zeitraum (heute, 7 Tage, 30 Tage,
  von–bis) · Ergebnis (alle, gesendet, verworfen, wartet). Anzahl je Filter wie im Scan.
- **Zeile:** Zeit, Ausgang, Datenpunkt, Topic/Register, Wert mit Einheit, Qualität, Ergebnis als
  Wort und Farbe (gesendet grün, wartet grau, verworfen rot mit Grund im Tooltip).
- Vom Datenpunkt aus: in den Details „Protokoll dieses Datenpunkts →“ öffnet die Ansicht
  vorgefiltert.
- **CSV** der gefilterten Zeilen.

### 2.3 Speicher

Vorgabe: **30 Tage**, je Projekt nicht einstellbar (das Protokoll gehört dem Gerät, nicht dem
Projekt); dazu höchstens 200 000 Zeilen, ältere zuerst weg. Aufgeräumt wird mit dem Ausdünnen
(höchstens einmal pro Stunde). Bei vollem Speicher (`readings_paused`) wird auch nicht mehr
protokolliert.

---

## 3. Teil B: Export für die Abrechnung

### 3.1 Einstellungen mit Vorgaben

Je Projekt gespeichert („Export-Vorlage“), damit der automatische Export jedes Mal dasselbe liefert.
Die Vorgaben sind so gewählt, dass die meisten nichts einstellen müssen:

| Einstellung | Vorgabe | Wählbar |
|---|---|---|
| Form | **Abrechnung:** eine Zeile je Zähler | Zeitreihe: eine Zeile je Tag oder Monat, eine Spalte je Wert |
| Werte | **Hauptwert je Zähler** (Wärme: Energie, Wasser: Volumen, Strom: Energie, wie „Hauptwert“ beim Wählen der Datenpunkte) | weitere Werte je Zähler oder je Medium für alle |
| Zähler | alle des Projekts | einzelne Linien |
| Zeitraum | **Vormonat** | Monat, Jahr mit Stichtag (z.B. 30.09.), von–bis |
| Stand am Stichtag | **automatisch** (siehe 3.2) | nur Auslesungen von mbusio, nur Stichtagswerte des Zählers |
| Format | **CSV für Excel:** Semikolon, Dezimalkomma auf Deutsch, UTF-8 mit BOM (wie die bestehenden CSV) | – |
| Automatisch | **aus** | monatlich am 1. bzw. jährlich nach dem Stichtag |
| Ziel | **Ordner** `exporte/<projekt>/` im Datenordner, dazu Liste zum Herunterladen | später E-Mail (Teil C) |

### 3.2 Stand am Stichtag: woher

„Stand am 30.09.“ heisst: Zählerstand am Ende dieses Tages. Reihenfolge bei **automatisch**:

1. **Stichtagswert des Zählers**, wenn sein Datum genau der Stichtag ist (Speicher mit Datum aus
   einer späteren Auslesung). Er ist exakt, auch wenn mbusio an dem Tag nicht gelesen hat.
2. sonst die **letzte Auslesung von mbusio am oder vor dem Stichtag**.
3. sonst leer.

Neben jedem Stand steht, **woher** er kommt und **wann** gelesen: `Zähler (Stichtag)`,
`Auslesung 30.09. 23:45` oder `fehlt`. Ist die Uhrzeit einer Auslesung unsicher (ohne Zeitquelle),
steht `Zeit unsicher` dabei. Kein Wert wird still geraten.

### 3.3 Tabelle „Abrechnung“

Eine Zeile je Zähler und gewähltem Wert:

`Projekt; Linie; Name; Ort; ID; PA; Medium; Wert; Einheit; Stand Anfang; Quelle Anfang; Stand Ende; Quelle Ende; Verbrauch; Hinweis`

- **Verbrauch** = Ende − Anfang. Bei einem **Zählertausch** im Zeitraum rechnet er über den Tausch
  hinweg (Endstand alt + Verbrauch neu, wie bei den Datenpunkten); der Hinweis nennt den Tausch.
- **Faktor und Verschiebung** eines Datenpunkts gelten auch hier (z.B. Wh → kWh).
- **Hinweis** bei: Zählerstand gesunken, Wert fehlt, Zeit unsicher, Zähler gestört im Zeitraum.

### 3.4 Wo in der Oberfläche

Anlage → Projekt → **Abrechnung exportieren**: Feld im Detailbereich mit den Einstellungen aus 3.1,
**Vorschau** der ersten Zeilen (mit den Hinweisen), **Herunterladen**, Schalter **Automatisch** und
die Liste der letzten automatischen Exporte zum Herunterladen. Der heutige Knopf „Exportieren“
(Projekt als JSON zum Mitnehmen) wird zu „Projekt sichern“, damit beides nicht verwechselt wird.

### 3.5 Automatisch

Der Zeitplan erzeugt die Datei nach der ersten geplanten Auslesung des neuen Zeitraums (also z.B. am
1. nach 00:05), damit der Stand zum Monatsende vollständig ist. Fehlt sie (Gerät war aus), holt er
sie beim nächsten Start nach, wie verpasste Termine. Eine Meldung im Protokoll nennt die Datei.

---

### 3.6 Zählertausch erkennen, defekte Zähler melden

Eine Abrechnung ist nur so gut wie die Zuordnung der Stände. Zwei Lücken im heutigen Stand:

- **Tausch nur, wenn man ihn meldet.** Beim Auslesen über die Primäradresse prüft mbusio nicht, ob
  ein anderer Zähler antwortet; eine neue ID überschreibt die alte still. Der Export rechnet dann
  „neuer Stand − alter Stand“ (meist negativ), Datenpunkte springen und gelten als „gesunken“.
  Bei Sekundäradressierung kann das nicht passieren (die ID ist die Adresse).
- **Defekte Zähler** erkennt mbusio (antwortet nicht, gestört, Fehlerwert, Fehler im Statusbyte,
  gesunken), der Export zeigt davon aber nur „fehlt“, „Zeit unsicher“ und „gesunken“.

**Tausch erkennen.** Beim Speichern einer Auslesung (`store_reading`), wenn alles zutrifft:

1. der Zähler steht in der Liste und hat eine ID,
2. die Auslesung ist sauber (kein Fehler, keine Kollision) und hat eine **andere** ID,
3. mbusio hat den **alten Zähler schon selbst ausgelesen** (die letzte gespeicherte Auslesung hat
   die alte ID). Ohne das ist es kein Tausch, sondern eine Zählerliste, die nicht stimmt (z.B. vom
   Kunden mit falscher Zuordnung importiert): dann wie heute die neue ID übernehmen, aber mit
   Meldung „ID weicht von der Liste ab: … statt …“ im Protokoll.

Dann geschieht dasselbe wie beim gemeldeten Tausch („Zähler getauscht …“): Tausch eintragen
(`exchanges`), Zählerstände der Datenpunkte laufen weiter (Verschiebung), Eintrag im
Änderungsverlauf des Berichts, Auslesung beim Zähler gespeichert. Unterschiede:

- `exchanges.confirmed = 0`: **erkannt, nicht bestätigt**. Die Zählerliste zeigt beim Zähler einen
  orangen Hinweis „Tausch erkannt“; in den Details „alte ID → neue ID, am …“ und **Bestätigen**.
  Bestätigen setzt `confirmed = 1` (der gemeldete Tausch ist von Anfang an bestätigt).
- Meldung im Protokoll und in der Bus-Konsole: „Zähler …: Tausch erkannt, alte ID … → neue ID …“.
- Der Export schreibt „Zählertausch erkannt (nicht bestätigt)“ in den Hinweis, bis bestätigt ist.

Technisch: Der Kern des Tauschs wird von `exchange_meter` getrennt (`exchange_core`, mit
`detected`), weil geplante Auslesungen schon in einer Transaktion speichern; der erkannte Tausch
läuft in einem Savepoint (verschachtelbar), der gemeldete wie bisher in einer eigenen Transaktion.

**Defekte Zähler melden.** Je Auslesung wird festgehalten, ob der Zähler selbst einen Fehler
meldet (`meters.fault`, wie `clock_off` beim Zähler gespeichert):

| Schlüssel | woran erkannt | Wort |
|---|---|---|
| `status_error` | Statusbyte 0x08 (dauerhaft) oder 0x10 (vorübergehend) | meldet Fehler |
| `error_flags` | Fehlerflags (VIF FD 17) ungleich 0 | meldet Fehler (Fehlerflags) |
| `power_low` | Statusbyte 0x04 | Batterie schwach |

Die Zählerliste zeigt das als Hinweis am Zähler. Der Export schreibt je Zähler in den Hinweis:

- Zählertausch (bestätigt) bzw. Zählertausch erkannt (nicht bestätigt), mit Datum,
- im Zeitraum nicht gelesen,
- antwortet zurzeit nicht, zurzeit gestört (3 Termine nicht gelesen),
- meldet Fehler / Batterie schwach in der Auslesung, aus der ein Stand stammt,
- wie bisher: Stand fehlt, Zeit unsicher, Zählerstand gesunken.

Über der Vorschau steht die Zusammenfassung je Art („2 Tausch erkannt · 1 antwortet nicht …“), in
der CSV stehen die Hinweise je Zeile.

**Grenze:** Bekommt ein neuer Zähler dieselbe ID wie der alte (praktisch nie), bleibt nur der
gesunkene Stand als Hinweis.

## 4. Teil C: E-Mail (später)

Einstellungen wie bei MQTT: Server, Port, TLS, Benutzer, Passwort, Absender, Empfänger je Projekt.
Das Passwort bleibt wie beim MQTT-Passwort nur in der Datenbank auf dem Gerät und geht nie an den
Browser (die Oberfläche zeigt nur „gespeichert“). Gesendet wird die Datei aus 3.5 als Anhang;
klappt es nicht, bleibt sie im Ordner und das Protokoll sagt warum. Eigener Schritt, weil ein
SMTP-Client dazukommt und es ohne echten Mailserver schwer zu testen ist.

---

## 5. Umsetzung in Schritten

1. **Weitergabe-Protokoll** (Teil A): Tabelle, Einträge bei MQTT, Modbus „zuletzt abgefragt“,
   Ansicht mit Filtern und CSV, Aufräumen. Tests: gesendet, verworfen, wartet, Filter, Grenze.
2. **Export von Hand** (3.1–3.4): Vorlage je Projekt, Stand am Stichtag, Abrechnung und
   Zeitreihe, Vorschau, Herunterladen. Tests mit dem Simulator: Stichtagswert vor Auslesung,
   fehlender Wert, Zählertausch im Zeitraum, Zeit unsicher.
3. **Automatisch** (3.5) mit Nachholen.
4. **E-Mail** (Teil C).

Schritt 1 und 2 sind unabhängig; Schritt 1 ist kleiner.

## 6. Entschieden (26.10.2026)

- Protokoll **30 Tage fest** (dazu höchstens 200 000 Zeilen).
- Abrechnung: **eine Zeile je Zähler und Wert**, lässt sich in Excel leichter filtern.
- Ablage `exporte/` im Datenordner, erreichbar über **Herunterladen** in der Oberfläche, bis die
  E-Mail kommt.
- Vorgaben wie in 3.1 (einstellbar, Wahl je Projekt gespeichert).

## 7. Stand

**Schritt 1 umgesetzt (26.10.2026):** Tabelle `out_log` (`datapoint/outputs/log.rs`),
`mqtt_queue.dp`; „gesendet“ beim Bestätigen durch den Broker, „verworfen“ mit Grund beim Begrenzen
der Warteschlange, „wartet“ aus der Warteschlange; Modbus je Leitsystem letzte Abfrage im Status
(`status.modbus.peers`) und an der Zeile des Ausgangs; aufgeräumt mit dem Ausdünnen; `GET
/api/out-log` mit Filtern und CSV; Ansicht Weitergabe → Protokoll (`weitergabe-protokoll.js`), vom
Datenpunkt aus vorgefiltert. Tests: Rust `out_log_shows_waiting_sent_and_dropped`, Browser
`17-protokoll`, Modbus und MQTT ergänzt.

**Schritt 2 umgesetzt (26.10.2026):** `src/billing/` (mod: Einstellungen je Projekt in
`meta`, Zeitraum, Werte zur Auswahl; stand: Stand am Ende eines Tages und Zählertausch; table:
Abrechnung und Zeitreihe als Tabelle, daraus CSV und Vorschau), Index `readings(meter, timestamp)`,
`GET/PUT /api/projects/{id}/billing`, `…/billing/csv`, Anlage → Projekt → „Abrechnung exportieren“
(`anlage-abrechnung.js`), „Exportieren“ heisst „Projekt sichern“. Abweichend von 3.2: fehlt eine
Auslesung vor dem Anfang, gilt die erste im Zeitraum (Quelle „erste Auslesung danach“), damit
neu aufgenommene Zähler nicht leer bleiben. Dezimalzeichen einstellbar, weil Excel in Deutschland
ein Komma, in der Schweiz einen Punkt erwartet. Zählertausch: alt und neu über die ID im Telegramm,
weil der neue Zähler vor dem Eintrag des Tauschs gelesen wird. Noch nicht: einzelne Linien wählen,
Hinweis „gestört im Zeitraum“. Tests: Rust `billing::tests` (5), Browser `18-abrechnung`.

**Schritt 3 umgesetzt (26.10.2026), 3.6:** `swap_check` in `store_reading` (nur Primäradresse,
alter Zähler schon gelesen), `exchange_core` mit `detected` im Savepoint (`Db::savepoint`),
`exchanges.confirmed`, `open_swaps`/`confirm_swap`, `POST /api/exchanges/{id}/confirm`, `swap` in
der Zählerliste; `meters.fault` aus der letzten Auslesung (`fault_of`), zählt bei „Fehler“ ausser
Batterie schwach. Export: Hinweisarten je Zeile, `summary` in der Tabelle, Fehler auch bei der
Quelle (Zeitreihe). Tests: Rust `swap_is_detected_…`, `list_id_mismatch_is_no_swap`,
`meter_fault_is_kept`, `notes_for_detected_swap_not_read_and_fault`, Browser `19-tausch`.

**Schritt 4 umgesetzt (26.10.2026), 3.5:** `Settings.auto`, `billing/auto.rs` (fällige
Zeiträume aus `meta` `billing_done:<projekt>`, Datei schreiben, Liste), Zeitplan alle 15 Minuten
(`export_billing` in `scheduler/upkeep.rs`), Speichern mit `auto` prüft beim nächsten Durchlauf.
Abweichend: Ordner `exporte/<projekt-nr>/` statt Name (Umbenennen trennt die Dateien nicht), der
Name steht in der Datei. Der letzte Zeitraum wartet auf eine Auslesung danach, höchstens 2 Tage
(Zähler ohne Plan blockieren nicht). Nachholen höchstens 12 Zeiträume; wieder eingeschaltet nur
der letzte. Bei eigenem Zeitraum ruht der Schalter, statt das Speichern abzulehnen. Dateien werden
nicht aufgeräumt (wenige KB je Monat). Tests: Rust `auto_export_*` (2), Browser `18-abrechnung`.

**Nächster Schritt:** E-Mail (Teil C).
