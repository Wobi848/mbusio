# node-red-contrib-mbusio

**English** · [Deutsch](README.de.md)

Node-RED nodes for [mbusio](https://github.com/Wobi848/mbusio) (M-Bus master): pick project,
gateway, line, meter and datapoints from lists and get live meter values with quality, **without an
MQTT broker**.

## Nodes

- **mbusio-server** (configuration): address of mbusio (`http://192.168.1.20:8080`) and optional
  access key (`--token`). All nodes of the same project share one connection.
- **mbusio status**: state of a project or line for notifications: `payload` = all fine (reachable,
  nothing disturbed or invalid), `msg.mbusio` with the count per quality and the list of datapoints
  with a problem. Use a delay for alarms (a short restart of mbusio also reports `false`).
- **mbusio in**: in the editor pick one after the other: project › gateway › line › meter ›
  datapoints, or "all of this meter" / "all of this line" (including ones added later).

Output per value:

```js
msg.payload = 12345.6                 // number, factor already applied
msg.topic   = "Flat 1 · Energy"       // name of the datapoint
msg.mbusio  = { no, unit, time, quality, meter, ident, pa, gateway, line, project }
```

- Right after deploy the current value, then every new reading as set in mbusio (every reading, on
  change, daily) and every change of quality.
- Default: only good values; with "all values" also `stale`, `disturbed`, `invalid`.
- Status: green = good, yellow = stale or datapoint missing, red = disturbed or access denied,
  grey = mbusio not reachable (new attempt every 10 s).
- Read only: reading the bus stays with the plan in mbusio. Values only come from lines in
  continuous operation.

## Requirements

mbusio 0.5 or later (with `/api/nodered/…`), reachable on the LAN (`--lan`). Node-RED 4.0 or later,
Node.js 18.5 or later.

## Structure

- `lib/connection.js`: connection per project (tree, server-sent events, retry, last value per
  datapoint for nodes added later, no duplicates after reconnecting)
- `nodes/`: the nodes; the editor's pick lists are fetched by Node-RED via `POST /mbusio/tree`, so
  the key stays on the server
- `resources/mbusio-editor.js`: shared editor code (load tree, pick lists)
- Interface in mbusio: [docs/konzept-node-red.md](../docs/konzept-node-red.md) (German)

Tests: `npm test` (against a fake mbusio).

## License

MIT
