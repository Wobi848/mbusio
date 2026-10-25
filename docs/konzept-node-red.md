# Konzept: Node-RED-Integration

Stand: 25.10.2026 · Status: **Schnittstelle in mbusio umgesetzt** (2.1, 2.2), Nodes offen · betrifft mbusio ab Version 0.5

Node-RED soll Werte direkt von mbusio bekommen, **ohne MQTT-Broker**. Im Editor wählt man
Projekt → Gateway → Linie → Zähler → Datenpunkt aus Listen, und im Flow sieht man sofort, ob ein
Wert gut ist. Dazu bekommt mbusio eine kleine, nur lesende Schnittstelle, und es gibt ein Paket
**node-red-contrib-mbusio**.

---

## 1. Ziel und Abgrenzung

**Ziel:** Werte von mbusio in Node-RED einbinden, ohne Topics oder Adressen zu kennen, mit
sichtbarer Qualität und einer Nachricht, die direkt in Logik-Nodes passt (z.B. rpo-suite).

**Warum nicht über MQTT:** Das ginge heute schon mit einem `mqtt in`-Node, braucht aber einen
Broker, man muss jedes Topic abtippen, und Node-RED sieht nur Datenpunkte mit MQTT-Häkchen. Die
eigene Schnittstelle kennt alle Datenpunkte und die ganze Struktur (Projekt, Gateway, Linie,
Zähler).

**Nicht Teil davon:**
- **Kein Auslesen oder Schreiben aus Node-RED.** Der Bus gehört dem Plan von mbusio; Node-RED
  bekommt nur, was mbusio ausgelesen hat. So kann ein Flow nie den Bus stören.
- Keine Einstellungen von mbusio aus Node-RED ändern.
- **Kein Nachliefern:** War Node-RED weg, bekommt es beim Wiederverbinden den aktuellen Stand,
  nicht die Werte dazwischen. Node-RED ist Logik, kein Archiv; dafür gibt es den Verlauf in
  mbusio und den MQTT-Zwischenspeicher.
- Home Assistant (läuft über MQTT) und Modbus/BACnet sind eigene Themen.

```
 mbusio (Pi) ── HTTP im LAN ──► Node-RED
   GET  /api/nodered/tree     Auswahl im Editor (Projekt › Gateway › Linie › Zähler › Datenpunkt)
   GET  /api/nodered/stream   laufende Werte (Server-Sent Events)
```

## 2. Die Schnittstelle in mbusio

Neu unter `/api/nodered/…`, nur lesend, mit demselben Zugriffsschlüssel wie die übrige API
(Header `X-Token`, Start mit `--lan --token …`). Bestehende Endpunkte bleiben unverändert.

### 2.1 `GET /api/nodered/tree`

Alles, was der Editor zur Auswahl braucht, verschachtelt und mit aktuellem Wert:

```json
{ "version": 1, "projects": [
  { "id": 1, "name": "Musterstrasse 5", "gateways": [
    { "id": 3, "name": "Keller", "lines": [
      { "id": 7, "name": "Linie 1", "meters": [
        { "id": 12, "name": "Whg 1", "medium": "heat", "ident": "20395066", "pa": 3,
          "datapoints": [
            { "no": 1, "name": "Whg 1 · Energie", "unit": "kWh",
              "value": 12345.6, "time": "2026-10-24T18:00:00Z", "quality": "good" } ] } ] } ] } ] } ] }
```

Enthalten sind alle **aktiven Datenpunkte** (in mbusio unter „Werte wählen“ angehakt), egal ob
MQTT an ist. Kennung ist `no`: Die Nummer bleibt im Projekt für immer gleich, auch wenn Namen
sich ändern oder ein Zähler getauscht wird.

### 2.2 `GET /api/nodered/stream?project=1`

Eine offene HTTP-Verbindung (Server-Sent Events), über die mbusio Ereignisse schickt:

| Ereignis | Wann | Inhalt |
|---|---|---|
| `value` | direkt nach dem Verbinden für jeden Datenpunkt (aktueller Stand), danach bei jeder neuen Auslesung und wenn sich nur die Qualität ändert | `{ "no", "value", "unit", "time", "quality" }` |
| `changed` | Datenpunkte, Zähler oder Linien wurden in mbusio geändert | `{}`, Node-RED holt den Baum neu |
| Kommentar `: ping` | alle 30 s | hält die Verbindung offen, Node-RED merkt Abbrüche |

Optional `&line=7` oder `&meter=12`, um nur einen Teil zu bekommen.

**Warum SSE und nicht WebSocket oder Abfragen:** Die Werte fliessen nur in eine Richtung. SSE ist
normales HTTP, axum kann es von Haus aus (`axum::response::sse`), und der Client in Node-RED ist
ein paar Zeilen `fetch`. Abfragen im Takt wären verspätet und machen unnötig Last.

**Weitergabe-Regel:** Es gilt die Einstellung des Datenpunkts wie bei MQTT (bei jeder Auslesung,
nur bei Änderung, 1× täglich), damit sich Node-RED und MQTT gleich verhalten.

### 2.3 Umsetzung in mbusio (gebaut)

- Signal `Db.changes` (siehe [konzept-ausgaenge.md](konzept-ausgaenge.md) 3.2); der Strom holt
  bei jedem Signal den Stand und schickt nur, was neu ist (`src/datapoint/live.rs`).
- `src/web/nodered.rs`: die zwei Endpunkte (HTTP und SSE, keine Logik); `tree` aus vorhandenen
  Abfragen (`db.tree()`, `dp_by_meters`) zusammengesetzt.
- Werte nur aus Linien im Dauerbetrieb, wie bei MQTT; der Baum zeigt alle aktiven Datenpunkte mit
  `operation` je Linie.
- Neue Abhängigkeit nur `futures-util` (kommt ohnehin mit axum); bei tokio das Feature `sync`.
- Beim Herunterfahren enden offene Ströme (`Shared.stop`), sonst würde der Server auf sie warten.
- Unbekanntes Projekt: Fehler 404 statt leerem Strom. `time_unsure` ist (noch) nicht dabei.
- Geprüft 25.10.2026: Rust-Test (Dauerbetrieb, Weitergabe-Art, Qualität, Aufbau geändert, Baum),
  mit laufendem mbusio: Token (401), Baum, Strom mit Stand, neuer Auslesung und `changed`,
  Beenden mit offenem Strom (4 ms).

## 3. Die Nodes

### 3.1 Konfiguration `mbusio-server`

| Feld | Bedeutung |
|---|---|
| Adresse | `http://192.168.1.20:8080` |
| Zugriffsschlüssel | wie `--token`; als Credential gespeichert, nicht im Flow-Export |

Eine Verbindung je Server: Alle `mbusio-in`-Nodes teilen sich einen Stream.

### 3.2 `mbusio-in` (Werte empfangen)

**Auswahl im Editor**, Listen nacheinander, jede filtert die nächste:

1. Projekt
2. Gateway
3. Linie
4. Zähler (Symbol des Mediums, Name, ID · PA, wie in mbusio)
5. Datenpunkt: einer, mehrere, **„alle dieses Zählers“** oder **„alle dieser Linie“**
   („alle …“ schliesst Datenpunkte ein, die später dazukommen)

**Ausgang** (eine Nachricht je Wert):

```js
msg.payload  = 12345.6                      // Zahl, Faktor schon angewendet
msg.topic    = "Whg 1 · Energie"            // Name des Datenpunkts
msg.mbusio   = {
  no: 1, unit: "kWh", time: "2026-10-24T18:00:00Z", quality: "good",
  meter: "Whg 1", ident: "20395066", pa: 3,
  gateway: "Keller", line: "Linie 1", project: "Musterstrasse 5"
}
```

