# Konzept: Änderungen erkennen, gezielter scannen

Stand: 18.10.2026 · Status: **Abschnitte 2 und 3 umgesetzt**, 4 offen

## 1. Ausgangslage

| Heute | Problem |
| --- | --- |
| Jeder Scan zeigt nur sein eigenes Ergebnis. | Was sich seit dem letzten Mal geändert hat (neuer Zähler, Zähler weg, andere PA, aus einer Kollision werden zwei Zähler), muss man selbst vergleichen. |
| Der Sekundärscan sucht immer den **ganzen Bus** ab. | Bei 50 Zählern dauert das Minuten, auch wenn man nur einen Teil sucht. |
| Ein abgebrochener Sekundärscan beginnt wieder von vorn. | Zeit geht verloren. |
| Nach einem Eingriff (Alle beheben, Zählerwechsel) will man wissen, ob noch alle da sind. | Dafür gibt es nur den vollen Scan. |

**Grundsatz:** Der volle Scan bleibt die Wahrheit über den Bus und wird nicht „schlau“ abgekürzt;
ein Scan, der Teile auslässt, übersieht genau das, was man sucht. Es wird nichts automatisch in die
Zählerliste übernommen oder ausgelesen.

## 2. Änderungen seit dem letzten Scan (umgesetzt, 18.10.2026)

Nach jedem Scan vergleicht mbusio mit dem **letzten Scan derselben Art auf dieser Linie**
(Primär mit Primär, Sekundär mit Sekundär). Kostet keine Buszeit.

```
Seit dem letzten Scan (gestern 14:02):  2 neu · 1 fehlt · 1 geändert
  PA 10   10000010  KAM   OK        neu            ← vorher nur „Kollision auf PA 10“
  PA 10   10000011  KAM   OK        neu
  PA 7    30000001  SEN   OK        geändert: PA 3 → 7
  –       40000004  SEN   –         fehlt          ← grau, war beim letzten Mal da
```

- **neu:** diesmal gefunden, beim letzten Mal nicht (auch: aus einer Kollision werden einzelne
  Zähler sichtbar).
- **fehlt:** beim letzten Mal da, diesmal nicht. Steht als graue Zeile in der Trefferliste, damit
  man es nicht übersieht.
- **geändert:** gleicher Zähler (gleiche ID und Hersteller), aber andere PA, Baudrate oder anderer
  Status; angezeigt als „alt → neu“.
- Zeilen mit Änderung sind hervorgehoben (wie die Problemzeilen, eigene Farbe); die Zusammenfassung
  steht über der Trefferliste. Kein Unterschied → nur „Keine Änderung seit dem letzten Scan“.
- Erkennung eines Zählers: Sekundäradresse (ID, Hersteller, Version, Medium); beim Primärscan ohne
  ID über die PA.
- Gespeichert wird je Linie und Art das letzte Ergebnis (die Linie merkt sich heute schon ihre
  Auffälligkeiten). Der Vergleich gilt immer zum unmittelbar vorherigen Scan.

## 3. Sekundärscan eingrenzen und weitersuchen (umgesetzt, 18.10.2026)

In der Kachel Sekundärscan:

```
Sekundärscan (SA)
ID beginnt mit [1234____] (leer = ganzer Bus)          [Sekundärscan starten]
Abgebrochen bei 12** · [Weitersuchen]
```

- **Präfix der ID** (1–7 Stellen) fest vorgegeben; die Wildcard-Suche beginnt erst dahinter.
  Beispiel `1234` sucht nur `12340000` bis `12349999`.
- **Weitersuchen:** bricht man einen Sekundärscan ab, merkt sich mbusio die Stelle; „Weitersuchen“
  macht dort weiter, die bisherigen Treffer bleiben stehen.

Umgesetzt: Feld „ID beginnt mit“ beim Sekundärscan (gehört zum Bereich: Scans mit anderem Anfang
werden nicht miteinander verglichen). Beim Abbruch meldet der Scan Maske und Baudrate;
„Weitersuchen“ übernimmt die bisherigen Treffer, überspringt Baudraten und Masken davor und setzt
genau dort fort. Tests: Rust (`secondary_scan_with_prefix…`, `secondary_scan_resumes…`), UI `11-secondary`.

## 4. Schnellprüfung der bekannten Zähler

„Sind die Zähler vom letzten Scan noch alle da?“ – jeder bekannte Zähler wird über seine
Sekundäradresse direkt angesprochen, etwa 1 s pro Zähler statt eines vollen Scans.

- Gedacht nach einem Eingriff (Alle beheben, Zählerwechsel, Arbeiten am Strang).
- Ergebnis wie in 2: alle da, oder „fehlt“ bzw. „geändert“.
- Findet **keine neuen** Zähler; heisst deshalb Prüfung, nicht Scan. Für die volle Wahrheit bleibt
  der volle Scan.

## 5. Umsetzung (nach Freigabe, in dieser Reihenfolge)

| Teil | Wo | Was |
| --- | --- | --- |
| Letzten Scan merken | `src/db/` | je Linie und Art das letzte Ergebnis mit Zeit speichern |
| Vergleich | `web/js/scan.js` (oder Server) | neu, fehlt, geändert berechnen; Zeilen markieren, Zusammenfassung, „fehlt“-Zeilen |
| Präfix, Weitersuchen | `src/mbus/master/scan.rs`, `src/web/bus.rs` | `scan_secondary` mit Start-Maske; der Job liefert die Stelle des Abbruchs |
| Schnellprüfung | `src/mbus/master/scan.rs` | je bekannter SA auswählen und Kopf lesen, mit Baudrate je Zähler |
| Tests | Rust und `tests/ui` | Simulation ändern (Zähler weg, PA geändert) → Vergleich; Präfix; Weitersuchen nach Abbruch |

## 6. Entschieden

- Der Vergleich gilt auch über Neuladen und Neustart hinweg (gespeichert je Linie und Art): gerade
  beim zweiten Besuch einer Anlage ist das der Nutzen.
- Nur vollständige Scans werden verglichen und gemerkt; ein abgebrochener Scan nicht.
- Anderer Bereich (Adressen, Baudraten) als beim letzten Scan: kein Vergleich.
- „PA ändern“ und „Baudrate ändern“ führen den gemerkten Scan nach, gewollte Änderungen erscheinen
  also nicht als Änderung.
