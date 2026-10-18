# Konzept: Bedienung und Aufbau der Oberfläche

Stand: 18.10.2026 · Status: **abgestimmt, in Umsetzung**

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

## 6. Entscheidungen (18.10.2026)

1. Spalte „Senden“ in der Auslesung fällt weg; dort nur ein Link zur Weitergabe (bzw. „In
   Zählerliste aufnehmen“, falls der Zähler noch nicht in der Liste steht).
2. Reiter Weitergabe zeigt standardmässig die aktive Linie, umschaltbar auf das ganze Projekt.
3. Reiter: 1 Projekt, 2 Scan, 3 Zähler, 4 Weitergabe; Werkzeuge und Simulator ohne Nummer.
