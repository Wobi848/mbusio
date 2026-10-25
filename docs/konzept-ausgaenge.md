# Konzept: Ausgänge (Schnittstellen nach aussen)

Stand: 25.10.2026 · Status: **Entwurf, nicht gebaut** · Grundlage für Home Assistant, Node-RED,
Modbus TCP und BACnet/IP

mbusio soll mehrere Schnittstellen bekommen. Dieses Dokument legt fest, **was alle gemeinsam
haben und was jede nur für sich hat**, damit jede neue Schnittstelle nur ihren Protokoll-Teil
mitbringt und der Code nicht mit jeder Schnittstelle unübersichtlicher wird.

Verwandt: [konzept-dauerbetrieb.md](konzept-dauerbetrieb.md) (Datenpunkte, MQTT, Phasen),
[konzept-node-red.md](konzept-node-red.md).

---

## 1. Heutiger Stand

**Gut und bleibt:**

- **Datenpunkt** als gemeinsame Grundlage: stabile Nummer `no` je Projekt, stabile Kennung aus der
  Bedeutung des Werts, Faktor, Einheit, Qualität (good / stale / disturbed / invalid),
  Weitergabe-Art (jede Auslesung / bei Änderung / täglich).
- **Je Datenpunkt und Ausgang** ein/aus und eine stabile Adresse (Tabelle `dp_outputs`), mit
  Vorschlag beim Einschalten, „Alle nach Muster setzen“, CSV-Export/Import, Liste für den Integrator.
- **Eine Stelle für neue Werte:** `Db::dispatch` (nach jeder Auslesung) und `Db::outputs_refresh`
  (geänderte Qualität, gerade eingeschaltet). Die Ausgänge halten das Auslesen nie auf.
- **MQTT** mit Warteschlange in der Datenbank, eigener Thread, Last Will.

**Was bei weiteren Schnittstellen zum Problem würde:**

| Stelle | Heute | Mit 4 Ausgängen |
|---|---|---|
| `src/datapoint/outputs.rs` | 7 `match key { "mqtt" => … }` (Name, Adressname, Adresse prüfen, Vorschlag, eingeschaltet, senden, Listen-Basis) | 28 Zweige, über die Datei verteilt |
| `Db.mqtt_enabled` | eigenes Feld in `Db` | ein Feld je Ausgang |
| `src/web/outputs.rs` | `if key == "mqtt"` für die Integrator-Liste | je Ausgang ein `if` |
| `src/web/bus.rs` | `"mqtt": mqtt_status(…)` in `/api/status` | je Ausgang ein Feld |
| `web/js/transfer.js` | Zustand, Formular, Speichern nur für MQTT (`key !== "mqtt"`) | je Ausgang Sonderfälle |

Ausserdem passt das heutige Modell nur auf **Ausgänge, die senden** (MQTT). Modbus und BACnet sind
**Server**: Das Leitsystem fragt, mbusio antwortet mit dem aktuellen Wert. „Zuletzt gesendet“ und
„1× täglich“ gibt es dort nicht.

## 2. Drei Arten von Ausgängen

| Art | Beispiele | Adresse je Datenpunkt | Was passiert bei einem neuen Wert |
|---|---|---|---|
| **Senden** | MQTT (+ Home Assistant) | Topic | nach Weitergabe-Art in die Warteschlange |
| **Bereitstellen** | Modbus TCP, BACnet/IP | Register / Objektnummer | nichts zu senden; der Server liest den aktuellen Wert; BACnet meldet Änderungen (COV) |
| **Strom** | Node-RED (SSE) | keine, Kennung ist `no` | an verbundene Empfänger, wenn welche da sind |

Gemeinsam für alle: Datenpunkt, Nummer, aktueller Wert, Qualität, Zeit, Ort (Projekt › Gateway ›
Linie › Zähler). Das liefert mbusio an **einer** Stelle.

**Home Assistant ist kein eigener Ausgang**, sondern eine Option von MQTT: Zu jedem Datenpunkt mit
MQTT-Häkchen schickt mbusio zusätzlich eine Konfig-Nachricht (MQTT Discovery). Eigenes Konzept,
siehe 6.

## 3. Zielaufbau

### 3.1 Ein Ausgang = ein Modul, das eine Schnittstelle erfüllt

```rust
/// Was jeder Ausgang können muss; alles Gemeinsame steht ausserhalb
pub trait Output: Sync {
    fn key(&self) -> &'static str;                 // "mqtt", "modbus" … (gespeichert, nie umbenennen)
    fn label(&self) -> String;                     // "MQTT"
    fn address_label(&self) -> String;             // "Topic", "Register"
    fn check_address(&self, a: &str) -> Result<()>;
    fn suggest_address(&self, db: &Db, p: &Place, d: &Datapoint) -> Result<String>;
    fn is_on(&self, db: &Db) -> bool;              // als Ganzes eingeschaltet
    /// nur Art „Senden“: Wert zustellen (z.B. in die Warteschlange); sonst nichts tun
    fn deliver(&self, db: &mut Db, d: &Datapoint, address: &str, s: &Sample) -> Result<()>;
    fn list_info(&self, db: &Db) -> Option<Value>; // Zusatz für die Integrator-Liste
    fn status(&self, st: &AppState) -> Value;      // für /api/status und die Oberfläche
}

/// Alle Ausgänge, in der Reihenfolge der Oberfläche
pub fn outputs() -> &'static [&'static dyn Output] { &[&mqtt::MQTT] }
```

