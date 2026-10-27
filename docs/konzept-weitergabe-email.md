# Konzept: Weitergabe als ein Ort für alles, was rausgeht (E-Mail, Abrechnung, Warnungen, Bericht)

Stand: 26.10.2026 · Status: **entschieden** (Abschnitt 8), Umsetzung in Schritten (Abschnitt 9)

Verwandt: [konzept-export.md](konzept-export.md) (Abrechnung, Teil C E-Mail: heutiger Stand),
[konzept-ausgaenge.md](konzept-ausgaenge.md) (MQTT, Modbus).

## 1. Warum

Teil C (E-Mail) ist an der falschen Stelle gebaut. Der Mailserver gilt für das ganze Gerät, steckt
aber im Feld „Abrechnung exportieren“ eines Projekts. Er erscheint erst, wenn man dort
„Automatisch“ setzt und eine Adresse einträgt. Eine Abrechnung von Hand kann man nicht mailen.
Der Benutzer hat die E-Mail nicht gefunden.

**Grundsatz:** „Weitergabe“ ist alles, was das Gerät verlässt: an Leitsysteme (MQTT, Modbus) und an
Menschen (E-Mail mit Abrechnung, Warnung, Bericht). Dort richtet man es ein, dort sieht man, was
rausging. Die Anlage beschreibt, *was* es gibt (Projekte, Linien, Zähler); sie verweist auf die
Weitergabe, statt sie zu enthalten.

## 2. Aufbau des Bereichs Weitergabe

```
AUSGÄNGE  [● MQTT  verbunden …] [○ Modbus  aus] [● E-Mail  bereit · zuletzt 1.10. 00:10]   Abrechnung →  Protokoll →
────────────────────────────────────────────────────────────────────────────────────────────────────
Datenpunkte (wie heute) | Details
```

- Drei **Ausgänge** als Karten in einer Zeile, gleich gebaut: Schalter, Zustand, „Einstellungen“.
- Rechts zwei **Ansichten**, die die Liste der Datenpunkte ersetzen (wie heute das Protokoll):
  **Abrechnung →** und **Protokoll →**, jeweils mit „← Weitergabe“ zurück.
- Feste Adressen (Lesezeichen, Verweise aus anderen Bereichen):
  `#/weitergabe/abrechnung`, `#/weitergabe/protokoll`, `#/weitergabe/email` (öffnet die
  Einstellungen der E-Mail).

## 3. Ausgang E-Mail

**Karte:** Schalter (aus = es geht keine Mail raus; Dateien werden trotzdem abgelegt), Zustand
`nicht eingerichtet` · `bereit` · `Fehler`, Info „smtp.example.org · zuletzt gesendet 1.10. 00:10“
bzw. der letzte Fehler.

**Einstellungen** (Formular wie MQTT, im Detailbereich):

| Feld | Vorgabe |
|---|---|
| Server, Port | –, 587 (Port folgt der Verschlüsselung, solange er ein üblicher ist) |
| Verschlüsselung | STARTTLS · TLS · keine |
| Zertifikat nicht prüfen | aus |
| Benutzer, Passwort | Passwort nur auf dem Gerät, Oberfläche zeigt „gespeichert“ |
| Absender | leer = Benutzer |
| Warnungen an | leer = keine Warnungen (Abschnitt 5) |
| Bericht an, Häufigkeit | leer / aus · täglich · wöchentlich (Abschnitt 6) |
| **Testmail an** [Feld] **Senden** | Feld vorbelegt mit „Warnungen an“, sonst Absender |

Die Empfänger der **Abrechnung** stehen beim Projekt (Abschnitt 4), weil sie je Kunde verschieden
sind. Warnungen und Bericht gehen an den, der das Gerät betreut, deshalb hier.

**Im Protokoll** (Weitergabe → Protokoll): jede Mail ist ein Eintrag (`out_log`, Ausgang
`email`): Zeit, Betreff, Empfänger, Datei, gesendet / nicht gesendet mit Grund. Filter „E-Mail“.

## 4. Abrechnung in der Weitergabe

Die heutige Ansicht zieht um, Inhalt wie in konzept-export.md 3.1–3.6, mit diesen Änderungen:

