# Konzept: Modbus TCP (Server)

Stand: 25.10.2026 · Status: **in Arbeit** · Phase 4 aus [konzept-dauerbetrieb.md](konzept-dauerbetrieb.md),
Ausgang der Art „Bereitstellen“ nach [konzept-ausgaenge.md](konzept-ausgaenge.md)

mbusio wird ein **Modbus-TCP-Server**: Leitsystem, SPS oder Loxone lesen die Zählerwerte als
Register, ohne Broker und ohne eigene Programmierung. Modbus spricht praktisch jedes System.

## 1. Was ein Leitsystem sieht

- **Nur lesen:** Funktionen 04 (Input Registers) und 03 (Holding Registers) liefern **dieselben**
  Register (manche Systeme können nur eine von beiden). Schreiben gibt es nicht (Fehler 01).
- **Unit-ID:** jede wird beantwortet (mbusio ist ein Gerät, kein Gateway auf weitere Geräte).
- **Lücken:** Register ohne Datenpunkt liefern 0 statt eines Fehlers, damit ein Leitsystem
  grosse Bereiche am Stück lesen kann. Fehler 02 nur ausserhalb 0–65535, Fehler 03 bei mehr als
  125 Registern in einer Anfrage.

### 1.1 Registerblock je Datenpunkt (10 Register)

Startadresse = **(Nummer des Datenpunkts − 1) × 10**, z.B. Datenpunkt 1 → 0, Datenpunkt 7 → 60.
Die Nummer `no` bleibt im Projekt für immer gleich, also auch die Adresse; neue Datenpunkte
verschieben nichts, gelöschte hinterlassen eine Lücke. Eigene Adressen sind möglich (Vielfache
von 10, unter 60000).

| Register | Inhalt | Typ |
| --- | --- | --- |
| +0 … +3 | Wert, genau (auch grosse Zählerstände in Wh) | float64 |
| +4 … +5 | Wert | float32 |
| +6 … +7 | Wert × 100, gerundet, begrenzt auf den Bereich | int32 (mit Vorzeichen) |
| +8 | Qualität: 0 gut, 1 veraltet, 2 gestört, 3 ungültig, 4 kein Wert | uint16 |
| +9 | Alter des Werts in Minuten (bis 65535) | uint16 |

Das Leitsystem nimmt den Typ, den es kann (float64 ist bei grossen Zählerständen genauer als
float32 mit nur 7 Stellen; int32 für SPS ohne Gleitkomma). Faktor und Einheit gelten wie beim
Datenpunkt eingestellt.

**Wortreihenfolge** (Einstellung): Standard „gross zuerst“ (ABCD, wie Modbus selbst); „Worte
getauscht“ (CDAB) für Systeme, die das erwarten. Innerhalb eines Registers immer gross zuerst.

### 1.2 Zustand von mbusio (ab Register 60000)

| Register | Inhalt |
| --- | --- |
| 60000 | Lebenszeichen: Minuten seit dem Start von mbusio (läuft über) |
| 60001 | Anzahl Datenpunkte gestört oder ungültig |
| 60002 | Anzahl Datenpunkte veraltet |

## 2. Welche Werte

- Wie bei MQTT: je Datenpunkt das Häkchen **Modbus** im Reiter Weitergabe, mit Adresse
  (vorgeschlagen, stabil, überschreibbar). „Alle nach Muster setzen“ setzt die Adressen nach der
  Nummer.
- Werte nur aus **Linien im Dauerbetrieb**; sonst Qualität 4 „kein Wert“ (der Block existiert,
  damit die Registertabelle vollständig ist).
- **Ein Projekt je Server:** Die Register gehören einem Projekt (Einstellung, Vorgabe das aktive),
  denn die Nummern der Datenpunkte zählen je Projekt. Adressen müssen darum nur im Projekt
  eindeutig sein (bei MQTT dagegen auf dem ganzen Broker).
- Die Weitergabe-Art (jede Auslesung, bei Änderung, täglich) gilt für Modbus nicht: Ein
  Leitsystem liest immer den aktuellen Stand.

## 3. Einstellungen

Reiter Weitergabe → Ausgänge → Modbus → Einstellungen:

| Feld | Vorgabe | Bemerkung |
| --- | --- | --- |
| Modbus einschalten | aus | |
| Adresse (Netzwerkschnittstelle) | `0.0.0.0` (alle) | Modbus hat keine Zugangskontrolle: wenn möglich die IP des Anlagennetzes |
| Port | 502 | unter Linux braucht 502 Rechte (Dienst als root oder `setcap`), sonst z.B. 1502 |
| Projekt | aktives | |
| Wortreihenfolge | ABCD | oder CDAB |

Gespeichert in `meta.modbus`. Zustand (hört auf …, Anfragen, verbundene Leitsysteme, Fehler wie
„Port belegt“) steht beim Ausgang und in der Statusleiste.

## 4. Technik

- Eigener kleiner Server (`src/modbus/`, wie der eigene MQTT-Client): MBAP-Kopf, Funktionen 03 und
  04, Fehlerantworten. Keine neue Bibliothek.
- **Registerbild im Speicher:** Bei jedem Änderungssignal (`Db.changes`) und einmal pro Minute
  (Alter, veraltet) neu aufgebaut; Anfragen werden aus dem Bild beantwortet, ohne Datenbank. So
  hält auch ein Leitsystem, das jede Sekunde liest, mbusio nicht auf.
- Anbindung an die Datenpunkte: `src/datapoint/outputs/modbus.rs` (Adresse prüfen und
  vorschlagen, Liste für den Integrator); `deliver` tut nichts (Server liest den Stand).
- Die Schnittstelle `Output` bekommt dafür eine Angabe, ob Adressen global (MQTT) oder je Projekt
  (Modbus) eindeutig sein müssen.

## 5. Liste für den Integrator

Wie bei MQTT als CSV und JSON (Reiter Weitergabe): je Datenpunkt Startregister, Name, Einheit,
Zähler, Ort; im JSON dazu der Aufbau des Blocks, Wortreihenfolge, Funktionen, Port und die
Register ab 60000.

## 6. Nicht in dieser Phase

- Uhrzeit vom Leitsystem über ein Register (Konzept Dauerbetrieb, Uhrzeit): später, wenn gewünscht.
- Modbus RTU (seriell) und Schreiben von Werten.

## 7. Prüfen

- Rust-Tests: Kodierung (float64/float32/int32, beide Wortreihenfolgen), Anfragen und
  Fehlerantworten, Registerbild (Qualität, Dauerbetrieb, Lücken, Zustandsregister), Server über
  TCP.
- UI-Test: Ausgang einrichten, Häkchen, Adresse, Liste.
- Echter Test mit einem Modbus-Client gegen mbusio mit Simulator.
