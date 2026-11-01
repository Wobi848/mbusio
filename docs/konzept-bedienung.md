# Konzept: Bedienung und Aufbau der Oberfläche

Stand: 18.10.2026 · Status: **umgesetzt** (Schritte 1–6)

mbusio ist in kurzer Zeit stark gewachsen: Inbetriebnahme, Dauerbetrieb, Datenpunkte, MQTT,
Simulator. Die Funktionen wurden jeweils dort angebaut, wo es gerade passte. Ergebnis: Manche
Abläufe sind auf mehrere Stellen verteilt, und im Code steht vieles mehrfach. Dieses Konzept legt
fest, **welcher Ablauf wohin gehört** und **aus welchen Bausteinen** die Oberfläche gebaut wird.
Neue Funktionen (z.B. Modbus) werden zuerst hier eingeordnet, dann gebaut.

---

## 1. Wer macht was?

| Rolle | Situation | Will |
| --- | --- | --- |
| **Techniker vor Ort** | Inbetriebnahme, Störung beheben; Laptop oder Handy, wenig Zeit | Zähler finden, Fehler beheben, benennen, Bericht für den Kunden |
| **Integrator** | Anbindung ans Leitsystem, oft im Büro | festlegen, welche Werte wohin gehen; Liste für das Leitsystem |
| **Betreiber / Service** | Anlage läuft, ab und zu nachsehen | sehen, ob alles läuft; was gestört ist; Verläufe |

Daraus folgen **vier Abläufe**, und jeder bekommt **genau eine Stelle** in der Oberfläche:

1. **Anlage aufbauen:** Projekt, Gateways, Linien, Verbindung.
2. **Zähler finden und in Ordnung bringen:** Scan, Auffälligkeiten, beheben.
3. **Zähler führen:** benennen, Ort, auslesen, Verlauf, Plan, Zählertausch.
4. **Weitergeben:** was (Datenpunkte), wohin (MQTT, später Modbus), wie oft; Zustand der Ausgänge.

Dazu zwei Nebenstellen: **Werkzeuge** für Experten (Rohtelegramme, Adresse/Baudrate von Hand,
Log) und **Simulator** zum Üben.

## 2. Heute: wo liegt was, was ist das Problem?

| Ablauf | Heute verteilt auf | Problem |
| --- | --- | --- |
| Weitergeben | Werkzeuge → Auslesen (Spalte „Senden“); Zähler → Knopf „Datenpunkte“ (Auswahlfenster); Zähler → Bereich Datenpunkte; Zähler → Bereich MQTT | **vier Stellen**, drei davon im Reiter „Zähler“, der eigentlich die Zählerliste ist; man findet es nicht |
| Dauerbetrieb (Plan, Zustand) | Projekt-Karte (Zeile je Linie), Zählerliste (Spalte Plan), Leiste unten (Uhrzeit, Speicher, MQTT), Formulare (Plan) | Zustand ist verstreut; es gibt keinen Ort, an dem man sieht „läuft alles?“ |
| Zähler führen | Zählerliste, Benennen-Modus, Liste importieren, Zähler-Formular, Verlauf | im Grundsatz gut; aber zu viele Knöpfe in einer Reihe |
| Messwerte exportieren | eigener Bereich im Reiter Zähler | passt, aber unauffällig |

## 3. Ziel: Reiter und Inhalte

```
1 Projekt     Anlage (Gateways, Linien), Projektdaten, Bericht, Export/Import
              + oben: Zustand der Anlage (läuft alles?)
2 Scan        Verbindung, Scan, Auffälligkeiten, „Alle beheben“
3 Zähler      Zählerliste: benennen, Ort, Liste importieren, auslesen, Verlauf, Plan je Zähler,
              Zählertausch, Messwerte exportieren
4 Weitergabe  NEU: Datenpunkte (was), Ausgänge MQTT / später Modbus (wohin), Vorlagen, CSV
──────────
Werkzeuge     einzelnen Zähler roh auslesen, Adresse/Baudrate, Reset, Telegramme, Log
Simulator     simulierte Anlagen
```

### 3.1 Projekt: Zustand der Anlage

Oben im Reiter eine kompakte Zeile, nur wenn ein Plan oder ein Ausgang aktiv ist:

`Plan: 3 Linien · nächster Termin 00:05 · gestört 2 Zähler │ Uhrzeit: bestätigt (NTP) │ Speicher: 82 % frei │ MQTT: verbunden, 0 wartend`

Jeder Teil ist ein Link zur Stelle, wo man es behebt. Die Leiste unten zeigt weiterhin nur
Dringendes (Scan läuft, Uhrzeit nicht bestätigt, MQTT getrennt, Speicher voll).

### 3.2 Weitergabe (neuer Reiter)

Ein Reiter, drei Teile von oben nach unten, in der Reihenfolge, in der man es macht:

1. **Ausgänge** (wohin): je Ausgang eine Zeile mit Zustand, z.B.
   `MQTT · verbunden mit 192.168.1.10 · 1 234 gesendet · 0 wartend [Einstellungen]`.
   Einstellungen klappen auf. Später kommt `Modbus TCP · Port 502 · …` als zweite Zeile dazu.
2. **Datenpunkte** (was), **nach Zählern gruppiert**:
   - Je Zähler eine Kopfzeile (Name, Ort, ID, letzte Auslesung). Aufklappen zeigt **alle Werte
     des Zählers** mit aktuellem Wert und Häkchen „weitergeben“: das ist die heutige Spalte
     „Senden“ und das heutige Auswahlfenster in einem.
   - Gewählte Werte sind Datenpunkte. Je Datenpunkt eine Spalte **pro Ausgang** (MQTT ✓ mit
     Topic, später Modbus mit Register), dazu Name, Einheit, Faktor, Weitergabe (jede Auslesung /
     bei Änderung / täglich), Qualität.
   - Filter: Linie (Standard: aktive Linie, umschaltbar auf ganzes Projekt), „nur gewählte“.
   - Massenbearbeitung wie heute (markieren → Ausgang an/aus, Weitergabe, löschen).