- **Projekt** oben wählbar (Vorgabe: aktives Projekt).
- Abschnitt **Versand** immer sichtbar, nicht hinter „Automatisch“:
  - [ ] Automatisch nach Ende des Zeitraums ablegen (Vormonat, Vorjahr)
  - **E-Mail an** [Feld] (gilt für automatische Dateien und „Per E-Mail senden“)
  - fehlt der Mailserver: Hinweis „Noch kein E-Mail-Server. Einrichten →“ (`#/weitergabe/email`)
- Knöpfe: **Herunterladen (CSV)** · **Per E-Mail senden** (schickt die Tabelle mit den gezeigten
  Einstellungen sofort; gesperrt mit Erklärung, wenn Empfänger oder Server fehlen).
- **Abgelegte Dateien** mit „gesendet“ / „nicht gesendet: Grund“ / „Senden“, wie heute.
- **Anlage → Projekt:** statt des Felds eine Zeile „Abrechnung: monatlich automatisch, an
  a@b.de“ bzw. „Abrechnung: von Hand“ mit **Öffnen →** (`#/weitergabe/abrechnung?projekt=<id>`).

## 5. Warnungen per E-Mail

Für Geräte, die allein laufen (Pi im Keller): mbusio meldet sich, wenn etwas nicht stimmt.

**Was eine Warnung auslöst** (aus dem, was mbusio schon erkennt):

Jede Art ist **einzeln ein- und ausschaltbar**, Wartezeiten sind **einstellbar** (Vorgaben):

| Problem | Schlüssel | gilt ab (Vorgabe, einstellbar) | Vorgabe |
|---|---|---|---|
| Zähler gestört | `disturbed:<zähler>` | Beginn in `disturbances` (3 Fehlversuche) | ein |
| Zähler meldet Fehler | `fault:<zähler>` | `meters.fault` (ohne Batterie schwach) | ein |
| Batterie schwach | `battery:<zähler>` | `meters.fault = power_low` | ein |
| Zählertausch erkannt | `swap:<zähler>` | offener Tausch | ein |
| Linie im Dauerbetrieb nicht verbunden | `line:<linie>` | seit **30 min** | ein |
| MQTT nicht verbunden | `mqtt` | seit **60 min**, nur wenn eingeschaltet | ein |
| Speicher knapp / voll | `disk` | Stufe Low/Full | ein |
| Abrechnung nicht gesendet | `billing:<projekt>` | nach dem 1. Fehlversuch | ein |

**Wie:** Alle 15 Minuten die Menge der offenen Probleme bestimmen und mit der zuletzt gemeldeten
vergleichen (`meta` `warn_state`). **Neue** Probleme → eine Mail mit allen neuen (gebündelt), dazu
in derselben Mail, was seit der letzten **behoben** ist (abschaltbar). Höchstens **eine Warnmail
pro Abstand** (Vorgabe 1 Stunde; wählbar sofort, 15 min, 1 h, 4 h, 1 Tag); was in der
Zwischenzeit dazukommt, geht in die nächste. Ein Problem, das bleibt, wird nicht wiederholt
(Vorgabe; wählbar: täglich erinnern), der Bericht nennt es weiter. Betreff: „mbusio <Gerät/Projekt>: 2 neue Probleme“.
Klappt der Versand nicht: im Protokoll, erneuter Versuch beim nächsten Durchlauf, höchstens 1 Tag.

### 5.1 Meldungen in der Oberfläche

Dieselben Warnungen erscheinen auch ohne E-Mail in der Oberfläche: Statusleiste **Meldungen 3**,
anklickbar, Liste mit Zeit, Problem und Ort (Zähler, Linie) mit Sprung dorthin, „gesehen“ zum
Wegklicken; behobene verschwinden von selbst. Gleiche Regeln (Arten an/aus, Wartezeiten), ohne
Abstand. So sieht man beim Öffnen, was in der Zwischenzeit war, auch ohne Mailserver.

## 6. Bericht per E-Mail

**Aus · täglich · wöchentlich** (Vorgabe täglich), **Uhrzeit** (Vorgabe 07:00), **Wochentag**
(Vorgabe Montag), an „Bericht an“. Jeder Teil ist **abwählbar** (Vorgabe: alle):

- Auslesung: je Projekt und Linie gelesen x von y Zählern seit dem letzten Bericht
- Weitergabe: Datenpunkte gesendet / verworfen je Ausgang
- offene Probleme (wie Abschnitt 5, auch alte)
- Abrechnungen abgelegt / gesendet
- Speicher: frei, reicht noch ca. …
- Text, keine Anhänge; verpasst (Gerät aus) → beim nächsten Start einer, nicht mehrere

