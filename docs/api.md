# HTTP-API von mbusio

Stand: 25.10.2026 · gilt für mbusio ab 0.5 · für den Umbau der Oberfläche und für eigene Programme

Die Oberfläche ist nur ein Kunde dieser API: alles, was sie kann, geht auch hierüber. Die Logik
liegt in mbusio; eine neue Oberfläche muss nur diese Endpunkte benutzen.

Pfade relativ zu `http://<gerät>:8080`. Anfragen und Antworten JSON, ausser wo Datei angegeben.

---

## 1. Was für alle gilt

| Thema | Regel |
| --- | --- |
| **Zugriffsschlüssel** | Wenn mbusio mit `--token` läuft: Header `X-Token: <schlüssel>` oder `?token=` (für Downloads/Links). Sonst 401. |
| **Sprache** | `?lang=de|en` (Downloads), sonst Header `X-Lang`, sonst `Accept-Language`. Betrifft Meldungen, Bezeichnungen, CSV-Spalten. |
| **Fehler** | `{"error": "verständlicher Text"}` mit Status **400** (Eingabe falsch), **404** (gibt es nicht), **409** (Bus belegt: Scan läuft), **502** (Bus/Zähler: keine Antwort, Prüfsumme …), 401 (Schlüssel). Den Text kann man direkt anzeigen. |
| **Aktive Linie** | Scan, Zählerliste, Auslesen und Werkzeuge arbeiten auf der **aktiven Linie** (`active` in `/api/status`, wechseln mit `POST /api/lines/{id}/activate`). |
| **Aufträge im Hintergrund** | Scans und „Alle auslesen“ antworten sofort mit dem Auftrag (`JobStatus`); Fortschritt mit `GET /api/job` abfragen, abbrechen mit `POST /api/job/cancel`. Nur ein Auftrag gleichzeitig (sonst 409). |
| **Vorschau → Übernehmen** | Massenänderungen (Importe, Namen, Adressen nach Muster) erst mit `"apply": false` schicken: Antwort `ChangeSet` zeigt, was sich ändern würde; dann dasselbe mit `"apply": true`. |
| **Änderungen** | Jede erfolgreiche POST/PUT/DELETE-Anfrage meldet sich bei mitlaufenden Empfängern (Node-RED-Strom). |
| **Zeiten** | Ortszeit `"YYYY-MM-DD HH:MM:SS"`; wo UTC dabei ist, Sekunden seit 1970 (`utc`) bzw. RFC 3339. |

### 1.1 Wiederkehrende Typen

**ChangeSet** (Vorschau/Ergebnis): `added` (neue Einträge), `changed` (je Eintrag Feld, alt, neu),
`unchanged` (Anzahl), `warnings` (übernehmen geht trotzdem), `errors` (dann wird nichts
übernommen), `applied` (true nach `apply`).

**JobStatus** (Auftrag): `kind`, `running`, `done`/`total` (Fortschritt), `message`, `hits`
(Treffer eines Scans, `ScanHit`), `error`; bei Scans `scope` (Bereich), `previous` (letzter gleicher
Scan zum Vergleich: `{time, hits}`), `resume` (abgebrochener Sekundärscan: `{mask, baud}`).

**ScanHit**: `primary_address`, `secondary_address` (16 Hex), `id`, `manufacturer`, `medium`
(Schlüssel, z.B. `medium_water`), `version`, `status` (`ok` · `collision` · `no_data`), `baud`,
`reason` (Schlüssel der Ursache), `raw` (Bytes für die Telegramm-Analyse).

**MeterReading** (Auslesung): `timestamp`, `target`, `primary_address`, `header` (`id`,
`manufacturer`, `medium`, `status`, `secondary_address` …), `records` (je Wert: `quantity`,
`function`, `storage`, `tariff`, `subunit`, `unit`, `value`, `value_text`, `notes`, `dif`, `vif`,
`raw`), `raw_frames`, `time_unsure`, `utc`.

**Meter** (Zähler der Liste): `id`, `line_id`, `name`, `location`, `note`, `address` (PA, ID oder
Sekundäradresse), `baud` (eigene), `meter_id`, `manufacturer`, `medium`, `primary_address`,
`secondary_address`, `last_read`, `last_value` (Hauptwert als Text), `last_quantity`,
`last_error`, `plan` (eigener Plan, `null` = wie Linie).