3. **Vorlagen und Listen:** „Auswahl für alle Zähler gleicher Art übernehmen“, CSV exportieren
   und importieren (für Excel und als Liste für das Leitsystem).

**Aus anderen Reitern führt nur ein Link hierher**, keine zweite Bedienung:
- Zählerliste: Knopf „Weitergabe“ je Zähler → öffnet den Reiter Weitergabe mit diesem Zähler
  aufgeklappt.
- Werkzeuge → Auslesen: ein Satz „Werte dieses Zählers weitergeben → Weitergabe“.

### 3.3 Zähler

Die Zählerliste bleibt der Kern. Aufräumen:
- Knopfreihe gruppieren: **Liste** (importieren, exportieren, leeren) · **Benennen** ·
  **Auslesen** (alle) · **Bericht**. Seltenes in ein Menü „Mehr“.
- Je Zähler: Auslesen · Verlauf · Weitergabe (Link) · Bearbeiten. Zählertausch bleibt im
  Bearbeiten-Formular.
- Spalte Plan bleibt (zeigt nur, Einstellen im Formular).

### 3.4 Werkzeuge

Nur noch Expertenwerkzeuge. Die Auslesung zeigt alle Werte (wie heute), ohne Spalte „Senden“,
mit dem Link zur Weitergabe.

## 4. Bausteine (damit nichts mehrfach geschrieben wird)

### 4.1 Oberfläche

| Baustein | Ersetzt heute | Verwendet in |
| --- | --- | --- |
| **Tabelle** (Spalten, Zeilen, Sortieren, Markieren, Leer-Hinweis) | 8 von Hand gebaute Tabellen | Treffer, Zählerliste, Datenpunkte, Simulator, Vorschauen |
| **Vorschau → Übernehmen** (prüfen, Änderungen/Fehler/Warnungen zeigen, übernehmen) | 3 eigene Abläufe | Datenpunkt-CSV, Zählerliste importieren, Namen erzeugen, später Modbus-Register |
| **Aufklapp-Bereich** (Titel, Inhalt, Schliessen) | 5 eigene „Panels“ | Verlauf, Liste importieren, Namen, Vorschauen, Ausgang-Einstellungen |
| **Formular aus Felddefinition** (Felder, Prüfung, Werte lesen) | 4 Formulare + Planfeld + Uhrzeitfeld | Projekt, Linie, Zähler, Datenpunkt, MQTT, Simulator |
| **Wertauswahl eines Zählers** (Werte mit Häkchen) | Spalte „Senden“ + Auswahlfenster | Weitergabe |
| **Status-Chip** (Art + Text + Erklärung) | 15 Stellen von Hand | überall |

Jeder Reiter ist eine Datei (`web/js/<reiter>.js`), Bausteine in `web/js/ui.js`. Ein Reiter lädt
seine Daten selbst, wenn er geöffnet wird.

### 4.2 Server

| Baustein | Ersetzt heute |
| --- | --- |
| **Änderungssatz** `ChangeSet { added, changed, unchanged, warnings, errors, applied }` mit einer Funktion „prüfen oder übernehmen“ | drei eigene Ergebnis-Typen (Datenpunkt-CSV, Zählerliste, Namen) |
| **Ausgänge allgemein:** Tabelle `dp_outputs (datenpunkt, ausgang, adresse, ein)` und eine Schnittstelle je Ausgang (`vormerken`, `zustand`, `einstellungen`) | MQTT-Spalten direkt in der Datenpunkt-Tabelle; für Modbus müsste man das wiederholen |
| **Bezeichnungen** „Gateway › Linie“, „Zähler-Name“ an einer Stelle | mehrfach im SQL und im Code |

Der Umbau der Ausgänge kommt **vor** Modbus, damit Modbus nur noch ein zweiter Ausgang ist.

## 5. Vorgehen

Jeder Schritt einzeln, mit allen Rust- und Browser-Tests, kein Funktionsverlust:

1. **Server:** `ChangeSet`, allgemeine Ausgänge (`dp_outputs`, MQTT als erster Ausgang),
   Migration der bestehenden MQTT-Zuordnung. Zuerst, weil die neue Oberfläche auf dieser
   Datenform aufbaut (sonst müsste sie zweimal angepasst werden).
2. **Bausteine** `ui.js` (Tabelle, Vorschau/Übernehmen, Aufklapp-Bereich, Formular, Chip).
3. **Reiter Weitergabe** mit den Bausteinen: Datenpunkte und MQTT dorthin, Wertauswahl je
   Zähler, Links aus Zählerliste und Werkzeugen; doppelte Stellen entfernen; die übrigen Stellen
   auf die Bausteine umstellen.
4. **Projekt: Zustand der Anlage** oben im Reiter.
5. **Zählerliste:** Knopfreihe gruppieren.
6. Browser-Tests an die neue Aufteilung anpassen und je Reiter vervollständigen.

Danach ist Modbus (Phase 4) ein zweiter Ausgang im Reiter Weitergabe: eine Spalte mehr und eine
Zeile bei den Ausgängen.

## 6. Umsetzung (18.10.2026)

- Server: `ChangeSet` (src/changeset.rs) für Datenpunkt-CSV, Zählerliste importieren und Namen;
  Ausgänge allgemein (`dp_outputs`, src/datapoint/outputs.rs), MQTT als erster Ausgang, alte
  MQTT-Spalten werden beim Start übernommen und entfernt; ältere Projekt-Exporte werden verstanden.