**Qualität** (Einstellung „Werte weitergeben“):
- *nur gute* (Vorgabe): `stale`, `disturbed`, `invalid` gehen nicht an den Ausgang
- *alle*: jede Nachricht geht raus, die Logik prüft `msg.mbusio.quality` selbst

**Status am Node:**

| Zustand | Anzeige |
|---|---|
| alle Werte gut | grüner Punkt, letzter Wert und Einheit (ein Datenpunkt) bzw. „12 Werte“ |
| mindestens ein Wert veraltet | gelber Punkt „veraltet: 2“ |
| gestört / ungültig | roter Punkt „gestört: 1“ |
| keine Verbindung zu mbusio | grauer Ring „mbusio nicht erreichbar“, Neuversuch alle 10 s |
| Zugriffsschlüssel falsch | roter Ring „Zugriff verweigert“ |

### 3.3 `mbusio-status` (Zustand der Anlage)

Sendet bei jeder Änderung: `msg.payload = true/false` (mbusio erreichbar) und
`msg.mbusio = { online, disturbed: 2, stale: 0, invalid: 0 }` für das gewählte Projekt bzw. die
Linie. Gedacht für Meldungen, z.B. in einen `rpo-alarm` („Zähler gestört“).

## 4. Technik in Node-RED

- **Editor-Listen:** Der Editor fragt nicht mbusio direkt, sondern einen Endpunkt von Node-RED
  (`RED.httpAdmin`, nur mit Editor-Rechten), der mit dem gespeicherten Schlüssel an mbusio
  weiterreicht. Der Schlüssel bleibt auf dem Server, keine CORS-Probleme.
- **Gespeichert im Node:** Projekt-ID und `no` der Datenpunkte, dazu die Namen für die Anzeige.
  Ist mbusio im Editor nicht erreichbar, zeigt der Node die gespeicherte Auswahl mit Hinweis
  „Liste nicht aktuell“.
- **Datenpunkt verschwunden** (in mbusio abgewählt): Status gelb „Datenpunkt 4 fehlt in mbusio“,
  keine stillen Lücken.
- Paket `node-red-contrib-mbusio`, Quelltext im mbusio-Repo unter `node-red/`, damit Schnittstelle
  und Nodes zusammen geändert werden. **Keine Abhängigkeiten** (`fetch` ab Node 18).
  Die Schnittstelle trägt `version: 1`; die Nodes prüfen das und melden eine zu alte mbusio-Version.
- Tests mit `node-red-node-test-helper` wie in der rpo-suite, gegen mbusio mit Simulator.
- Sprachen Deutsch und Englisch.

## 5. Umsetzung in Schritten

1. **mbusio:** Kanal, `/api/nodered/tree`, `/api/nodered/stream`, Tests.
2. **Nodes:** `mbusio-server`, `mbusio-in` mit Auswahl und Status, Tests, Beispiel-Flow.
3. **Test:** mbusio mit Simulator, Node-RED-Test-Instanz. Prüfen: Auswahl, Werte, Qualität,
   mbusio aus/an, Datenpunkt abgewählt, Deploy/Neustart, falscher Schlüssel.
4. **`mbusio-status`.**
5. Veröffentlichen auf npm **erst nach Freigabe** und nach dem Release v0.5.0 von mbusio (das wartet
   auf den 7-Tage-Test).

## 6. Offene Fragen

1. Soll das Paket öffentlich auf npm (wie die rpo-suite) oder nur für eigene Anlagen sein?
2. Mehrere Datenpunkte in einem Node: eine Nachricht je Wert (so geplant) oder auf Wunsch alle
   zusammen in einer Nachricht (`msg.payload = { "Whg 1 · Energie": 12345.6, … }`)?
3. Soll Node-RED wirklich alle aktiven Datenpunkte sehen, oder braucht es wie bei MQTT ein eigenes
   Häkchen „Node-RED“ je Datenpunkt im Reiter Weitergabe?
