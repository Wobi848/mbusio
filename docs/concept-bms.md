# Concept: a simple, free, web-based building management system

Date: 11.11.2026 · Status: **draft, idea stage** · Working title: **bmsio**

A building management system (BMS) in the spirit of Avelon: everything in the browser, plant
graphics that look good, engineering that is fast. The difference: **free and open source (MIT)**,
self-hosted, and **simple enough that one person can set up a small site in an afternoon**.

This would be its own project and repository. mbusio stays a standalone tool and becomes the
first driver (see section 11).

---

## 1. Goals and non-goals

**Goals**

- **Everything in the browser:** operating *and* engineering. No workbench, no Windows-only
  tool, no licence dongle. A tablet in the plant room is enough.
- **Templates first:** a plant type (e.g. air handling unit with heat recovery) is engineered
  once and applied to many plants. A change to the template reaches every plant.
- **Good-looking graphics** out of the box: a symbol library and widgets, so a site looks right
  without a graphic designer.
- **One binary**, like mbusio: Windows, Linux, Raspberry Pi. No installation, no separate
  database server for a small site.
- **Free:** MIT licence, no point counts, no per-site fees.

**Non-goals (for now)**

- No controller programming (no replacement for DDC/PLC logic). The BMS reads, writes setpoints
  and switches, schedules and alarms; control loops stay in the controllers.
- No life-safety functions (fire, smoke control).
- No hosted cloud service at the start. Self-hosting comes first; a hub across several sites
  comes later (section 3).

## 2. What "simple" means here

Measured by these rules, every design decision is checked against them:

1. **The next step is always one button** (same principle as mbusio's UI).
2. **No addresses in graphics.** Graphics bind to the *meaning* of a point (tags), never to
   `bacnet://1234/AI:7`. Exchanging a controller does not touch a graphic.
3. **Defaults that work:** a discovered point gets a name, unit, tags and a trend without
   anyone touching it, where the rules can guess it.
4. **Mass work is a table, not a dialog:** point lists are edited like a spreadsheet, with
   multi-select, fill down, find and replace, CSV/Excel in and out.
5. **Nothing is lost:** every engineering change is versioned and can be undone.

## 3. Architecture

```
  Browser (operate + engineer)
        │  HTTPS + WebSocket
  ┌─────▼──────────────────────────────┐
  │ bmsio (one binary)                 │
  │  web server · API · live values    │
  │  model · templates · alarms        │
  │  scheduler · history               │
  │  drivers: BACnet, Modbus, MQTT,    │
  │  M-Bus, KNX, OPC UA, HTTP, … (§10) │
  │  built-in MQTT broker (optional)   │
  └─────┬──────────────────────────────┘
        │ field protocols
   controllers, meters, gateways
```

**Roles of the same binary:**

- **Site** (default): everything on one device in the building. SQLite for model and history.
- **Hub** (later): several sites report to one hub. The site opens the connection *outwards*
  (WebSocket or MQTT over TLS), so no VPN and no open port at the customer. The site keeps
  working and buffering when the link is down. The hub can use PostgreSQL + TimescaleDB for long
  histories across many sites.

The engineering always happens on the site's model; the hub shows and edits it remotely.

## 4. Data model

A tree of things, each thing with tags. The tag names follow **Project Haystack 4** where it
has a name for something; we do not need all of Haystack, only a documented subset.

```
site ── building ── floor ── space
  └── equip (plant, e.g. "AHU 01")
        └── equip (part, e.g. "supply fan")
              └── point
```

**Point**

| Field | Example | Note |
|---|---|---|
| `id` | `p_8f3a…` | stable, never reused |
| `name` | `AHU01 supply air temp` | shown to people |
| `tags` | `supply air temp sensor` | the meaning; templates bind to this |
| `kind` | number / bool / enum / string | |
| `unit` | °C | |
| `writable` | yes, with priority (BACnet) | setpoints, switches |
| `source` | `bacnet://dev1234/AI:7` | the only place an address lives |
| `history` | on change, 0.2 K, max 15 min | trend definition |
| `alarm` | limits, delay, class | optional |
| `aks` | `=AHU01.TE01` | optional plant designation code |

**Plant designation (AKS):** many projects already have a naming code. A configurable pattern
turns it into the tree and tags (`=AHU01.TE01` → equip `AHU01`, point type `TE` → `temp sensor`).

## 5. Templates

A template is a **package**, not just a picture:

```
templates/ahu-hr/
  template.json     # slots, parameters, version
  graphic.svg       # the plant graphic with placeholders
  defaults.json     # trends, alarms, schedules for the slots
```

**Slots** say which points the template needs, by meaning:

```json
{
  "id": "ahu-hr",
  "version": 3,
  "title": "Air handling unit with heat recovery",
  "params": { "hasCooler": { "type": "bool", "default": true } },
  "slots": {
    "supplyTemp":   { "tags": "supply air temp sensor", "required": true },
    "supplyTempSp": { "tags": "supply air temp sp", "writable": true },
    "supplyFan":    { "tags": "supply fan run cmd" },
    "coolValve":    { "tags": "cool valve cmd", "if": "hasCooler" }
  }
}
```

**Applying a template to a plant:**

1. Pick the plant (an `equip`) and the template.
2. bmsio fills the slots automatically: it searches the plant's points for the slot's tags.
3. The engineer sees a table: slot → point, with green (found), yellow (several candidates),
   red (missing). Missing optional slots hide their part of the graphic.
4. Save. The plant now has its graphic, trends and alarms.

**Changing a template:** a new template version is shown as a diff. "Apply to all 12 plants"
updates them; plants with local changes are listed and the engineer decides per plant.

**Sharing:** templates are plain files. They can be exported, imported and kept in a public
library, so the community builds up the common plant types.

## 6. Graphics

- **SVG**, not canvas: sharp at every size, styled by CSS (light and dark theme), animations
  by CSS (fan turns, pump runs, valve position).
- **Symbol library** for HVAC and electrical: fan, pump, valve, damper, coil, heat recovery,
  filter, sensor, meter. Each symbol has states as CSS classes: `off`, `on`, `fault`, `manual`,
  `stale`. A symbol knows which slots it shows (a pump: run command, feedback, fault).
- **Widgets:** value, setpoint input, switch, enum select, mini trend, alarm badge, link to
  another graphic.
- **Navigation:** site → building → floor → plant → detail, by clicking on the graphic and in
  a tree on the left.
- **Editor in the browser:** drag symbols from the library, snap to a grid, connect with pipes
  and ducts (orthogonal lines with flow direction), bind a symbol to a slot by picking from a
  list. Candidate library for the editor: **maxGraph** (successor of mxGraph from draw.io,
  Apache 2.0); otherwise an own SVG editor, as the needs are narrower than a general diagram tool.
- **Every graphic works on a phone:** the plant view gets a list fallback (the same slots as a
  table) for small screens.

## 7. Engineering tools (all in the browser)

| Tool | What it does |
|---|---|
| **Discovery** | Same for every protocol: BACnet/IP (Who-Is, object list), Modbus (device profile), M-Bus (mbusio's scan), MQTT (auto-discovery or listen and suggest), KNX (ETS import) and more (section 10). Shows what was found and what is new. |
| **Mapping rules** | Rules from object name or AKS to name, tags, unit, plant. Example: `*_TE*` → `temp sensor`. Applied on discovery and on demand; the result is shown before it is saved. |
| **Point list** | All points as a table: filter, multi-select, edit in place, fill down, find and replace. CSV/Excel import and export, layout close to a VDI 3814 function list. |
| **Templates** | Library, editor, apply to plants, slot table, version diff (section 5). |
| **Graphic editor** | Section 6. |
| **Simulator** | Every driver can run against simulated devices, so graphics, alarms and schedules can be tested at the desk (mbusio already has a simulator). |
| **Commissioning** | Go through the points of a plant one by one: value plausible? write test done? tick it off. Ends in a commissioning report (PDF), like mbusio's report. |
| **Versions** | Every save of the engineering is a version with author and comment. Diff and roll back. Export of the whole project as one file. |

## 8. Operation

- **Live view:** graphics and lists, values over WebSocket, quality shown on every value
  (good, stale, fault).
- **History:** trend per point, compare several points, export CSV.
- **Alarms:** limits, state alarms, delay, classes (A/B/C), acknowledge with comment, alarm
  history, notification by e-mail and push; later SMS through a gateway.
- **Schedules:** weekly program with exceptions and holidays. If the controller has a BACnet
  schedule, it is edited there; otherwise bmsio writes the values itself.
- **Users and rights:** roles (viewer, operator, engineer, admin), per site and building;
  every write and acknowledgement goes into an audit log.

## 9. Stack

| Layer | Choice | Why |
|---|---|---|
| Core | **Rust** (tokio, axum) | Proven in mbusio: one file, small, stable in continuous operation. |
| Storage | **SQLite** on a site; **PostgreSQL + TimescaleDB** on a hub | No database server for small sites; long histories across many sites on the hub. |
| Drivers | See section 10. Building blocks: `tokio-modbus`, `rumqttc` (client), `rumqttd` (embedded broker), `opcua`, M-Bus from mbusio; BACnet own or a C stack through FFI | BACnet/IP is required for a real BMS. The Rust crates are still thin. |
| Frontend | **TypeScript + Svelte 5** | Fine-grained updates for hundreds of live values on one graphic; built to static files and embedded in the binary. |
| Graphics | **SVG**, symbols as SVG files with CSS states, editor on maxGraph or own | Section 6. |
| Live data | WebSocket, subscription per open graphic | Only the values on screen are sent. |
| Reports | HTML → PDF in the browser (print) | No PDF library on the server, as in mbusio. |

## 10. Drivers and integrations

Goal: **every common protocol built in, and every one is connected the same way.** No driver
needs a config file; no driver needs a second program.

### 10.1 The same three steps for every protocol

1. **Connect:** pick the protocol, enter what it needs (IP, port, COM port, broker). One form.
2. **Discover:** bmsio lists what it finds, marked new / known / gone.
3. **Take over:** select points (table, multi-select). The mapping rules (section 7) propose
   name, tags, unit and plant; the engineer only corrects.

Behind this is **one driver interface** in the core. Every driver implements the same few
functions: `discover`, `subscribe` (or poll), `write`, `health`. Everything else (history,
alarms, quality, graphics, outputs) is the same for all protocols. The address of a point is a
URI per driver (`bacnet://…`, `modbus://…`, `mqtt://…`), stored only in `source`.

### 10.2 Built in

| Protocol | What makes it simple | Phase |
|---|---|---|
| **BACnet/IP** (client) | Who-Is → device list → object list with names, units, descriptions. **COV subscriptions** instead of polling. Write with priority and release. Read the controllers' schedules, trend logs and alarms (event notification). BBMD / foreign device for routed networks. | 1 (read), 4 (write, schedules, alarms) |
| **Modbus TCP / RTU** | **Device profile library** (YAML: register, type, scaling, unit, tags). Pick the device model, enter the unit ID, done. Profiles are shared like templates. Without a profile: register table to fill in, with a "read now" button per row. | 0 |
| **MQTT** | **Built-in broker** (optional), so devices can publish straight to bmsio without a separate broker; or client to an existing broker. **Auto-discovery** for common formats: Home Assistant discovery, Sparkplug B, Zigbee2MQTT, Shelly, Tasmota. For everything else: **listen and suggest** (bmsio listens for a minute, shows topics with their JSON fields and values, click a field to make it a point). Writing by command topic. | 0 |
| **M-Bus** (wired) | From mbusio, including scan, secondary addresses and the decoder. | 0 |
| **KNX IP** | Tunnelling and routing. **Import the ETS project** (`.knxproj`): group addresses with datapoint types become points with unit and kind, without typing a single address. | 4 |
| **OPC UA** (client) | Browse the server tree, pick nodes, subscriptions. | 5 |
| **HTTP / REST** | Poll a JSON URL and pick fields as with MQTT; receive webhooks. Covers inverters, chargers, weather services and many devices with a web API. | 2 |
| **LoRaWAN** | Through a network server (ChirpStack, The Things Network) over MQTT, with payload decoders from the public device repository. | 5 |
| **wireless M-Bus** | USB receiver, OMS decryption with keys per meter (shares the decoder with M-Bus). | later |
| **BACnet MS/TP** | First through a BACnet router (no own work); later directly through an RS-485 adapter. | later |

Not planned as own drivers: DALI, EnOcean, SNMP, Zigbee, … They come in through a gateway
(most of them speak MQTT, Modbus or BACnet) or as an external driver (10.3).

### 10.3 External drivers: anyone can add a protocol

A driver can also be **a separate program in any language** (Python, Node.js, Go, …). bmsio
starts it and talks to it through a small, documented JSON protocol over stdin/stdout:

```
bmsio → driver   {"op":"discover"}
driver → bmsio   {"op":"points","points":[{"addr":"ups1/battery","kind":"number","unit":"%","name":"UPS battery"}]}
bmsio → driver   {"op":"subscribe","addrs":["ups1/battery"]}
driver → bmsio   {"op":"value","addr":"ups1/battery","value":87,"ts":"2026-11-11T10:00:00Z"}
bmsio → driver   {"op":"write","addr":"ups1/test","value":true}
```

An external driver gets the same three steps in the UI as a built-in one. Writing a simple one
should take an afternoon; the documentation includes a complete example of about 50 lines.

### 10.4 Passing values on (bmsio as a server)

Other systems must be able to read from bmsio just as easily. Per point a switch, as in mbusio
([konzept-ausgaenge.md](konzept-ausgaenge.md)):

| Output | Use |
|---|---|
| **BACnet/IP server** | Points appear as BACnet objects to another BMS (e.g. M-Bus meters for a Siemens or Honeywell system). |
| **Modbus TCP server** | Register table for PLCs and older systems. |
| **MQTT publish** | Including Home Assistant discovery. |
| **REST API + WebSocket** | Own scripts, dashboards, Node-RED. |

This makes bmsio useful from day one as a **gateway**, even before it is used as the BMS.

## 11. Relation to mbusio

mbusio **stays** a standalone tool for M-Bus commissioning and service. bmsio reuses:

- the **M-Bus driver** and decoder (as a library crate split out of mbusio),
- the **data point idea** (stable number, quality, unit, how values are passed on;
  [konzept-ausgaenge.md](konzept-ausgaenge.md), [konzept-dauerbetrieb.md](konzept-dauerbetrieb.md)),
- the **simulator**, the **report** and the **UI principles** (one next step, tables for mass
  work, keyboard; [konzept-oberflaeche-neu.md](konzept-oberflaeche-neu.md)),
- the **languages** set-up ([concept-languages.md](concept-languages.md)).

Nothing is removed from mbusio for this.

## 12. Phases

| Phase | Content | Done when |
|---|---|---|
| **0 Model** | Data model, tags subset, SQLite schema, API, driver interface; Modbus (with profiles), MQTT (with broker and auto-discovery), M-Bus | Points of a test bench are read and kept with history. |
| **1 Viewer + BACnet** | BACnet/IP read with COV; live graphics from 3–5 built-in templates (AHU, heating circuit, boiler, meters), tag binding, trends | A real small plant with BACnet controllers runs on it, read-only. |
| **2 Engineering** | Discovery for all drivers, mapping rules, point list table, apply templates, versions, HTTP driver, external drivers | A new site is set up without editing a file by hand. |
| **3 Editor** | Graphic editor, symbol library, own templates, template diff | An engineer builds a new plant type in the browser. |
| **4 Operation** | Writing (BACnet with priority), alarms, schedules, users and rights, audit log; KNX with ETS import; outputs (BACnet server, Modbus server, MQTT, API) | It can run a building without a second system. |
| **5 Hub + more** | Hub across sites; OPC UA, LoRaWAN | Multi-site customer. |

BACnet/IP comes in phase 1 because it decides whether bmsio is usable in most commercial
buildings. Modbus and MQTT come first because they are quick to build and let the model and
the driver interface be tested on cheap hardware.

## 13. Open questions

- **Name** of the project and repository.
- **BACnet stack:** own Rust implementation or a C stack through FFI?
- **Editor:** maxGraph or own SVG editor? Decide after a one-week prototype of both.
- **Symbol style:** which symbol conventions do the engineers in the target market expect?
  To be reviewed by someone from the trade, like the languages.
- **First pilot site:** which building, which controllers, which protocols?
- **Licence of templates and symbols** in a shared library (MIT as well, or CC BY?).

## 14. Existing free tools, and why another one

| Tool | Strength | Why not enough here |
|---|---|---|
| **Home Assistant**, **openHAB** | Huge driver choice, free | Made for homes: no plant templates, no BMS-style point lists, alarms or commissioning. |
| **FUXA** (web SCADA, MIT) | Web-based SVG editor, Modbus/BACnet/OPC UA | General SCADA: graphics are bound to addresses, no template-to-many-plants workflow, no semantic model. Worth studying for its editor. |
| **Node-RED** | Flexible glue | A tool for integrators, not an operator interface. |

The gap bmsio fills: **semantic model + templates + engineering in the browser**, free.