- Oberfläche: Bausteine in `web/js/ui.js`; Reiter Weitergabe (`web/js/transfer.js`) ersetzt
  `datapoints.js`, `mqtt.js`, das Auswahlfenster und die Spalte „Senden“; alle Formulare und
  Status-Chips über die Bausteine; Zustand der Anlage im Projekt; Zählerliste mit Menü „Liste ▾“.
- Ausgänge werden über `match`-Zweige je Ausgang unterschieden (statt eines Trait-Objekts): ein
  neuer Ausgang ist ein Eintrag in `OUTPUTS` und je ein Zweig für Name, Adresse und Senden.

## 7. Entscheidungen (18.10.2026)

1. Spalte „Senden“ in der Auslesung fällt weg; dort nur ein Link zur Weitergabe (bzw. „In
   Zählerliste aufnehmen“, falls der Zähler noch nicht in der Liste steht).
2. Reiter Weitergabe zeigt standardmässig die aktive Linie, umschaltbar auf das ganze Projekt.
3. Reiter: 1 Projekt, 2 Scan, 3 Zähler, 4 Weitergabe; Werkzeuge und Simulator ohne Nummer.

## 8. Nächster Schritt (18.10.2026)

Neue wie erfahrene Benutzer sehen sofort, was zu drücken ist:

- Oben in den Reitern 1–4 eine Karte **„Nächster Schritt“** mit genau einem Knopf, abgeleitet aus dem
  Zustand der aktiven Linie: Anschluss festlegen → verbinden → Primärscan → alle merken → alle
  auslesen → Werte für die Weitergabe wählen. Der Knopf führt den Schritt direkt aus (oder öffnet
  die Stelle, an der man etwas eingeben muss). Ausblenden gilt bis zum nächsten Schritt.
- Die Reiter 1–4 zeigen den Fortschritt: ✓ erledigt, Punkt = als Nächstes.
- Der passende Knopf in der Seite hat einen umlaufenden Ring; Hauptknöpfe sind in der
  Akzentfarbe, gesperrte grau mit Grund im Tooltip („Zuerst mit dem Bus verbinden“).
- Code: `web/js/guide.js`, `web/css/guide.css`, Test `tests/ui/09-guide.test.mjs`.

## 9. Service und Dauerbetrieb ohne Modus-Schalter (umgesetzt, 18.10.2026)

### 9.1 Ausgangslage

mbusio wird auf zwei Arten eingesetzt:

| Einsatz | Wer | Ablauf | Braucht |
| --- | --- | --- | --- |
| **Service** | Techniker mit Laptop am Strang | verbinden → suchen → beheben → auslesen → Bericht, fertig | Reiter 1–3, Werkzeuge |
| **Dauerbetrieb** | Integrator, danach Betreiber | wie Service, dann Plan, Werte wählen, Ausgang; läuft danach allein | alles, vor allem Reiter 4 und den Zustand der Anlage |

Heute führt „Nächster Schritt“ jeden bis zur Weitergabe, und Reiter 4 steht mit Nummer und
Fortschrittspunkt da, auch wenn jemand nur einen Strang prüft. Für den Service ist das unnötig.

### 9.2 Entscheidung: kein Schalter, sondern abgeleitet

Ein Modus-Schalter würde jede Stelle der Oberfläche fragen lassen, in welchem Modus sie ist, jeden
Test verdoppeln und Benutzer suchen lassen („wo ist die Funktion hin?“). Stattdessen gilt:

**Ein Projekt ist im Dauerbetrieb, sobald dort etwas dafür eingerichtet ist:** ein Ausleseplan
(Projekt, Linie oder Zähler nicht „aus“), mindestens ein Datenpunkt oder ein eingeschalteter
Ausgang. Sonst ist es ein Service-Projekt. Das wird nicht gespeichert, sondern immer aus dem
Zustand abgeleitet; es gibt genau **eine Funktion** dafür (`inOperation()`), die alle Stellen
benutzen (Nächster Schritt, Reiter, Zustand der Anlage). Die Bedingung „Plan oder Ausgang“, die
der Zustand der Anlage heute schon hat, wird damit ersetzt.

### 9.3 Was sich ändert

**Nächster Schritt**

- Service-Projekt: Anschluss → verbinden → suchen → merken → auslesen → **„Linie in Ordnung“**.
  Die letzte Karte hat zwei Knöpfe: **„Bericht“** (Hauptknopf, öffnet den Bericht) und
  **„Dauerbetrieb einrichten“** (zweiter Knopf). Ausblenden wie bisher.
- „Dauerbetrieb einrichten“ oder ein Projekt im Dauerbetrieb: nach dem Auslesen weiter mit
  **Plan festlegen** (öffnet das Projektformular beim Plan) → **Werte wählen** (wie heute) →
  **Ausgang einrichten** (MQTT-Einstellungen im Reiter Weitergabe), dann keine Karte mehr.
- Wer „Dauerbetrieb einrichten“ drückt, ohne dass schon etwas eingerichtet ist, merkt sich das
  für dieses Projekt im Browser (wie „Ausblenden“), bis der erste Plan oder Datenpunkt da ist.

**Reiter**

- Service-Projekt: Reiter 1–3 mit Nummer und Fortschritt. **Weitergabe** steht ohne Nummer und
  ohne Fortschrittspunkt, wie Werkzeuge und Simulator, bleibt aber an ihrem Platz und jederzeit
  erreichbar.
- Dauerbetrieb: Weitergabe bekommt die Nummer 4 und nimmt am Fortschritt teil (wie heute).

