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

| Problem | Schlüssel | gilt ab |
|---|---|---|
| Zähler gestört | `disturbed:<zähler>` | Beginn in `disturbances` (3 Fehlversuche) |
| Zähler meldet Fehler | `fault:<zähler>` | `meters.fault` (ohne Batterie schwach) |
| Batterie schwach | `battery:<zähler>` | `meters.fault = power_low` |
| Zählertausch erkannt | `swap:<zähler>` | offener Tausch |
| Linie im Dauerbetrieb nicht verbunden | `line:<linie>` | seit 30 min |
| MQTT nicht verbunden | `mqtt` | seit 60 min, nur wenn eingeschaltet |
| Speicher knapp / voll | `disk` | Stufe Low/Full |
| Abrechnung nicht gesendet | `billing:<projekt>` | nach dem 1. Fehlversuch |

**Wie:** Alle 15 Minuten die Menge der offenen Probleme bestimmen und mit der zuletzt gemeldeten
vergleichen (`meta` `warn_state`). **Neue** Probleme → eine Mail mit allen neuen (gebündelt), dazu
in derselben Mail, was seit der letzten **behoben** ist. Höchstens **eine Warnmail pro Stunde**;
was in der Zwischenzeit dazukommt, geht in die nächste. Ein Problem, das bleibt, wird nicht
wiederholt (der Bericht nennt es weiter). Betreff: „mbusio <Gerät/Projekt>: 2 neue Probleme“.
Klappt der Versand nicht: im Protokoll, erneuter Versuch beim nächsten Durchlauf, höchstens 1 Tag.

## 6. Bericht per E-Mail

**Täglich** (Vorgabe 07:00) oder **wöchentlich** (Montag 07:00), an „Bericht an“:

- je Projekt und Linie: gelesen x von y Zählern seit dem letzten Bericht, Datenpunkte weitergegeben
- offene Probleme (wie Abschnitt 5, auch alte)
- Abrechnungen abgelegt / gesendet
- Speicher: frei, reicht noch ca. …
- Text, keine Anhänge; verpasst (Gerät aus) → beim nächsten Start einer, nicht mehrere

## 7. Technik

- `src/mail/` bleibt (Client, Nachricht). Neu: **ein** Weg zum Senden mit Protokoll:
  `send_logged(kind, message)` schreibt `out_log` und den Zustand der Karte (`meta` `mail_last`);
  Abrechnung, Testmail, Warnung, Bericht nutzen ihn.
- Gesendet wird im Zeitplan ohne Datenbank-Sperre (wie heute).
- `MailConfig` bekommt `enabled`, `warn_to`, `report_to`, `report` (`off`, `daily`, `weekly`);
  alte Einstellungen laden weiter (Vorgaben).
- Weitergabe-Oberfläche: `weitergabe-ausgaenge.js` um E-Mail erweitert, Abrechnung als
  `weitergabe-abrechnung.js` (aus `anlage-abrechnung.js`), `anlage-mail.js` entfällt.

## 8. Entschieden (26.10.2026)

1. Empfänger von Warnungen und Bericht **je Gerät** (Ausgang E-Mail); Abrechnung je Projekt.
2. Warnungen **gebündelt, höchstens 1 Mail pro Stunde**; bleibende Probleme nicht wiederholen.
3. Bericht Vorgabe **täglich 07:00** (geht nur raus, wenn „Bericht an“ ausgefüllt ist).
4. **Batterie schwach** ist eine Warnung (eigene Zeile).

## 9. Umsetzung in Schritten

1. **Ausgang E-Mail** (Karte, Einstellungen, Testmail, Schalter, Protokoll); Mailserver aus der
   Abrechnung entfernen. Tests: Browser (Karte, Einstellungen, Testmail, Protokoll), Rust.
2. **Abrechnung in der Weitergabe** (Ansicht, Adressen, Versand sichtbar, Per E-Mail senden,
   Verweis in der Anlage).
3. **Warnungen** (Probleme bestimmen, vergleichen, bündeln).
4. **Bericht.**