**Project** · **Gateway** · **Line**: siehe Abschnitt 6.

Bezeichnungen zu Schlüsseln (`energy`, `medium_water`, `checksum` …): `GET /api/labels`.

---

## 2. Zustand und Verbindung

| | |
| --- | --- |
| `GET /api/status` | **Alles für Kopfzeile und Statusleiste**, alle paar Sekunden abfragen. Felder: `version`, `active` (Projekt, Gateway, Linie), `connected`, `connection` (`port`, `baud`, `timeout_ms`, `timeout_auto`), `line` (Linien-Dienst: `connected`, `busy`, `queued`, `error`), `job_running`, `plans` (laufende Plan-Durchgänge je Linie: `label`, `done`, `total`, `ok`), `plan_active`, `held`, `time` (Uhrzeit: `confirmed`, `source`, `since`, `offset_s`), `disk` (`level` ok/low/full, `free_pct`, `free_bytes`, `total_bytes`, `db_bytes`, `per_day` = erwarteter Zuwachs je Tag beim heutigen Plan, `days_left` = so lange reicht der Platz, ohne Plan `null`), `mqtt`, `modbus` (Zustand der Ausgänge), `outputs` (Liste: `key`, `label`, `address_label`, `on`), `background` (läuft als Dienst), `os`, `token_required`, `now`, `warnings` (Meldungen: `open`, davon `unseen`). |
| `GET /api/ports` | Anschlüsse zur Auswahl: `[{port, label, dev}]`, USB-Wandler mit Namen zuerst, Simulatoren am Ende. |
| `POST /api/connect` | `{port, baud, timeout_ms?}` → `{ok, connection}`; Anschluss wird bei der aktiven Linie gespeichert. Ohne `timeout_ms` bleibt der Timeout der Linie (früher: auf 0 zurückgesetzt). |
| `POST /api/disconnect` | Trennen → `{ok, plan_paused}`. |
| `POST /api/time` | Uhrzeit vom Gerät übernehmen: `{epoch_ms}` → `{ok, offset_s, time}`. |
| `GET /api/log?since=<nr>` | Telegramm-Log ab Nummer (für laufende Anzeige). `DELETE /api/log` leert es. |

## 3. Scan (aktive Linie)

| | |
| --- | --- |
| `POST /api/scan/primary` | `{from=0, to=250, retries=1, bauds=[]}` (leer = Baudrate der Linie) → `JobStatus`. |
| `POST /api/scan/secondary` | `{bauds=[], prefix="" (ID beginnt mit, 0–7 Ziffern), resume?}` → `JobStatus`. |
| `GET /api/job` | Laufender oder letzter Auftrag (`JobStatus`). |
| `POST /api/job/cancel` | Abbrechen → `{ok}`. |
| `GET /api/last-scan` | Letzter Scan der aktiven Linie (oder `null`). |
| `POST /api/bus-bauds` | Auf welchen Baudraten antworten Geräte? `{bauds=[]}` → `{bauds}`. |
| `POST /api/free-address` | Freie Primäradresse suchen: `{from, skip=[]}` → `{address}`. |
| `PUT /api/lines/{id}/issues` | Auffälligkeiten des Scans speichern (für Projekt-Karte und Bericht). |

## 4. Werkzeuge: einzelner Zähler (aktive Linie)

`address` überall: Primäradresse (`"5"`), ID (8 Ziffern) oder Sekundäradresse (16 Hex).
`from_baud`: Zähler läuft mit anderer Baudrate als der Bus.

| | |
| --- | --- |
| `POST /api/read` | `{address, baud?}` → `MeterReading` (in der Liste: wird beim Zähler gespeichert). |
| `POST /api/set-address` | Primäradresse ändern: `{address, new_address, from_baud?}` → `{ok, new_address}`; wird geprüft (Ziel frei, nur ein Zähler) und protokolliert. Steht der Zähler in der Liste, führt mbusio sie nach (über die PA gemerkt: neue Adresse; sonst PA nachgetragen). |
| `POST /api/switch-baud` | `{address, baud, from_baud?}` → `{ok, baud}`. Zählerliste wird nachgeführt (eigene Baudrate nur, wenn sie von der Linie abweicht). |
| `POST /api/detect-baud` | `{address}` → `{bauds}` (auf welchen er antwortet). |
| `POST /api/reset` | Application Reset: `{address, subcode?, from_baud?}` → `{ok}`. |
| `POST /api/raw` | Rohe Bytes senden: `{hex: "10 40 01 41 16"}` → `{kind, hex, decoded?}`. |
| `POST /api/analyze` | Telegramm erklären: `{hex}` → `{summary, fields, problems}`. |