**Zustand der Anlage** (Reiter Projekt): erscheint genau dann, wenn `inOperation()` gilt.

**Simulator:** der Reiter erscheint nur, solange die aktive Linie eine Simulation benutzt (oder er
gerade offen ist); sonst führt unter Werkzeuge ein Knopf „Simulator öffnen“ hin.

**Unverändert:** alle Funktionen, Formulare, Tabellen und der Server. Kein neuer Speicherort,
keine neue Einstellung.

### 9.4 Fehlersuche, während der Dauerbetrieb läuft

Braucht keinen eigenen Modus, das regelt der Linien-Dienst schon: Bedienung von Hand (Scan,
Auslesen, Adresse ändern) geht immer vor, geplante Auslesungen warten. Wer die Linie mit
„Trennen“ von Hand trennt, pausiert ihren Plan, bis wieder verbunden wird (Zustand der Anlage
zeigt „pausiert, von Hand getrennt“).

### 9.5 Umsetzung

1. `inOperation()` in `project.js` (neben `findLine`), Zustand der Anlage darauf umstellen.
2. `guide.js`: Abschluss „Linie in Ordnung“ mit zwei Knöpfen; Schritte Plan und Ausgang; Merker
   „Dauerbetrieb einrichten“ je Projekt.
3. Reiter Weitergabe: Nummer und Fortschritt nur im Dauerbetrieb (eine Klasse, gleiche Gestaltung
   wie Werkzeuge).
4. Test `09-guide` um beide Wege erweitern (Service endet beim Bericht; Dauerbetrieb bis zum
   Ausgang). Texte DE/EN, README, CHANGELOG.

### 9.6 Entschieden

1. Der Service-Weg endet mit dem Knopf **Bericht**: das ist, was der Techniker dem Kunden abgibt.
2. **Ausgang einrichten** ist ein eigener Schritt: ohne Ausgang gehen gewählte Werte nirgends hin.
   Weitergabe gilt erst als erledigt, wenn Werte gewählt **und** ein Ausgang eingeschaltet ist.

## 10. Nächster Schritt in der Leiste unten (18.10.2026)

**Problem:** Die Karte „Nächster Schritt“ stand oben im Reiter. Wer in einer langen Seite arbeitet
(Trefferliste, Zählerliste), musste immer wieder hochscrollen, um zu sehen, was zu tun ist. Unten
gab es schon eine Leiste, die immer sichtbar ist: zwei Orte für „was ist los / was ist zu tun“.

**Lösung:** eine Leiste unten für beides, die Karte oben fällt weg.

```
Ruhe:      ● Bereit · SIMULATOR:ok10 · 2400 Bd   [Chips]  │ 2/3 Gefundene Zähler übernehmen  [Alle merken →] ⌃ ×
Bus aktiv: ● Primärscan · Adresse 37 · 4 gefunden  ▓▓▓░░░  [Abbrechen]          (Schritt tritt zurück)
Fertig:    ● Bereit …                              │ ✓ Linie in Ordnung  [Dauerbetrieb einrichten] [Bericht →]
```

- Links der Zustand des Busses wie bisher (nicht schliessbar, eine Zeile), rechts der nächste
  Schritt mit genau einem Hauptknopf; gilt in allen Reitern.
- **⌃ aufklappen:** über der Zeile erscheinen Fortschritt (●●○) und der Erklärungstext. Der Browser
  merkt sich auf- oder zugeklappt. **×** blendet den Schritt aus, bis der nächste kommt.
- Während der Bus arbeitet oder ein Plan liest, ist der Schritt ausgeblendet.
- Handy: Zustand oben, Schritt darunter mit Knopf über die ganze Breite.
- Der Seitenabstand unten folgt der wirklichen Höhe der Leiste (aufgeklappt höher), damit nie
  etwas verdeckt ist.
- Code: Platz `#guide` in der Leiste (`index.html`), Inhalt aus `guide.js`, Gestaltung in
  `guide.css`; die Karte und ihr CSS entfallen.

## 11. Oberfläche v2: was gehört wozu (umgesetzt, 18.10.2026)

Umgesetzt in sechs Schritten:

1. **Zähler-Baustein** `ui.meter` überall gleich (Medium-Symbol und Farbe, Name oder „ohne Namen“,
   ID · PA · Hersteller): Trefferliste, Zählerliste, Weitergabe, Werkzeuge.
2. **Drei Ebenen:** Kopfzeile = Projekt (Auswahl, Anlage, Werkzeuge, Simulator, ⚙), Linien-Leiste =
   Linie (Wahl, Verbindung mit einem Knopf für den nächsten Schritt, Plan; Verbindungsformular
   darunter), Laschen Scan · Zähler · Weitergabe, die in die Karte übergehen.
3. **Zählerliste:** Leiste an der Tabelle (Suche, Filter, Hinzufügen, „Alle N auslesen“),
   Bearbeiten klappt in der Zeile auf; ebenso im Projektbaum.
4. **Scan:** „1 Suche einstellen“ und „2 Ergebnis“ mit Filter und Satz der Hauptaktionen;
   Auffälligkeiten als Streifen unter ihrer Zeile statt Sammelkasten.
5. **Werkzeuge:** Ziel zuerst, Einstellungen darunter (jetzt → neu), gesperrt ohne Ziel.
   **Weniger Rahmen:** nur die Karte hat einen Rahmen; innen Flächen und Trennlinien.
6. **Handy:** Treffer und Zähler als Karten, Leiste unten zweizeilig; Dunkelmodus geprüft.

**Abweichungen vom Entwurf:** „Nächster Schritt“ bleibt in der Leiste unten (Abschnitt 10), nicht als
Karte in der Inhaltskarte; Anschluss und Verbinden zeigt nur die Linien-Leiste. Bei gleicher PA oder
ID steht ein Streifen nach der Gruppe statt einer Klammer über die Zeilen.