## 7. Technik

- `src/mail/` bleibt (Client, Nachricht). Neu: **ein** Weg zum Senden mit Protokoll:
  `send_logged(kind, message)` schreibt `out_log` und den Zustand der Karte (`meta` `mail_last`);
  Abrechnung, Testmail, Warnung, Bericht nutzen ihn.
- Gesendet wird im Zeitplan ohne Datenbank-Sperre (wie heute).
- `MailConfig` bekommt `enabled`, `retry_days` (Vorgabe 7), `warn: WarnSettings` (Empfänger, je
  Art an/aus, Wartezeiten Linie/MQTT, Abstand, behoben melden, erinnern) und `report:
  ReportSettings` (Empfänger, Häufigkeit, Uhrzeit, Wochentag, Teile). Alles mit Vorgaben über
  `#[serde(default)]`, alte Einstellungen laden weiter.
- Weitergabe-Oberfläche: `weitergabe-ausgaenge.js` um E-Mail erweitert, Abrechnung als
  `weitergabe-abrechnung.js` (aus `anlage-abrechnung.js`), `anlage-mail.js` entfällt.

## 8. Entschieden (26.10.2026)

**Alles ist einstellbar** (Abschnitt 3, 5, 6, 10); die folgenden Punkte sind nur die Vorgaben.

1. Empfänger von Warnungen und Bericht **je Gerät** (Ausgang E-Mail); Abrechnung je Projekt.
2. Warnungen **gebündelt, höchstens 1 Mail pro Stunde**; bleibende Probleme nicht wiederholen.
3. Bericht Vorgabe **täglich 07:00** (geht nur raus, wenn „Bericht an“ ausgefüllt ist).
4. **Batterie schwach** ist eine Warnung (eigene Zeile).

## 9. Einstellungen im Überblick

Weitergabe → E-Mail → Einstellungen, drei Abschnitte untereinander (aufklappbar wie „Weitere
Werte“ bei der Abrechnung, der erste offen):

| Abschnitt | Einstellung | Vorgabe |
|---|---|---|
| **Server** | Server, Port, Verschlüsselung, Zertifikat prüfen, Benutzer, Passwort, Absender | –, 587, STARTTLS, ja, –, –, Benutzer |
| | Nicht gesendete erneut versuchen | 7 Tage |
| | Testmail an … Senden | – |
| **Warnungen** | Warnungen an | leer = keine |
| | je Art an/aus (Tabelle Abschnitt 5) | alle an |
| | Linie nicht verbunden seit | 30 min |
| | MQTT nicht verbunden seit | 60 min |
| | höchstens eine Warnmail pro | 1 Stunde |
| | Behobenes melden | ja |
| | Offene Probleme erinnern | nie |
| **Bericht** | Bericht an | leer = keiner |
| | Häufigkeit, Uhrzeit, Wochentag | täglich, 07:00, Montag |
| | Teile: Auslesung, Weitergabe, Probleme, Abrechnung, Speicher | alle |

**Betreff** je Art der Mail einstellbar, mit Platzhaltern; leer = Vorgabe; unter dem Feld eine
Vorschau („→ Abrechnung Müllerstrasse 09/2026“):

| Mail | wo | Vorgabe |
|---|---|---|
| Abrechnung | je Projekt (Abrechnung → Versand) | `Abrechnung {projekt} {zeitraum}` |
| Warnung | Ausgang E-Mail → Warnungen | `mbusio {geraet}: {anzahl} neue Probleme` |
| Bericht | Ausgang E-Mail → Bericht | `mbusio {geraet}: Bericht {datum}` |
| Testmail | fest | `mbusio: Testmail` |

Platzhalter: `{projekt}`, `{zeitraum}` (09/2026, Q3/2026, H2/2026, 2026 oder von–bis), `{von}`,
`{bis}`, `{geraet}`, `{anzahl}`, `{datum}`. **Gerätename** (Abschnitt Server, Vorgabe: Name des
Rechners) unterscheidet mehrere Geräte im Posteingang. Unbekannte Platzhalter bleiben stehen,
damit man den Tippfehler sieht. Zeilenumbrüche werden entfernt (Kopfzeile).

