# Konzept: schneller und gezielter scannen

Stand: 18.10.2026 · Status: **Entwurf zur Abstimmung**

## 1. Ausgangslage

| Heute | Problem |
| --- | --- |
| Der Sekundärscan sucht immer den **ganzen Bus** ab (Wildcard-Suche über die 8 Stellen der ID). | Bei 50 Zählern dauert das Minuten, auch wenn man nur 3 Zähler sucht. |
| Die IDs stehen oft **schon in der Zählerliste**, z.B. aus der importierten Liste des Kunden (Typenschilder). | Dieses Wissen nutzt der Scan nicht. |
| Jeder neue Scan **ersetzt** die Trefferliste. Ein abgebrochener Sekundärscan beginnt wieder von vorn. | Wer nach einer Korrektur nur einen Teil neu sucht, verliert den Rest. |
| Ob alle Zähler der Liste am Bus sind, sieht man nur, indem man vergleicht. | Fehlende oder unbekannte Zähler fallen spät auf. |

## 2. Ziel

Drei Erweiterungen, alle an **einer Stelle**: in der Kachel **Sekundärscan** im Reiter Scan. Keine
neue Seite, kein neuer Reiter.

```
Sekundärscan (SA)
Suchen in:  (•) ganzer Bus   ( ) nur Zählerliste (30 IDs)   ( ) ID beginnt mit [1234____]
                                                            [Sekundärscan starten]
```

### 2.1 Gezielt: nur die Zähler der Liste

- Für jeden Zähler der Zählerliste mit ID: **direkt ansprechen** (Auswahl mit seiner ID, die
  übrigen Stellen als Wildcard), dann den Kopf der Antwort lesen. Mit seiner eigenen Baudrate,
  falls bei ihm eingetragen, sonst mit der Baudrate der Linie.
- Etwa **1 s pro Zähler** statt Minuten für den ganzen Bus.
- Ergebnis je Zähler, in der gewohnten Trefferliste:
  - **gefunden**, mit PA, Baudrate, Hersteller, Medium (wie heute),
  - **antwortet nicht**: neuer Status, als Auffälligkeit gezählt („3 Zähler der Liste antworten
    nicht“), mit „Warum?“ (Verkabelung, Baudrate, Zähler gewechselt),
  - **mehrere antworten** auf dieselbe ID (gleiche ID bei verschiedenen Herstellern): nur für
    diese ID wird mit Wildcards weitergesucht, wie heute beim Sekundärscan.
- Zusammenfassung oben: „28 von 30 Zählern der Liste gefunden“.

### 2.2 Eingegrenzt: ID beginnt mit …

- Ein Präfix der ID (1–7 Stellen) wird fest vorgegeben; die Wildcard-Suche beginnt erst hinter
  dem Präfix. Beispiel: Präfix `1234` sucht nur die IDs `12340000` bis `12349999`.
- Typisch, wenn ein Lieferant fortlaufende IDs hat oder nur ein Gebäudeteil gesucht wird.
- Technisch fast geschenkt: die Suche beginnt einfach an der Stelle hinter dem Präfix.

### 2.3 Ergebnisse behalten

- Ein Sekundärscan **ergänzt** die Trefferliste, statt sie zu ersetzen. Zusammengeführt wird über
  die volle Sekundäradresse; ein erneut gefundener Zähler bekommt den neuen Stand.
- Treffer aus einem früheren Scan, die diesmal nicht geprüft wurden, bleiben stehen, grau mit
  dem Hinweis „früherer Scan“. Ein Knopf **„Trefferliste leeren“** fängt neu an.
- Der Primärscan bleibt wie heute: er prüft einen festen Bereich und ersetzt die Liste.
- **Weitersuchen:** wird ein Sekundärscan über den ganzen Bus abgebrochen, merkt sich mbusio die
  Stelle. „Weitersuchen“ macht dort weiter, statt bei `0` zu beginnen.

### 2.4 Zählerliste und Bus vergleichen

Nach jedem Scan, unter der Trefferliste, nur wenn es etwas zu sagen gibt:

`Zählerliste ↔ Bus: 2 fehlen am Bus (12345678, 23456789) · 3 neu, nicht in der Liste [Neue übernehmen]`

- **Fehlen am Bus** = in der Liste, aber in diesem Scan nicht gefunden.
- **Neu** = gefunden, aber nicht in der Liste; „Neue übernehmen“ merkt nur diese.
- Nach einem gezielten Scan (2.1) gibt es naturgemäss keine neuen Zähler, nur fehlende.

## 3. Nächster Schritt

- Steht die Zählerliste schon (z.B. aus einer importierten Liste), aber die Zähler sind noch nicht
  am Bus bestätigt, schlägt „Nächster Schritt“ **„Zähler der Liste suchen“** vor (gezielter Scan)
  statt eines Primärscans über alle Adressen.
- Fehlen danach Zähler, zeigt die Leiste „2 Zähler der Liste antworten nicht“ als Auffälligkeit.

## 4. Umsetzung

| Teil | Wo | Was |
| --- | --- | --- |
| Gezielt | `src/mbus/master/scan.rs` | `scan_known(ids, progress)`: je ID auswählen, Kopf lesen, bei mehreren Antworten mit Wildcards nur unter dieser ID weitersuchen; Baudrate je Zähler über `at_baud` |
| Präfix, Weitersuchen | `src/mbus/master/scan.rs` | `scan_secondary` bekommt eine Start-Maske (Präfix fest bzw. Stelle des Abbruchs) |
| API | `src/web/bus.rs` | `POST /api/scan/secondary` mit `{ scope: "bus" \| "list" \| "prefix", prefix, resume }`; der Job liefert die Stelle des Abbruchs |
| Zusammenführen, Vergleich | `web/js/scan.js` | Treffer über die SA zusammenführen, „früherer Scan“, Vergleich mit der Zählerliste |
| Simulator | `src/sim/` | kann es schon (Auswahl mit Wildcards); Vorlage „Mehrfamilienhaus“ mit 30 Zählern zum Testen |
| Tests | Rust und `tests/ui` | gezielt: 30 von 30 gefunden, ein fehlender Zähler, gleiche ID zweimal; Präfix; Weitersuchen nach Abbruch |

Kein neuer Speicherort: die Zählerliste ist die Quelle der IDs, die Trefferliste bleibt, wo sie ist.

## 5. Offene Fragen

1. Soll ein **gefundener Zähler der Liste** gleich seine PA und Baudrate in die Zählerliste
   übernehmen (dann kann „Merken über PA“ direkt auslesen), oder nur anzeigen?
2. Gilt „Ergebnisse behalten“ (2.3) auch für den **Primärscan**, oder bleibt der wie heute?