### 11.1 Knöpfe in drei Stufen (18.10.2026)

| Stufe | Klasse | Aussehen | wofür |
| --- | --- | --- | --- |
| Primär | (keine) | Akzentfarbe gefüllt | der eine nächste Schritt eines Bereichs (Scan starten, Alle auslesen, N übernehmen) |
| Sekundär | `tonal` | helle Akzentfläche, Text und Rand in Akzentfarbe | wichtig, aber nicht der Hauptweg (Prüfen, N beheben, Sekundärscan starten, Zähler hinzufügen, Dauerbetrieb einrichten, Setzen/Umstellen) |
| Neutral | `ghost` | weiss mit Rand | Aktionen, die sich je Zeile wiederholen (Merken, Wählen, Werte wählen) |
| Leise | `quiet` | weiss mit Rahmen und Text in Akzentfarbe (seit 31.10.2026, §15) | Seltenes (Bericht, Liste, Schliessen, Auslesen je Zeile) |

### 11.2 Farben für Probleme (19.10.2026)

- **Orange = Hinweis:** der Zähler funktioniert und kommt beim Übernehmen mit (z.B. gleiche ID, aber
  über die PA ansprechbar; fremde Baudrate).
- **Rot = Fehler:** funktioniert nicht, bleibt draussen (Kollision, keine Daten, antwortet nicht).
- Gilt für Zeile, Streifen und Status-Chip in der Trefferliste und für die Zählerliste („antwortet
  nicht“). Wechsel „Merken über SA/PA“ ändert, was mitkommt, und damit die Farbe.
- Rot ist sonst nur für Gefährliches (Löschen).

## 12. Schnellscan: eigenes Projekt nötig? (umgesetzt: A, schlank, 19.10.2026)

**Entscheidung:** kein eigenes Projekt, kein Kennzeichen, kein automatisches Leeren. Begründung und
verworfene Varianten unten (12.1–12.4). Umgesetzt:

- **„Leeren …“ für jedes Projekt** (Projekt-Werkzeuge, quiet, danger, mit Rückfrage „N Zähler,
  ihre Auslesungen und der letzte Scan in allen Linien von <Projekt>“). Gelöscht: Zähler mit
  Datenpunkten (Cascade), Auslesungen und Datenpunkt-Verlauf (`clear_readings`, denn Auslesungen
  hängen an der Linie, beim Zähler nur `SET NULL`), Änderungsprotokoll (`clear_changes`, sonst
  stehen im Bericht Änderungen der vorigen Anlage) und der letzte Scan je Linie (`last_scan`,
  `issues`, `scans`: Trefferliste, Auffälligkeiten, Vergleich). Bleiben: Projekt, Gateways, Linien,
  Anschluss, Baudrate, Plan, Ausgänge. `Db::clear_project` nutzt die bestehenden Löschfunktionen
  in einer Transaktion; `POST /api/projects/{id}/clear` lehnt ab, solange ein Scan läuft oder
  eine Linie des Projekts ausliest oder Aufträge wartend hat.
- **Startprojekt einer neuen Datenbank heisst „Schnellscan“** (EN „Quick scan“); bestehende
  Datenbanken bleiben unverändert.
- Nicht gebaut: „Als Projekt speichern …“ (geht über Neues Projekt / Umbenennen).
- Zwei Linien am selben Port: als offener Punkt in konzept-dauerbetrieb.md, Abschnitt 11.

### 12.1 Wofür

Ein Techniker steckt den Laptop an eine fremde Linie und will nur wissen, **was da los ist**:
verbinden, scannen, Auffälligkeiten ansehen, vielleicht einen Zähler auslesen. Er will dafür kein
Projekt anlegen, keine Anlage beschreiben, nichts weitergeben, und das Ergebnis soll kein
bestehendes Projekt verschmutzen.

### 12.2 Was es heute schon gibt

- **Ein Standardprojekt existiert bereits:** beim ersten Start legt mbusio „Standard“ mit
  Gateway 1 und Linie 1 an (`Db::ensure_active`). Wer nichts einrichtet, scannt dort.
- **Der Modus ist schon abgeleitet (§9):** ein Projekt ohne Plan, Datenpunkte und Ausgang ist
  ein Service-Projekt. Reiter Weitergabe ohne Nummer, kein Zustand der Anlage, Nächster Schritt
  endet beim Bericht. Genau das ist ein Schnellscan, nur ohne Bericht.
- **Scannen merkt nichts von selbst:** Treffer kommen erst mit „Merken“ in die Zählerliste.
- **Bedienung von Hand geht vor (§9.4):** auch neben einem laufenden Plan.

Ein Schnellscan geht also heute schon. Was fehlt, ist kleiner als ein eigenes Projekt:

| Lücke | Folge heute |
| --- | --- |
| Name „Standard“ sagt nicht, wofür es da ist | man legt doch ein Projekt an, oder scannt im Kundenprojekt |
| Reste der letzten Linie bleiben | gemerkte Zähler von gestern; „Änderungen seit dem letzten Scan“ vergleicht mit einer **anderen** Anlage und meldet alles als neu/fehlt |
| Nächster Schritt drängt zu „Merken“ und „Auslesen“ | beim reinen Debuggen störend (× blendet ihn aus, aber jedes Mal) |
| Gefundenes in ein richtiges Projekt bringen | geht nur über Projekt umbenennen oder Zählerliste exportieren/importieren |

### 12.3 Varianten

**A: Kein neues Konzept, das Standardprojekt heisst „Schnellscan“ und lässt sich leeren**