- Der gemeinsame Teil (`dispatch`, `outputs_refresh`, Adressvergabe, „Alle nach Muster setzen“,
  CSV, Integrator-Liste) ruft nur noch die Schnittstelle auf und kennt keinen Ausgang beim Namen.
- `Place` (Projekt, Gateway, Linie, Zähler) ersetzt das Tupel aus `dp_place` und wird von allen
  Ausgängen benutzt.
- `Db.mqtt_enabled` wird zu einem Satz „eingeschaltete Ausgänge“, gefüllt über `is_on`.
- Jeder Ausgang lebt in seinem Ordner (`src/mqtt/`, später `src/modbus/`, `src/bacnet/`,
  `src/web/nodered.rs`) mit eigenem Thread oder Server, wie MQTT heute.

### 3.2 Ein Ereignis für neue Werte

`dispatch` und `outputs_refresh` legen jeden neuen Wert bzw. jede geänderte Qualität zusätzlich als
`ValueEvent { project, no, value, quality, utc, unsure }` auf einen `tokio::sync::broadcast`-Kanal.
Wer zuhört, bekommt es (Node-RED-Strom, BACnet-COV); hört niemand zu, kostet es nichts. Wer zu
langsam ist, verpasst Ereignisse und holt den aktuellen Stand neu. Das Auslesen wartet nie.

Den **aktuellen Wert** haben alle schon in der Datenbank (`datapoints.last_value`, `invalid`,
`last_utc`); Server-Ausgänge lesen dort und brauchen keine eigene Kopie.

### 3.3 Oberfläche

`transfer.js` behandelt Ausgänge nur noch allgemein (Liste, Zustand, Häkchen, Adresse). Was
ein Ausgang an Einstellungen hat, steht je Ausgang in einem kleinen Block
`OUTPUT_UI = { mqtt: { fields, load, save }, … }`. Neue Ausgänge fügen dort einen Eintrag hinzu
und ändern sonst nichts.

### 3.4 Kompatibilität

- Gespeicherte Daten (`dp_outputs`, `meta.mqtt`), API-Endpunkte, CSV-Spalten und das Feld `mqtt`
  in `/api/status` bleiben gleich. Neu ist höchstens zusätzlich `outputs` in `/api/status`.
- Der Umbau ändert **kein Verhalten**; die vorhandenen Tests (heute 116) müssen unverändert grün
  bleiben, dazu die UI-Tests mit dem Test-Broker.

## 4. Wie die geplanten Schnittstellen hineinpassen

| Schnittstelle | Art | Was sie mitbringt | Was sie vom gemeinsamen Teil nimmt |
|---|---|---|---|
| Home Assistant | Option von MQTT | Konfig-Nachrichten, Zuordnung Grösse → `device_class` | Topics, Ort, Einheit, Qualität |
| Node-RED | Strom | `/api/nodered/tree`, `/api/nodered/stream` | Ereignis-Kanal, Ort, aktueller Wert |
| Modbus TCP | Bereitstellen | Server, Register-Aufbau, Adressvorschlag | Häkchen, Adressen, aktueller Wert, Qualität |
| BACnet/IP | Bereitstellen | Server, Objekte, Gerätesuche, COV | wie Modbus, dazu Ereignis-Kanal für COV |

Node-RED braucht kein Häkchen je Datenpunkt (alle aktiven), es sei denn, die offene Frage in
[konzept-node-red.md](konzept-node-red.md) wird anders entschieden. Dann wäre es ein normaler
Ausgang mit leerer Adresse.

## 5. Umsetzung in Schritten

1. **Umbau ohne neue Funktion:** Schnittstelle `Output`, `Place`, MQTT als erste Umsetzung,
   `mqtt_enabled` ersetzt, `web/outputs.rs` und `bus.rs` allgemein, `transfer.js` mit `OUTPUT_UI`.
   Prüfen: alle Rust- und UI-Tests grün, MQTT gegen den Test-Broker unverändert.
2. **Ereignis-Kanal** (3.2) mit Test.
3. **Home Assistant Discovery** (eigenes kurzes Konzept, siehe 6).
4. **Node-RED** nach [konzept-node-red.md](konzept-node-red.md).
5. **Modbus TCP, BACnet/IP** erst nach Freigabe (Regel: Modbus vorerst nicht) und nach dem
   7-Tage-Test.

Jeder Schritt einzeln committen; keiner ändert gespeicherte Daten.

## 6. Ausblick: Home Assistant Discovery (Stichpunkte für das eigene Konzept)

- Option in den MQTT-Einstellungen „Home Assistant“ mit Präfix (Vorgabe `homeassistant`).
- Je Datenpunkt mit MQTT-Häkchen eine retained Konfig-Nachricht
  `homeassistant/sensor/<geraet>/<no>/config`: `state_topic` = Topic des Datenpunkts,
  `value_template` `{{ value_json.value }}`, Einheit, `device_class` und `state_class` aus der
  Grösse (Energie, Volumen, Leistung, Durchfluss, Temperatur; Zählerstände `total_increasing`),
  `availability_topic` = `<basis>/status`, Qualität als Attribut.
- **Gerät = Zähler** (Hersteller, Modell, ID), damit HA die Werte eines Zählers zusammenfasst.
- Stabile `unique_id` aus Client-ID, Projekt und `no`; Häkchen weg → leere Konfig-Nachricht
  (HA entfernt den Sensor).
- Offene Frage: wie mit den heute von Hand angelegten Sensoren umgehen (doppelt vermeiden).