## 5. Zählerliste, Auslesungen, Verlauf (aktive Linie)

| | |
| --- | --- |
| `GET /api/meters` | Zähler der aktiven Linie (`Meter` + `plan_info`: gültiger Plan, nächster Termin, gestört/ausgesetzt). `fault`: Fehler, den der Zähler in der letzten Auslesung selbst meldet (`status_error`, `error_flags`, `power_low`, sonst `null`). `swap`: erkannter, nicht bestätigter Zählertausch `{id, time, old_id, new_id}` oder `null`. |
| `POST /api/meters` | Zähler merken: `Meter` (mindestens `address`) → `Meter`. |
| `PUT /api/meters/{id}` | Ändern (Name, Ort, Adresse, Baudrate …): `Meter` → `Meter`. |
| `DELETE /api/meters/{id}` | Entfernen. `DELETE /api/meters` leert die Liste → `{ok, removed}`. |
| `POST /api/meters/{id}/read` | Einen Zähler auslesen → `MeterReading`. |
| `POST /api/meters/read-all` | Alle auslesen (auch gemischte Baudraten) → `JobStatus`. |
| `GET /api/meters/{id}/history` | Verlauf: `{meter, points: [{timestamp, time_unsure, values: [{key, quantity, unit, tariff, value, text}]}]}`. |
| `POST /api/meters/{id}/exchange` | Zählertausch: `{address, baud?}` des neuen → `{ok, details, reading}`. |
| `POST /api/exchanges/{id}/confirm` | Erkannten Zählertausch bestätigen (`id` aus `swap`) → `{ok}`. |
| `PUT /api/meters/{id}/plan` | Eigener Plan des Zählers: `{plan}` (`null` = wie Linie) → `{ok, warnings}`. |
| `POST /api/meters/import` | Zählerliste aus mbusio (JSON): `{meters: [Meter]}` → `{ok, added}`. |
| `POST /api/meters/import-list` | Liste vom Kunden (CSV/Excel, schon zerlegt): `{rows: [{id, pa, name, location, note}], apply}` → `ChangeSet`. |
| `POST /api/meters/names` | Namen nach Muster: `{pattern, ids?, only_empty, apply}` → `ChangeSet`. |
| `GET /api/meters/csv` | Zählerliste als CSV (Datei). |
| `GET /api/readings` | Die letzten 5000 Auslesungen der aktiven Linie (`MeterReading`), älteste zuerst. `DELETE` löscht sie. |
| `GET /api/export?format=csv|json` | Auslesungen als Datei. |

## 6. Projekte, Gateways, Linien

**Project**: `id`, `name`, `customer`, `site`, `technician`, `order_no`, `remarks`, `created`,
`plan` (Vorgabe für alle Linien, `"off"` …), `thin_days`.
**Gateway**: `id`, `project_id`, `name`, `address`, `note`.
**Line**: `id`, `gateway_id`, `name`, `connection` (COM3, /dev/ttyUSB0, tcp://…, SIMULATOR …;
leer = noch nicht festgelegt), `baud`, `timeout_ms`, `addressing` (`secondary`/`primary`), `plan`
(`null` = wie Projekt), `operation` (Dauerbetrieb).