- Neuinstallation: das Startprojekt heisst „Schnellscan“ statt „Standard“. Bestehende Datenbanken
  bleiben, wie sie sind.
- Im Projekt-Menü ein Knopf **„Leeren“** (quiet, mit Rückfrage): löscht Zähler, Auslesungen und
  den gemerkten letzten Scan aller Linien des Projekts, behält Anschluss und Baudrate. Damit
  startet jeder Einsatz sauber, auch in jedem anderen Projekt.
- **„Als Projekt speichern …“** fragt nach einem Namen, benennt das Projekt um und legt ein neues,
  leeres „Schnellscan“ an. Kein Kopieren, kein Verschieben von Zählern.
- Nächster Schritt: unverändert (Service-Weg). Kein Sonderfall.

Aufwand: ein Server-Aufruf „Projekt leeren“ (es gibt schon `DELETE /api/meters` je Linie), ein
Knopf, ein Name, ein Test. Keine neue Spalte, kein neuer Zustand.

**B: Eigenes Schnellscan-Projekt mit Kennzeichen**

- Spalte `projects.scratch`; genau ein solches Projekt, immer vorhanden, nicht löschbar.
- Dort: nur Reiter Scan, Zähler und Werkzeuge; kein Plan, keine Weitergabe; Nächster Schritt
  endet nach dem Scan; beim Wechsel auf eine andere Linie automatisch geleert.

Aufwand: neue Spalte und Migration, Sonderfälle in Reitern, Plan-Formular, Weitergabe, Guide,
Projekt löschen, Export/Import, dazu je Sonderfall ein Test.

### 12.4 Warum B nicht nötig ist

- B ist der **Modus-Schalter**, den §9.2 bewusst verworfen hat: jede Stelle müsste fragen „bin
  ich im Schnellscan?“. Was B ausblendet (Plan, Weitergabe, Zustand), blendet §9 im
  Service-Projekt schon aus, weil nichts eingerichtet ist.
- Der einzige echte Unterschied ist **„sauber anfangen“**. Das braucht jedes Projekt ab und zu
  (Zählertausch, zweiter Einsatz), also gehört es als „Leeren“ zu allen Projekten und nicht als
  Eigenschaft eines besonderen Projekts.
- Automatisch leeren (B) kann Arbeit wegwerfen; ein Knopf mit Rückfrage nicht.

### 12.5 Vorschlag

**Variante A.** Reihenfolge: (1) „Leeren“ für jedes Projekt, (2) Startprojekt heisst
„Schnellscan“, (3) „Als Projekt speichern …“. Jeder Schritt einzeln nutzbar; nach (1) entscheiden,
ob (2) und (3) überhaupt noch fehlen.

### 12.6 Offen für die Entscheidung

1. Soll „Leeren“ auch die Auslesungen (Verlauf) löschen, oder nur Zähler und letzten Scan?
2. Bestehende Datenbanken: „Standard“ umbenennen, wenn es leer ist, oder so lassen?
3. Am Raspberry Pi im Dauerbetrieb: Schnellscan auf einer Linie, die denselben Port wie eine
   geplante Linie benutzt, ist nicht abgefangen (zwei Linien, ein Port). Gehört eher zu
   konzept-dauerbetrieb.md; nur prüfen, ob es vor v0.5.0 nötig ist.

## 13. Dauerbetrieb je Linie einschalten (umgesetzt, 19.10.2026)

**Entschieden:** alle Empfehlungen aus 13.7 (Schalter je Linie, Weitergabe nur aus eingeschalteten
Linien, Migration wie 13.6). Abweichung von 13.5: `operation` wird im Projekt-Export zwar
geschrieben, beim **Import aber nie übernommen**; eine übernommene Anlage liest erst, wenn man sie
einschaltet (sonst würde ein Gateway-Export den Laptop einschalten). Nach dem Einschalten liest der
Plan ab dem nächsten Termin, wie beim erstmaligen Setzen eines Plans. Auf dem Handy zeigt die
Kopfzeile nur Punkt und Anzahl.

### 13.1 Problem

Heute liest der Zeitplan **jede Linie aller Projekte**, auf der ein Plan nicht „Aus“ ist, und gibt
gewählte Werte weiter, sobald MQTT an ist (§9: abgeleitet, kein Schalter). Für ein Gateway mit
einer Anlage passt das. Für den Laptop des Servicetechnikers nicht:

- Nach 100 Einsätzen sind viele Projekte da, manche mit Plan oder Datenpunkten vom Testen. Der
  Laptop liest im Hintergrund weiter, versucht Ports, die längst nicht mehr dran sind (Meldungen
  „nicht verbunden“), und Simulator-Linien liefern munter Werte.
- Ist MQTT an, gehen Werte aus alten Projekten an den Broker, auch beim blossen Auslesen von Hand.
- Ein Plan im **Projekt** gilt für alle Linien darin; wer ihn zum Testen setzt, schaltet damit
  ungewollt alle Linien ein.
- Nirgends sieht man auf einen Blick, was auf diesem Gerät gerade im Hintergrund läuft.

### 13.2 Vorschlag: Schalter „Dauerbetrieb“ je Linie

**Eine Linie liest nur automatisch und gibt nur weiter, wenn ihr Dauerbetrieb eingeschaltet ist.**
Standard: aus. Plan, Datenpunkte und Ausgänge bleiben Einstellungen; sie wirken erst, wenn die
Linie eingeschaltet ist.

| | Dauerbetrieb aus (Standard) | Dauerbetrieb ein |
| --- | --- | --- |
| Geplantes Auslesen | nein, auch wenn ein Plan eingestellt ist | ja, nach Plan |
| Weitergabe (MQTT) | nein, auch nicht beim Auslesen von Hand | ja, gewählte Werte |
| Hintergrund-Verbindung | nein (nur solange man die Linie von Hand benutzt) | ja, Dienst hält sie |
| Qualität an den Broker | nein | ja (`disturbed`, `stale`) |

