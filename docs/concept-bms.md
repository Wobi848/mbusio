# Concept: a simple, free, web-based building management system

Date: 11.11.2026 · Status: **draft, idea stage** · Working title: **bmsio**

A building management system (BMS; in German *Gebäudeleittechnik*, GLT; in the US often
*building automation system*, BAS) in the spirit of Avelon: everything in the browser, plant
graphics that look good, engineering that is fast. The difference: **free and open source (MIT)**,
self-hosted, **open to plugins from the community**, and **simple enough that one person can set
up a small site in an afternoon**.

This would be its own project and repository. mbusio stays a standalone tool and becomes the
first driver (see section 13).

---

## 1. Goals and non-goals

**Goals**

- **Everything in the browser:** operating *and* engineering. No workbench, no Windows-only
  tool, no licence dongle. A tablet in the plant room is enough.
- **Templates first:** a plant type (e.g. air handling unit with heat recovery) is engineered
  once and applied to many plants. A change to the template reaches every plant.
- **Good-looking graphics** out of the box: a symbol library and widgets, so a site looks right
  without a graphic designer.
- **One binary, Linux only** (x86-64 and ARM64, so also the Raspberry Pi). No separate database
  server for a small site. The *server* runs on Linux; the *users* work with any browser on any
  system, Windows included.
- **100% open source:** one edition with every feature, no "enterprise" part, no point counts,
  no per-site fees, no telemetry, no account needed (section 17).
- **MQTT first:** every point of every protocol is in one MQTT tree, readable and writable with
  rights; the UI itself uses it (section 11).
- **Extensible by the community:** drivers, device profiles, templates, symbols, widgets and
  apps as plugins (section 12).

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
  Browser (operate + engineer)          Node-RED, Home Assistant,
        │  HTTPS (REST)                  Grafana, scripts, plugins
        │  MQTT over WebSocket (live)            │ MQTT
  ┌─────▼────────────────────────────────────────▼──┐
  │ bmsio (one binary)                              │
  │  MQTT broker + topic tree (§11)                 │
  │  web server · REST API                          │
  │  model · templates · alarms · scheduler         │
  │  history                                        │
  │  drivers: BACnet, Modbus, MQTT, M-Bus, KNX,     │
  │           OPC UA, HTTP, … (§10)                 │
  └─────┬───────────────────────────────────────────┘
        │ field protocols
   controllers, meters, gateways