| | |
| --- | --- |
| `GET /api/projects` | **Baum**: `{projects: [Project + gateways: [Gateway + lines: [Line + meters (Anzahl), last_read, scanned, issues, plan_info, datapoints {count, invalid}]]], active}`. |
| `POST /api/projects` | Anlegen: `Project` → `{ok, project}` (mit erster Linie, wird aktiv). |
| `PUT /api/projects/{id}` · `DELETE` | Ändern → `Project` · Löschen. |
| `POST /api/projects/{id}/clear` | Projekt leeren (Zähler, Auslesungen, Änderungen, letzter Scan; Linien bleiben) → `{ok, removed}`. |
| `GET /api/projects/{id}/export` | Projektdatei (JSON, Datei). `POST /api/projects/import` mit deren Inhalt → `{ok, project}`. |
| `GET /api/projects/{id}/billing` | Export für die Abrechnung: `{settings, kinds, preview, error, files}`. `files`: abgelegte Dateien des automatischen Exports `{name, size, time, mail}`, neueste zuerst; `mail`: `{sent, error}` oder `null` (nicht zu senden). `settings`: `form` (`billing`, `series`), `step` (`month`, `day`), `main` (Hauptwert je Zähler), `keys` (weitere Werte), `period` (`prev_month`, `prev_quarter`, `prev_half`, `prev_year`, `custom` mit `from`, `to` als JJJJ-MM-TT), `decimal` (`,`, `.`, leer = nach Sprache), `auto` (nach Ende des Zeitraums als Datei ablegen; ruht bei `custom`), `mail_to` (Empfänger der abgelegten Dateien, mit Komma; leer = nur ablegen), `lines` (nur diese Linien; leer = alle, gelöschte zählen nicht), `compare_years` (0–10, Vorgabe 1: Verbrauch derselben Zeiträume früherer Jahre als Spalten vor „Hinweis“), `wait_days` (0–31, Vorgabe 2), `mail_subject` (Platzhalter `{projekt}` `{zeitraum}` `{von}` `{bis}` `{geraet}`; leer = Vorgabe). Dazu `subject` (Betreff für den gezeigten Zeitraum) und `subject_default`. `kinds`: Werte im Projekt `{key, label, unit, meters, main}`. `lines`: Linien mit Zählern `{id, label, meters}`. `preview`: `{from, to, header, rows (die ersten 30), total, notes, summary}`; `summary`: Zeilen je Art des Hinweises (`swap`, `swap_open`, `swap_gap`, `missing`, `decreased`, `not_read`, `no_answer`, `disturbed`, `fault`, `power_low`, `unsure`, `never_read`); bei unvollständigem Zeitraum `null` und `error`. |
| `PUT /api/projects/{id}/billing` | Einstellungen speichern (ganzes `settings`); Antwort wie `GET`. |
| `GET /api/projects/{id}/billing/csv` | Ganze Tabelle nach den gespeicherten Einstellungen als CSV (Datei). |
| `GET /api/projects/{id}/billing/files/{name}` | Abgelegte Datei herunterladen (nur Namen aus `files`). |
| `POST /api/projects/{id}/billing/mail` | Abrechnung mit den gespeicherten Einstellungen jetzt an `mail_to` senden → `{ok, to, file}`; Fehler mit Grund. |
| `POST /api/projects/{id}/billing/files/{name}/send` | Datei jetzt an `mail_to` senden → wie `GET …/billing`; Fehler mit Grund (steht danach auch an der Datei). |
| `GET /api/mail` | Ausgang E-Mail (einmal je Gerät): `{config: {enabled, device, host, port, security (starttls, tls, none), insecure, username, password (immer leer), from}, has_password, device (Gerätename wie verwendet), last: {time, subject, error} oder null}`; `config.retry_days` (1–60, Vorgabe 7): so lange werden nicht gesendete Dateien erneut versucht. |
| `PUT /api/mail` | Speichern: `{config, password?}` (`password` weglassen = behalten, `""` = löschen). |
| `GET /api/warnings` | Meldungen (offene Probleme, die lange genug bestehen), neueste zuerst: `{items: [{key, kind, text, place, since, line, meter, project, seen}]}`; `kind` aus disturbed, fault, battery, swap, line, mqtt, disk, billing. Einstellungen in `config.warn` von `/api/mail`: `{to, subject, off (ausgeschaltete Arten), line_minutes (30), mqtt_minutes (60), interval_minutes (0, 15, 60, 240, 1440; Vorgabe 60), resolved (true), remind (false)}`; `GET /api/mail` liefert dazu `warn_subject_default`. Bericht in `config.report`: `{to, subject, frequency (off, daily, weekly; Vorgabe daily), time ("07:00"), weekday (0 = Montag … 6), off (abgewählte Teile aus reading, outputs, problems, billing, disk)}`; dazu `report_subject_default`. |
| `POST /api/warnings/seen` | `{keys}` als gesehen markieren (leer = alle) → `{ok}`. |
| `POST /api/mail/report` | Bericht jetzt mit den gespeicherten Einstellungen (Zeitraum seit dem letzten Bericht, sonst ein Tag bzw. eine Woche), ändert den Zeitplan nicht → `{ok, to}` oder Fehler mit Grund. |
| `POST /api/mail/test` | Testmail mit den gespeicherten Einstellungen (auch wenn ausgeschaltet): `{to}` → `{ok}` oder Fehler mit Grund. Jede Mail steht im Protokoll (`GET /api/out-log`, Ausgang `email`, Ergebnis `sent` oder `failed` mit Grund). |
| `DELETE /api/projects/{id}/changes` | Änderungsprotokoll leeren → `{ok, removed}`. |
| `POST /api/projects/{id}/gateways` | Gateway anlegen: `Gateway` → `Gateway`. `PUT`/`DELETE /api/gateways/{id}`. |
| `POST /api/gateways/{id}/move` · `/api/lines/{id}/move` | Reihenfolge: `{dir: -1|1}`. |
| `POST /api/gateways/{id}/lines` | Linie anlegen: `Line` → `Line`. `PUT`/`DELETE /api/lines/{id}`. |
| `POST /api/lines/{id}/activate` | Aktive Linie wechseln (verbindet, falls Anschluss bekannt) → `{ok, connected, error?}`. |
| `PUT /api/lines/{id}/addressing` | Zähler über SA oder PA merken: `{mode}` → `{ok, changed, kept}`. |
| `GET /api/report?project=` | Daten für den Bericht (Projekt, Linien, Zähler, Änderungen, Auffälligkeiten). |