Wort: „**Dauerbetrieb**“, nicht „aktiv“. „Aktiv“ heisst heute schon „diese Linie ist in der
Oberfläche geöffnet“ (Linien-Leiste); zwei Bedeutungen würden verwirren.

### 13.3 Wo man es sieht und schaltet

- **Linien-Leiste:** rechts ein eigener Block „Dauerbetrieb“ mit **grossem Schalter** (Zustand und
  Bedienung in einem), daneben „läuft“ bzw. „aus · nur von Hand“, darunter der Plan mit ✎ (öffnet
  den Plan der Linie). Ein: grüne Fläche. Ein, aber nicht als Dienst: zusätzlich „nicht als Dienst –
  stoppt beim Schliessen“ (orange, Tooltip mit Anleitung). Aus mit Plan: „Plan stündlich (wartet)“. Aus ohne Plan: Link „Plan
  festlegen“; der Schalter führt dann zum Plan bzw. zum Anschluss statt eine Fehlermeldung zu
  zeigen. Ausschalten fragt nach. Handy: Block über die ganze Breite.
- **Projektbaum:** derselbe Schalter klein unter jeder Linie, dort direkt schaltbar (auch für
  Linien, die nicht geöffnet sind). Ohne Plan blass mit „kein Ausleseplan“; der Schalter öffnet
  dann den Plan der Linie.
- **Kopfzeile, überall sichtbar:** „Dauerbetrieb: 3 Linien in 2 Projekten“, aufklappbar mit Liste
  (Projekt › Gateway › Linie, nächster Termin, Zustand), jede Zeile öffnet die Linie. Steht nichts
  im Dauerbetrieb, steht dort nichts. So sieht der Techniker sofort, was sein Laptop im Hintergrund
  tut, und der Integrator, was das Gateway tut.
- **Nächster Schritt:** „Dauerbetrieb einrichten“ führt wie heute durch Plan → Werte → Ausgang und
  endet mit **„Dauerbetrieb einschalten“** für die Linie.
- **Hinweis „Läuft nur, solange mbusio geöffnet ist“** (Konzept Hintergrund) erscheint, sobald
  irgendeine Linie im Dauerbetrieb ist und mbusio nicht als Dienst läuft.

### 13.4 Was sich an §9 ändert

`inOperation()` wird einfacher: ein Projekt ist im Dauerbetrieb, wenn **eine seiner Linien
eingeschaltet** ist (statt „Plan, Datenpunkt oder Ausgang eingerichtet“). Reiter Weitergabe mit
Nummer, Zustand der Anlage usw. richten sich danach. Der Merker „Dauerbetrieb einrichten gedrückt“
im Browser bleibt für den Weg dorthin.

### 13.5 Umsetzung (klein)

- Datenbank: Spalte `lines.operation` (0/1, Standard 0); Import übernimmt sie nicht.
- Zeitplan (`Db::planned`) nimmt nur eingeschaltete Linien; ausgeschaltete Linien setzen keine
  Termine und halten keine Verbindung (`stop_unused_services` wie heute).
- Weitergabe (`dispatch`, `outputs_refresh`) nur für Datenpunkte eingeschalteter Linien.
- API `PUT /api/lines/{id}/operation {on}`; lehnt „ein“ ohne wirksamen Plan ab.
- Oberfläche: Schalter, Kennzeichen, Liste in der Kopfzeile, Guide-Schritt, Texte DE/EN.
- Tests: Rust (nicht eingeschaltet → kein Termin, keine Weitergabe; ein → wie heute), UI (Schalter,
  Kopfzeile, Guide bis „einschalten“).

### 13.6 Bestehende Daten

v0.5.0 ist noch nicht veröffentlicht. Vorschlag: Bei der Migration werden Linien eingeschaltet,
auf denen heute ein Plan wirkt **und** mbusio als Dienst läuft (Pi, Windows-Dienst), damit ein
laufendes Gateway nach dem Update weiterliest. Im Fenster (Laptop) bleibt alles aus.

### 13.7 Zur Entscheidung

1. Schalter je **Linie** (Vorschlag) oder je Projekt? Je Linie passt zu „jede Linie ihr eigener
   Bus“ und erlaubt, eine Linie zum Debuggen herauszunehmen.
2. Weitergabe auch nur bei eingeschalteten Linien (Vorschlag: ja, sonst sendet Auslesen von Hand
   in alten Projekten an den Broker)?
3. Migration wie 13.6, oder einfach alles aus?

## 14. Leitbild: Werkzeug, nicht Webseite (25.10.2026)

**Ausgangslage:** mbusio fühlt sich an wie eine Webseite mit vielen Funktionen: Karten mit viel
Luft, Seite lädt beim Wechsel von Projekt oder Linie neu, Rückfragen als Browser-Fenster,
geführter Ablauf überall, kaum Tastatur. Ziel ist ein **professionelles Werkzeug**: direkt,
präzise, schnell, dicht und für Leute gebaut, die es täglich benutzen.

Diese Prinzipien gelten für jede Änderung an der Oberfläche, in dieser Rangfolge:

1. **Selbsterklärend statt erklärt.** Was ein Element ist, in welchem Zustand es ist und was
   eine Aktion bewirkt, sieht man am Element selbst: sprechende Beschriftung, Zustand als Farbe
   und Wort direkt daneben, die sinnvolle nächste Aktion dort, wo man hinschaut. Erklärtexte,
   Hilfekästen und der geführte Ablauf sind Ergänzung für Neue, nie Voraussetzung. Braucht ein
   Element eine Erklärung, um verstanden zu werden, ist das Element falsch, nicht die Erklärung
   zu kurz. Tooltips nur für Details, nie für das Wesentliche.