Bei der **Abrechnung** (je Projekt) bleibt einstellbar, was es heute gibt (Zeitraum, Form, Werte,
Linien, Dezimalzeichen, automatisch, E-Mail an), dazu neu: **warten auf Auslesung höchstens**
(Vorgabe 2 Tage, heute fest).

## 10. Umsetzung in Schritten

**Schritt 1 umgesetzt (26.10.2026):** `MailConfig.enabled`, `device`; `Db::mail_record` (jede Mail
ins `out_log` mit `name` = Betreff, Ergebnis `failed` mit Grund; `meta` `mail_last` für die
Karte); Karte und Einstellungen in `weitergabe-ausgaenge.js`, Testmail dort, `#/weitergabe/email`;
in der Abrechnung nur noch Hinweis mit Sprung. Ausgeschaltet: automatische Dateien warten (kein
Fehler an der Datei), Testmail geht. Tests: Rust `record_goes_to_log_and_card`, Browser
`20-email`.

**Schritt 2 umgesetzt (27.10.2026):** `billing/settings.rs` (Zeiträume Quartal/Halbjahr,
`label`, `years_back`, `compare_years`, `wait_days`, `mail_subject`), `billing/usage.rs`
(Verbrauch eines Zeitraums, auch für Vorjahre), `billing/series.rs` (Zeitreihe), Tabelle mit
`billing_table_for` (automatisch mit den Bezeichnungen des eingestellten Zeitraums),
`billing_subject`, `billing_mail_now`, `POST /api/projects/{id}/billing/mail`, `retry_days` am
Ausgang E-Mail. Oberfläche `weitergabe-abrechnung.js` (+ Texte), Adresse
`#/weitergabe/abrechnung?projekt=<id>`, Anlage zeigt die Zeile (Kurzfassung im Projektbaum);
`anlage-abrechnung.js` und `anlage-mail.js` entfallen; Namen der Zeiträume gemeinsam
(`period_*`). Tests: Rust `quarter_half_labels_and_years_back`, `previous_years_as_columns`,
`subject_placeholders_and_mail_now`, `auto_export_wait_days_setting`; Browser 18, 19, 20, 08.


**Schritt 3 umgesetzt (27.10.2026):** `src/warn/` (Regeln: `WarnSettings` in `MailConfig.warn`,
`WarnState` in `meta` `warn_state` mit offenen, gemeldeten und behobenen Problemen; `update`,
`mail_due`, `mail_done`), Zählerprobleme und nicht gesendete Abrechnungen aus der Datenbank
(`meter_problems`, `billing_problems`), Linie/MQTT/Speicher im Zeitplan (`scheduler/warn.rs`).
Abweichend von „alle 15 Minuten“: bei jeder Prüfung des Zeitplans (10 s), damit die Meldungen
gleich da sind; gespeichert wird nur bei Änderungen (SD-Karte). Linie und MQTT, die gerade
verbinden oder (nach dem Start) noch keinen Dienst haben, gelten als „unklar“ und behalten ihren
Zustand, sonst käme nach jedem Neustart dieselbe Warnung neu. Ein gemeldetes Problem, das kurz
weg und vor der nächsten Mail wieder da ist, gilt nicht als neu. Nur Behobenes löst keine Mail
aus, es steht in der nächsten. Fehlgeschlagen: alle 15 min neu, nach einem Tag gilt es als
gemeldet. API `GET /api/warnings`, `POST /api/warnings/seen`, `status.warnings`. Oberfläche:
`app-meldungen.js` (Statusleiste, Liste über der Statusleiste wie die Bus-Konsole), Abschnitt
„Warnungen“ in Weitergabe → E-Mail. Tests: Rust `warn::tests` (5), Zeitplan
`disturbed_meter_becomes_warning_and_mail`; Browser `22-meldungen`.

1. **Ausgang E-Mail** (Karte, Einstellungen, Testmail, Schalter, Protokoll); Mailserver aus der
   Abrechnung entfernen. Tests: Browser (Karte, Einstellungen, Testmail, Protokoll), Rust.
2. **Abrechnung in der Weitergabe** (Ansicht, Adressen, Versand sichtbar, Per E-Mail senden,
   Verweis in der Anlage).
3. **Warnungen** (Probleme bestimmen, vergleichen, bündeln).
4. **Bericht.**