## 7. Ausleseplan und Dauerbetrieb

Plan als Text: `"off"`, `"monthly 00:05"`, `"daily 00:05"`, `"hourly"`, `"15min"` (24-h-Zeit;
siehe `plan.rs`). Antworten mit `warnings` (z.B. Buslast zu hoch).

| | |
| --- | --- |
| `PUT /api/projects/{id}/plan` | `{plan}` → `{ok, warnings}`. |
| `PUT /api/lines/{id}/plan` | `{plan}` (`null` = wie Projekt) → `{ok, warnings}`. |
| `PUT /api/lines/{id}/operation` | Dauerbetrieb `{on}`; braucht Anschluss und wirksamen Plan (sonst 400 mit Grund). |
| `PUT /api/projects/{id}/thin` | Verlauf ausdünnen: `{days}` (`null` = alles behalten); behalten wird je Zähler und Tag die letzte Auslesung. |
| `GET/PUT /api/storage` | Speicherschutz des Geräts: `{protect, first_days, second_days}` (Vorgabe `true`, 365, 90; erste ≥ zweite ≥ 30). |

## 8. Datenpunkte und Weitergabe

**Datapoint**: `id`, `meter_id`, `no` (stabile Nummer im Projekt), `key` (Kennung des Werts),
`name`, `unit`, `factor`, `shift`, `send` (`every`/`change`/`daily`), `active`,
`outputs` (`{"mqtt": {enabled, address}, "modbus": {…}}`); in Listen zusätzlich `quality`
(`good`/`stale`/`disturbed`/`invalid`/`none`), `last_value`, `last_time`, `label`, `line`, `meter`.

| | |
| --- | --- |
| `GET /api/datapoints?scope=line|project` | `{meters: [Zähler mit datapoints], templates, outputs}` für den Reiter Weitergabe. |
| `GET /api/meters/{id}/values` | Werte der letzten Auslesung zur Auswahl (Kennung, Bezeichnung, Wert, schon Datenpunkt?, Hauptwert?). |
| `PUT /api/meters/{id}/datapoints` | Auswahl setzen: `{keys, outputs=[], template?: {auto}}` → `{ok, added, removed, others}`. |
| `PUT /api/datapoints/{id}` | Ändern: `Datapoint` → `Datapoint`. Adresse, die ein anderer Datenpunkt hat: 400. |
| `POST /api/datapoints/bulk` | `{ids, action}`: `on`, `off`, `delete`, `send:every|change|daily`, `out:<ausgang>:on|off` → `{ok, changed}`. |
| `DELETE /api/templates/{id}` | Vorlage löschen. |
| `GET /api/projects/{id}/datapoints/csv` | Datenpunkte als CSV (Datei, Spalten je Ausgang). |
| `POST /api/projects/{id}/datapoints/import` | `{rows: [[felder]], apply}` → `ChangeSet`. |