2. **Arbeitskontext bleibt.** Kein Neuladen der Seite, keine Sprünge an den Seitenanfang,
   Auswahl, aufgeklappte Bereiche, Filter und Scrollposition bleiben beim Wechsel von Linie,
   Reiter oder nach einer Aktion erhalten.
3. **Direkt bedienen.** Werte dort ändern, wo sie stehen (in der Zeile, im Feld), nicht über
   Umwege. Keine Browser-Dialoge (`confirm`, `prompt`, `alert`): Rückfragen im Kontext, nur
   für Gefährliches; Rückgängig, wo es passt, statt Rückfrage.
4. **Dichte statt Luft.** Bildschirm für Inhalt nutzen: kompakte Zeilen, kleine Abstände, keine
   Karte um jeden Inhalt, Überschriften so gross wie nötig. Was oft zusammen gebraucht wird,
   steht gleichzeitig da (z.B. Liste und Details nebeneinander).
5. **Tastatur und Auswahl.** Tab-Reihenfolge stimmt, Enter bestätigt, Escape bricht ab,
   Pfeiltasten in Listen, Shortcuts für das Häufigste. Mehrfachauswahl in Listen mit
   Aktionen für die Auswahl (auslesen, Plan, Werte, löschen).
6. **Sofortige, ruhige Rückmeldung.** Jede Eingabe zeigt sofort Wirkung, lange Vorgänge
   blockieren nichts und zeigen Fortschritt am Ort. Keine Animationen, die warten lassen.
7. **Konsistent.** Gleiches sieht gleich aus und verhält sich gleich (Knöpfe §11.1, Farben
   §11.2, Schalter für Ein/Aus, gleiche Wörter für gleiche Aktionen).

**Geräte:** Desktop/Laptop zuerst (dort wird eingerichtet und ausgewertet). Handy bleibt
vollständig bedienbar (Techniker am Strang), darf aber anders angeordnet sein; es bestimmt nicht
die Dichte am Desktop.

**Vorgehen:** Bestandsaufnahme mit diesem Leitbild (ohne Code zu ändern), Schwachstellen nach
Nutzen, Häufigkeit, Aufwand und Risiko, getrennt in sofort machbar und grösserer Umbau.
Technische Aussagen mit Code-Stellen oder Messungen belegen. Danach entscheidet der Nutzer,
umgesetzt wird in kleinen Schritten mit Tests. Kein Wechsel des Frameworks, keine Änderung nur
weil etwas moderner aussieht.

## 15. Knöpfe erkennbar, Platz in den Leisten (31.10.2026)

- **Jeder Knopf sieht aus wie ein Knopf.** Randlose Textknöpfe waren in der Praxis fast unsichtbar,
  darum haben `quiet` und `icon` einen Rahmen in Akzentfarbe (der Ton `--line-strong` hat auf Weiss
  nur etwa 1,5:1). Ohne Rahmen bleiben nur Menüpunkte und der Aufklapp-Pfeil im Baum.
- **Nächster Schritt oben rechts** in der Laschenleiste (löst den Platz unten aus §10 ab). Genau ein
  gefüllter Knopf in den Leisten (Aktionen für einen gewählten Eintrag dürfen zusätzlich gefüllt sein): Steht der eigene Hauptknopf des Schritts schon da, tritt der Schritt-Knopf
  zurück (`tonal`); sonst tritt der Hauptknopf der Seite zurück (`next-elsewhere`).
- **Anzeigen sehen nicht wie Aktionen aus:** „N im Dauerbetrieb“ ist `plain`, kein Rahmen, ▾ fürs Menü.
- **Orange nur für Probleme:** Kein Ziel in den Werkzeugen ist der Ausgangszustand, also kein Orange.
- **Dunkelmodus nach Messung:** Feldrahmen `--line-strong` mindestens 3:1 zur Fläche, Tabellenköpfe
  sichtbar abgesetzt, auf `--err` immer `--err-ink` statt Weiss (Weiss hatte nur 2,5:1).
- **Farben:** `--line-strong` auch hell mindestens 3:1. `--ok` hat mehr Gelbanteil, damit es sich
  von `--accent` trennt; Teal bleibt, weil es auch im Icon und im Bericht steckt.
- **Wort vor Symbol, Klarheit vor Dichte:** Die Ausgänge behalten „Einstellungen“ und „aus“ und
  belegen dafür bei 1250 px zwei Zeilen. Ein Versuch mit ⚙ statt des Worts wurde verworfen.
- **„N im Dauerbetrieb“** nur, wenn es mehr sagt als der Schalter daneben: mehrere Linien, eine
  andere Linie oder eine Warnung (z.B. nicht als Dienst).
- **Anlage:** Textspalten ohne Mindestbreite, die Aktionen bleiben immer ganz. Unter 1250 px stehen
  Zähler und Datenpunkte nur noch im Detailbereich.

## 16. Navigation in der Seitenleiste (1.11.2026)

Ab 1000 px stehen Kopfzeile und Laschen links in einer Spalte: Projekt und Linie, Anschluss,
Dauerbetrieb, Bereiche, unten Einstellungen. Die Funktionen sind dieselben, nur der Platz ist neu;
darunter bleibt die Anordnung wie bisher. Menüs gehen neben der Leiste auf. Der Inhalt ist dadurch
232 px schmaler, darum greifen die Regeln für mittlere Breiten entsprechend früher. Erster Schritt
zum Entwurf „mbusio Design-Entwurf“; Übersicht und feste Orte pro Thema folgen erst nach Prüfung.