```

**Roles of the same binary:**

- **Site** (default): everything on one device in the building. SQLite for model and history.
- **Hub** (later): several sites report to one hub. The site opens the connection *outwards*
  (an MQTT bridge over TLS, section 11), so no VPN and no open port at the customer. The site keeps
  working and buffering when the link is down. The hub can use PostgreSQL + TimescaleDB for long
  histories across many sites.

The engineering always happens on the site's model; the hub shows and edits it remotely.

**Linux only.** Supporting one operating system keeps the project small and lets it use what
Linux offers directly:

| Delivery | For |
|---|---|
| **Static binary** (musl) for x86-64 and ARM64 | Any distribution, copy and start. |
| **`.deb` package** with a **systemd** service | Debian, Ubuntu, Raspberry Pi OS: `apt install`, starts on boot, restarts on failure, logs in the journal. |
| **Container image** | Docker / Podman, also on a NAS. |
| **Ready SD card image** for the Raspberry Pi (later) | Flash, plug in, open the browser. |

Updates through the package manager or the container; the binary does not update itself.
Serial ports are `/dev/ttyUSB*` and `/dev/serial/by-id/*` (stable names for USB adapters).
Plugins run as separate processes, so Linux tools for limiting them (separate user, systemd
sandboxing, cgroups) are available (section 12).

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
| **EDE import** | Import BACnet point lists in the EDE format (*Engineering Data Exchange*, a CSV layout many vendor tools export). Points, names, units and descriptions arrive before the first scan, and the mapping rules tag them. |
| **Point list** | All points as a table: filter, multi-select, edit in place, fill down, find and replace. CSV/Excel import and export, layout close to a VDI 3814 function list. |
| **Templates** | Library, editor, apply to plants, slot table, version diff (section 5). |
| **Graphic editor** | Section 6. |
| **Simulator** | Every driver can run against simulated devices, so graphics, alarms and schedules can be tested at the desk (mbusio already has a simulator). |
| **Commissioning** | Go through the points of a plant one by one: value plausible? write test done? tick it off. Ends in a commissioning report (PDF), like mbusio's report. |
| **Versions** | Every save of the engineering is a version with author and comment. Diff and roll back. Export of the whole project as one file. |

## 8. Operation

- **Live view:** graphics and lists, values over MQTT over WebSocket, quality shown on every value
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
| Platform | **Linux** only, x86-64 and ARM64 | One system to test and support (section 3). |
| Core | **Rust** (tokio, axum) | Proven in mbusio: one file, small, stable in continuous operation. |
| Plugins | Separate processes (any language), JSON over stdin/stdout or MQTT; data packages as files; widgets as web components in a sandbox | Section 12. |
| Storage | **SQLite** on a site; **PostgreSQL + TimescaleDB** on a hub | No database server for small sites; long histories across many sites on the hub. |
| Drivers | See section 10. Building blocks: `tokio-modbus`, `rumqttc` (client), `rumqttd` (embedded broker), `opcua`, M-Bus from mbusio; BACnet own or a C stack through FFI | BACnet/IP is required for a real BMS. The Rust crates are still thin. |
| Frontend | **TypeScript + Svelte 5** | Fine-grained updates for hundreds of live values on one graphic; built to static files and embedded in the binary. |
| Graphics | **SVG**, symbols as SVG files with CSS states, editor on maxGraph or own | Section 6. |
| Live data | **MQTT over WebSocket** from the built-in broker, subscription per open graphic | One live data path for UI and integrations; only the values on screen are sent (section 11). |
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
| **BACnet/IP** (client) | Who-Is → device list → object list with names, units, descriptions. **COV subscriptions** instead of polling. Write with priority and release. Read the controllers' schedules, trend logs and alarms (event notification). BBMD / foreign device for routed networks. EDE file import. Later **BACnet/SC** (secure connect, TLS), which newer controllers such as LOYTEC's already offer. | 0 (read), 4 (write, schedules, alarms) |
| **Modbus TCP / RTU** | **Device profile library** (YAML: register, type, scaling, unit, tags). Pick the device model, enter the unit ID, done. Profiles are shared like templates. Without a profile: register table to fill in, with a "read now" button per row. | 1 |
| **MQTT** | **Built-in broker** (optional), so devices can publish straight to bmsio without a separate broker; or client to an existing broker. **Auto-discovery** for common formats: Home Assistant discovery, Sparkplug B, Zigbee2MQTT, Shelly, Tasmota. For everything else: **listen and suggest** (bmsio listens for a minute, shows topics with their JSON fields and values, click a field to make it a point). Writing by command topic. (This is MQTT *into* bmsio from other devices; the built-in broker and topic tree are section 11.) | 1 |
| **M-Bus** (wired) | From mbusio, including scan, secondary addresses and the decoder. | 1 |
| **KNX IP** | Tunnelling and routing. **Import the ETS project** (`.knxproj`): group addresses with datapoint types become points with unit and kind, without typing a single address. | 4 |
| **OPC UA** (client) | Browse the server tree, pick nodes, subscriptions. | 5 |
| **HTTP / REST** | Poll a JSON URL and pick fields as with MQTT; receive webhooks. Covers inverters, chargers, weather services and many devices with a web API. | 2 |
| **LoRaWAN** | Through a network server (ChirpStack, The Things Network) over MQTT, with payload decoders from the public device repository. | 5 |
| **wireless M-Bus** | USB receiver, OMS decryption with keys per meter (shares the decoder with M-Bus). | later |
| **BACnet MS/TP** | First through a BACnet router (no own work); later directly through an RS-485 adapter. | later |

Not planned as own drivers: DALI, EnOcean, SNMP, Zigbee, … They come in through a gateway
(most of them speak MQTT, Modbus or BACnet) or as an external driver (10.3).

### 10.3 External drivers: anyone can add a protocol

A driver can also be **a separate program in any language** (Python, Node.js, Go, …), shipped
as a plugin (section 12). bmsio
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
| **MQTT namespace** | Every point, alarm and device state, always on (section 11). |
| **MQTT extras** | Home Assistant discovery, Sparkplug B (for SCADA systems such as Ignition). |
| **REST API** | Engineering data, history, scripts. |

This makes bmsio useful from day one as a **gateway**, even before it is used as the BMS.

## 11. MQTT first

MQTT is not one driver among many: it is **the way everything in and around bmsio talks**.
Whatever a site knows (BACnet, Modbus, M-Bus, KNX, …) appears as one clean MQTT tree, in the
spirit of a *Unified Namespace*. Anyone who can use MQTT can read and write the whole building,
with rights.

### 11.1 Built-in broker, on by default

- bmsio contains an MQTT 5 broker, **switched on by default** (port 1883 locally, 8883 with TLS,
  MQTT over WebSocket on the web port).
- **MQTT users are bmsio users.** Roles become ACLs automatically: a viewer can subscribe, an
  operator can also write to `…/set`, a plugin gets exactly the topics in its permissions.
  No separate password file.
- An **existing broker** (Mosquitto, EMQX, HiveMQ) can be used instead: bmsio then publishes its
  tree there and does not run its own.

### 11.2 Topic tree

Readable paths from the model (section 4), and a stable path by id for machines:

```
bmsio/v1/{site}/{building}/{equip…}/{point}          value (retained)
bmsio/v1/{site}/{building}/{equip…}/{point}/$meta    name, tags, unit, kind, writable (retained)
bmsio/v1/{site}/{building}/{equip…}/{point}/set      write request
bmsio/v1/{site}/id/{pointId}                         the same value, path never changes
bmsio/v1/{site}/$alarms/{alarmId}                    active alarms (retained), cleared = empty
bmsio/v1/{site}/$events                              alarm events, acknowledgements, writes
bmsio/v1/{site}/$devices/{driver}/{device}           online / offline (retained, last will)
```

Example: `bmsio/v1/hq/main/ahu01/supply-air-temp`

**Payload**, JSON, the same for every point:

```json
{ "v": 21.4, "u": "°C", "q": "good", "ts": "2026-11-11T10:00:00Z" }
```

`q` is the quality (good / stale / fault / manual). For very simple consumers a point can also
be published as a plain value on an extra `…/raw` topic (switch per point or per subtree).

**Writing:** publish `{"v": 22.0}` to `…/set`, optionally with `"prio": 8` (BACnet priority) or
`"release": true`. bmsio checks the right, writes to the device, logs it in the audit log and
answers through MQTT 5 request/response (`response topic`) with ok or the reason it failed.

Renaming a plant changes the readable path; the old path gets an empty retained message, the
`id` path stays. Integrations that must survive renames use the `id` path.

### 11.3 Everything uses it

| Who | How it uses MQTT |
|---|---|
| **Browser UI** | Live values come over **MQTT over WebSocket**, with a short-lived token from the login. The UI subscribes to exactly the points on screen. There is only one live data path, so the MQTT interface is always complete: whatever the UI can show, an integration can read. |
| **Drivers in plugins** | Besides stdin/stdout (10.3), a driver plugin can simply publish to and subscribe from its own subtree `bmsio/v1/{site}/$drivers/{plugin}/…`. A driver can then also run on another machine. |
| **Apps in plugins** | Subscribe to points, publish to `…/set`; REST only for history and engineering data. |
| **Devices** | Shelly, Tasmota, Zigbee2MQTT, ESPHome, own ESP32 sensors publish straight to the built-in broker; auto-discovery or "listen and suggest" (10.2) turns them into points. |
| **Hub** (later) | The site bridges its tree to the hub's broker over one outgoing TLS connection, with a queue when the link is down. The hub sees all sites under `bmsio/v1/{site}/…`. |
| **Other systems** | Node-RED, Home Assistant, Grafana, Ignition, Python scripts: subscribe and done. |

### 11.4 Where MQTT is not used

The core does **not** route its internal work through the broker: drivers, history, alarms and
the scheduler talk inside the process. A broker problem or a misbehaving client must never stop
history or alarms. The broker is the window to the outside, fed by the core, not its heart.

History queries and engineering (point lists, templates, versions) go over the REST API,
because they are requests with large answers, not live data.

### 11.5 Tools to make MQTT easy

- **MQTT explorer in the browser**: tree of all topics with live values, publish a test value,
  see who is connected. Nobody needs an extra tool to debug.
- **"Copy as MQTT"** on every point, plant and graphic: the topic, an example payload and a
  ready snippet for Node-RED, Home Assistant and Python.
- **Connection page per client** in the settings: create a user for a device or integration
  with a click, with only the topics it needs.

## 12. Plugins

The community should be able to add to bmsio without changing its core and without a release
of bmsio. A plugin is **a folder (or `.zip`) with a `plugin.toml`** and whatever its kind needs.

### 12.1 Kinds

| Kind | Content | Runs code? | Example |
|---|---|---|---|
| **Device profile** | YAML (Modbus registers, MQTT/JSON mapping, BACnet object name rules) | no | "Modbus profile for a heat meter of make X" |
| **Template** | `template.json`, `graphic.svg`, `defaults.json` (section 5) | no | "Air handling unit with heat recovery and humidifier" |
| **Symbols** | SVG symbols with states | no | "Symbol set for cooling plants" |
| **Mapping rules** | Rules from names / plant designation to tags | no | "Rules for the naming code of company Y" |
| **Translation** | Language files | no | A new interface language |
| **Driver** | Program, JSON over stdin/stdout (10.3) or over MQTT (11.3) | yes, as own process | "SNMP for UPS and IT racks" |
| **Widget** | Web component (JS + CSS) for graphics and dashboards | yes, in the browser, in a sandbox | "Wind rose", "Sankey energy flow" |
| **Script** | Small JavaScript, written in the browser (12.4) | yes, inside bmsio, in a sandbox | "Mixed temperature from two sensors", "Alarm if pump runs but no flow" |
| **App** | Program that uses the API: reads points and history, writes, adds pages | yes, as own process | "Monthly energy report", "Heating curve optimiser", "Weather forecast" |

Most community work will be **data plugins** (profiles, templates, symbols, rules). They run no
code and therefore carry no risk; they are the easiest way to contribute and should be the
first to exist.

### 12.2 Manifest

```toml
id          = "acme-snmp"
name        = "SNMP for UPS and racks"
version     = "1.2.0"
kind        = "driver"
bmsio       = ">=0.4"          # plugin API version it needs
author      = "…"
licence     = "MIT"
exec        = "python3 driver.py"

[permissions]                  # shown before install, enforced at run time
network     = ["udp:161"]
points      = "own"            # only points created by this plugin
write       = false
```

### 12.3 Write your own, share if you want

Every user can write plugins, and **sharing is a choice, never a condition**:

- **Built in the UI, exported as plugin:** a template, symbol set, device profile or set of
  mapping rules made in the browser is already a plugin. The button **Export as plugin** writes
  the folder with `plugin.toml`; nobody has to know the format.
- **New plugin from a starter:** "New plugin" creates a working skeleton for a driver (Python
  or Node.js), widget, script or app, with a README and tests, ready to edit.
- **Private stays private:** a plugin installed locally never leaves the site. No upload, no
  account, no telemetry. Companies can keep plugins for their own customers.
- **Sharing:** the button **Share** checks the plugin (schema, manifest, licence field), packs
  it and explains the pull request to the community index step by step. Later it can open the
  pull request itself.
- **Fork and improve:** every shared plugin shows its source; "Copy and change" makes a local
  copy with a new id, and the original author is named in it.

### 12.4 Scripts in the browser

For small logic, a process is too much and a separate project too much work. A **script** is a
few lines of JavaScript, written in an editor in the browser with completion for point names:

```js
// Calculated point: mixed temperature
const t1 = point("ahu01/supply-air-temp");
const t2 = point("ahu01/outside-air-temp");
return 0.7 * t1 + 0.3 * t2;
```

Scripts run in an embedded JavaScript engine (QuickJS through `rquickjs`) in a sandbox, with a
time and memory limit, without network or file access, and only with the points the script
names. They are triggered by value changes or a timer. JavaScript because most people who write
a few lines (Node-RED, Home Assistant, web) already know it.

### 12.5 Install and update, in the browser

- **Plugin page** in the settings: search the community index, see description, permissions,
  version and source, click **Install**. Updates are shown like new template versions (diff of
  permissions, changelog).
- **Community index:** a public Git repository with one entry per plugin (name, source URL,
  checksum, signature). Adding a plugin is a pull request there. Nothing is hosted by us
  except that list.
- **Local install:** upload a `.zip` or give a Git URL, for own plugins and for sites without
  internet.
- **Development mode:** a plugin folder on the server is reloaded on change, with its log in
  the browser.

### 12.6 Security

A plugin can switch pumps, so it is treated with care:

- **Code plugins run as their own process**, under their own Linux user, with systemd /
  namespace limits: only the network and files listed in `permissions`. A crash or hang of a
  plugin does not stop bmsio; it is restarted with a back-off and shown as faulty.
- **Writing** to points needs the `write` permission, and every write goes into the audit log
  with the plugin as author, like a user's write.
- **Widgets** run in a sandboxed `<iframe>` and get values only through a message channel, so a
  widget cannot read the session or call the API on its own.
- **Signed packages** from the community index; unsigned plugins need an explicit "trust"
  click by an admin.
- **Data plugins** are checked against their schema on install; a broken template cannot be
  saved.

### 12.7 Later: WebAssembly

Scripts (12.4) cover small logic in JavaScript. For heavier logic in other languages, plugins
can later also be WebAssembly modules run inside bmsio (`wasmtime`), sandboxed by design and
written in Rust, Go, C or AssemblyScript. Processes stay the default for drivers and apps
because they work with any language and any library.

## 13. Relation to mbusio

mbusio **stays** a standalone tool for M-Bus commissioning and service. bmsio reuses:

- the **M-Bus driver** and decoder (as a library crate split out of mbusio),
- the **data point idea** (stable number, quality, unit, how values are passed on;
  [konzept-ausgaenge.md](konzept-ausgaenge.md), [konzept-dauerbetrieb.md](konzept-dauerbetrieb.md)),
- the **simulator**, the **report** and the **UI principles** (one next step, tables for mass
  work, keyboard; [konzept-oberflaeche-neu.md](konzept-oberflaeche-neu.md)),
- the **languages** set-up ([concept-languages.md](concept-languages.md)).

Nothing is removed from mbusio for this.

## 14. Phases

| Phase | Content | Done when |
|---|---|---|
| **0 BACnet + MQTT** | Data model, tags subset, SQLite schema, driver interface; **BACnet/IP read** with COV, EDE import; **built-in broker and topic tree** (section 11) with users and ACLs; **MQTT as driver** (devices publishing into bmsio, auto-discovery, "listen and suggest"); MQTT explorer in the browser; history | The pilot site's BACnet points appear under readable topics, an MQTT device (e.g. a Shelly) becomes a point next to them, and both can be watched in the browser and in any MQTT client. |
| **1 Viewer** | Live graphics from 3–5 built-in templates (AHU, heating circuit, boiler, meters) over MQTT over WebSocket, tag binding, trends; Modbus with profiles; M-Bus | The pilot plant runs on it, read-only, with graphics. |
| **2 Engineering + plugins** | Discovery for all drivers, mapping rules, point list table, apply templates, versions, HTTP driver; plugin format, data plugins (profiles, templates, symbols, rules) and driver plugins, community index | A new site is set up without editing a file by hand; someone outside the project publishes a device profile. |
| **3 Editor** | Graphic editor, symbol library, own templates, template diff, widget plugins, scripts in the browser, "Export as plugin" and "Share" | An engineer builds a new plant type in the browser. |
| **4 Operation** | Writing (BACnet with priority, also over `…/set`), alarms, schedules, users and rights, audit log; KNX with ETS import; outputs (BACnet server, Modbus server, Home Assistant discovery, Sparkplug B) | It can run a building without a second system. |
| **5 Hub + more** | Hub across sites; OPC UA, LoRaWAN; app plugins, WebAssembly plugins | Multi-site customer. |

Phase 0 is **BACnet and MQTT, both directions for MQTT**: the pilot site is BACnet, and MQTT is
the backbone, so both are needed from the start. A BACnet/MQTT gateway with a clean topic tree
is already useful on its own, before there is a single graphic. It also proves the hardest parts
early: the BACnet stack, the data model behind the topics, and that two very different
protocols fit the same driver interface.

## 15. Open questions

- **Name** of the project and repository.
- **BACnet stack:** own Rust implementation or a C stack through FFI?
- **Editor:** maxGraph or own SVG editor? Decide after a one-week prototype of both.
- **Symbol style:** which symbol conventions do the engineers in the target market expect?
  To be reviewed by someone from the trade, like the languages.
- **First pilot site:** BACnet (decided), controllers from **Kieback&Peter** (DDC4000 series)
  and **LOYTEC** (L-INX automation servers). Both are BTL-listed with BACnet/IP; LOYTEC also
  offers BACnet/SC, OPC UA and MQTT on some models. Open: exact models and firmware, number of
  points, BACnet/IP directly or MS/TP behind a router, BBMD needed, can the vendor tools export
  EDE files?
- **Broker:** embedded `rumqttd` good enough (MQTT 5, ACLs, WebSocket), or ship Mosquitto next to
  the binary? Decide with a load test of a few thousand points.
- **Licence of the core:** see section 17.2, to be decided before the repository is public.
- **Licence of templates and symbols** in a shared library (MIT as well, or CC BY?).
- **Plugin index:** who reviews pull requests to the index, and who holds the signing key?
- **Plugin API stability:** from which version is the plugin API promised to stay compatible?

## 16. Existing free tools, and why another one

| Tool | Strength | Why not enough here |
|---|---|---|
| **Home Assistant**, **openHAB** | Huge driver choice, free | Made for homes: no plant templates, no BMS-style point lists, alarms or commissioning. |
| **FUXA** (web SCADA, MIT) | Web-based SVG editor, Modbus/BACnet/OPC UA | General SCADA: graphics are bound to addresses, no template-to-many-plants workflow, no semantic model. Worth studying for its editor. |
| **Node-RED** | Flexible glue | A tool for integrators, not an operator interface. |
| **OpenRemote** (AGPL) | Open-source IoT platform aimed at buildings and real estate, protocol agents, edge gateway | Closest in spirit. Asset model and rules, but no plant templates with graphics, and AGPL limits embedding. Worth a close look. |
| **ThingsBoard** (CE: Apache 2.0) | Mature IoT platform with dashboards and MQTT; newer versions have SCADA symbols | Device and telemetry centred, not plant centred; BACnet only through a gateway. |
| **ScadaBR**, **Rapid SCADA** | Free SCADA, ScadaBR with BACnet | Classic SCADA engineering, dated web UI, no semantic model. |
| **VOLTTRON** (Eclipse), **BEMServer** | Building energy research and energy management | Platforms for data and energy analysis, not an operator interface with graphics. |
| **Haxall** (open core of SkySpark) | Project Haystack reference, tags and analytics | Shows how Haystack is done well; not a BMS with graphics and BACnet operation. |
| Vendor systems (e.g. **Qanteon** from Kieback&Peter, **LWEB-900** from LOYTEC), **Niagara**, **Avelon** | Complete and proven | Commercial, licensed per site or point, tied to one vendor's world. |

The gap bmsio fills: **semantic model + templates + engineering in the browser**, free.

## 17. Open source

### 17.1 What "100% open source" means here

- **One edition.** Every feature is in the public code. No open core, no closed "enterprise"
  modules, no feature behind a licence key, no limit on points, sites or users.
- **No lock-in.** Every piece of data can be exported in open formats (project file, CSV, the
  MQTT tree, the REST API); the formats are documented.
- **No telemetry, no account.** bmsio does not phone home. The community index is read only
  when the user opens the plugin page.
- **Open development.** Issues, concepts (`docs/`), roadmap and decisions are public once the
  repository is public. Contributions under the **Developer Certificate of Origin** (sign-off
  in the commit), **no CLA**: nobody, the project included, can later take the code closed.
- **Documented plugin API** with a promise of compatibility from version 1.0, and examples
  under MIT that anyone may copy into own plugins.

The repository starts private, so the first structure can be built in peace; it goes public as
soon as phase 0 runs at the pilot site.

### 17.2 Licence (to decide)

| Option | Effect |
|---|---|
| **MIT or Apache-2.0** (permissive) | Easiest for everyone to use and embed, as with mbusio. A company may build a closed product on it. Apache-2.0 adds a patent grant, useful with manufacturers. |
| **AGPL-3.0** for the core (copyleft) | Whoever changes the core and runs it, also as a service, must publish the change. bmsio stays open, also in hosted forks. Used by Grafana and OpenRemote. Some companies avoid AGPL. |

Either way, **plugins are free in their licence**: drivers, apps and widgets talk to bmsio over
documented interfaces (stdin/stdout, MQTT, REST, message channel) as separate programs, so a
plugin can be MIT, GPL or private. The plugin SDK and examples are MIT.

The community index lists only plugins with an OSI-approved open source licence, so everything
that is shared is open as well. Private plugins can be anything and are simply not listed.

The licence must be chosen before the first outside contribution: without a CLA it can hardly
be changed afterwards.
