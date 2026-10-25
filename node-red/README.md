# node-red-contrib-mbusio

Node-RED-Nodes für [mbusio](https://github.com/Wobi848/mbusio) (M-Bus-Master): Projekt, Gateway,
Linie, Zähler und Datenpunkte aus Listen wählen und die laufenden Zählerwerte mit Qualität
bekommen, **ohne MQTT-Broker**.

*English below.*

## Nodes

- **mbusio-server** (Konfiguration): Adresse von mbusio (`http://192.168.1.20:8080`) und
  optionaler Zugriffsschlüssel (`--token`). Alle Nodes desselben Projekts teilen sich eine
  Verbindung.
- **mbusio in**: Auswahl im Editor nacheinander: Projekt › Gateway › Linie › Zähler ›
  Datenpunkte, oder „alle dieses Zählers“ / „alle dieser Linie“ (auch später dazukommende).

Ausgang je Wert:

```js
msg.payload = 12345.6                 // Zahl, Faktor schon angewendet
msg.topic   = "Whg 1 · Energie"       // Name des Datenpunkts
msg.mbusio  = { no, unit, time, quality, meter, ident, pa, gateway, line, project }
```

- Nach dem Deploy sofort der aktuelle Wert, danach jede neue Auslesung wie in mbusio eingestellt
  (jede Auslesung, bei Änderung, täglich) und jede geänderte Qualität.
- Vorgabe: nur gute Werte; mit „alle Werte“ auch `stale`, `disturbed`, `invalid`.
- Status: grün = gut, gelb = veraltet oder Datenpunkt fehlt, rot = gestört oder Zugriff
  verweigert, grau = mbusio nicht erreichbar (neuer Versuch alle 10 s).
- Nur lesend: Das Auslesen des Busses bleibt beim Plan in mbusio. Werte kommen nur aus Linien im
  Dauerbetrieb.

## Voraussetzungen

mbusio ab der Version mit `/api/nodered/…` (siehe CHANGELOG von mbusio), im LAN erreichbar
(`--lan`). Node-RED ab 4.0, Node.js ab 18.5.

## Aufbau

- `lib/connection.js`: Verbindung je Projekt (Baum, Server-Sent Events, Neuversuch, letzter Wert
  je Datenpunkt für später dazukommende Nodes, keine Doppelten nach Neuverbindung)
- `nodes/`: die zwei Nodes; die Auswahlliste im Editor holt Node-RED über `POST /mbusio/tree`,
  damit der Schlüssel auf dem Server bleibt
- Schnittstelle in mbusio: [docs/konzept-node-red.md](../docs/konzept-node-red.md)

Tests: `npm test` (gegen ein nachgebautes mbusio).

## English

Node-RED nodes for mbusio: pick project, gateway, line, meter and datapoints from lists and get
live meter values with quality, without an MQTT broker. `mbusio-server` holds address and
optional access key; `mbusio in` sends one message per value (`payload` number, `topic` name,
`msg.mbusio` with unit, time, quality, meter, line …). Only good values by default. Read-only.