## 9. Ausgänge (MQTT, Modbus)

| | |
| --- | --- |
| `GET /api/mqtt` | `{config, has_password, status}`; das Passwort kommt nie zurück. |
| `PUT /api/mqtt` | `{config, password}`: `password` `null` = gespeichertes behalten, `""` = löschen. Felder von `config`: `enabled`, `host`, `port`, `tls`, `insecure`, `ca`, `username`, `client_id`, `base`, `topic` (Muster), `time_sync`, `ha`, `ha_prefix`. Felder, die man nicht schickt, gehen auf die Vorgabe zurück: immer die ganze Einstellung schicken. |
| `GET /api/modbus` | `{config, status, projects}`; `status.peers`: je Leitsystem (IP) `last` (letzte Abfrage) und `requests`. |
| `GET /api/out-log` | Protokoll der Weitergabe, neueste zuerst, höchstens 500 Zeilen. Filter: `output`, `dp`, `days` (1, 7, 30; 0 = alle 30 Tage), `result` (`sent`, `waiting`, `dropped`), `q` (Suche in Name und Adresse). Antwort `{rows, counts, more}`; Zeile: `time`, `output`, `dp`, `name`, `address` (volles Topic), `value`, `quality`, `result`, `reason` (`too_old`, `too_many`). Was wartet, kommt aus der Warteschlange. `format=csv`: die gefilterten Zeilen für Excel. |
| `PUT /api/modbus` | `{enabled, bind, port, project (0 = aktives), swap_words}`. |
| `POST /api/projects/{id}/outputs/{key}/readdress` | Alle Adressen eines Ausgangs nach Muster: `{apply}` → `ChangeSet`. |
| `GET /api/projects/{id}/outputs/{key}/list?format=csv|json` | Liste für den Integrator (Datei); im JSON zusätzlich, was ein Leitsystem wissen muss (MQTT: Topics, QoS; Modbus: Registerblock, Wortreihenfolge). |

## 10. Simulator

| | |
| --- | --- |
| `GET /api/sim` | Vorlagen und eigene Simulationen, Medien, Fehlerarten, aktive Linie. |
| `POST /api/sim` | Anlegen: `{name, from?}` (Kopie) → `{ok, key}`. |
| `PUT /api/sim/{key}` | Speichern: `{name, meters: [{primary, id, manufacturer, version, medium, baud?, fault}]}` → `{ok, restarted}`. |
| `DELETE /api/sim/{key}` | Löschen. `POST /api/sim/{key}/use` verbindet die aktive Linie damit → `{ok, connection}`. |

## 11. Node-RED (laufende Werte)

Siehe [konzept-node-red.md](konzept-node-red.md): `GET /api/nodered/tree` (Projekt › Gateway ›
Linie › Zähler › Datenpunkte mit Wert, `version: 1`) und `GET /api/nodered/stream?project=&line=&meter=`
(Server-Sent Events `value` und `changed`). Auch für eine Oberfläche nutzbar, die Werte live
zeigen will, ohne abzufragen.

---

## 12. Hinweise für eine neue Oberfläche

- **Abfragen:** `/api/status` alle paar Sekunden reicht für Kopfzeile, Statusleiste und Zustand der
  Ausgänge; während eines Auftrags zusätzlich `/api/job` (Fortschritt, Treffer).
- **Alles Lange läuft als Auftrag** (Scan, alle auslesen): sofort zurück, Fortschritt abfragen,
  abbrechen möglich. Einzelne Befehle (auslesen, Adresse ändern) antworten erst, wenn der Bus fertig
  ist (Sekunden); während eines Auftrags 409.
- **Fehlertexte** sind für Benutzer geschrieben (Sprache nach `X-Lang`) und können direkt angezeigt
  werden; Schlüssel wie `reason` bei Treffern über `/api/labels` übersetzen.
- **Ausgeblendete Felder:** bei Einstellungen (MQTT) immer die ganze gespeicherte Einstellung
  zurückschicken, nicht nur die sichtbaren Felder.
- Was die heutige Oberfläche zeigt und wo, steht in [konzept-bedienung.md](konzept-bedienung.md).
