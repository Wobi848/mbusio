# Concept: a simple, free, web-based building management system

Date: 11.11.2026 · Status: **draft, idea stage** · Working title: **bmsio**

A building management system (BMS; in German *Gebäudeleittechnik*, GLT; in the US often
*building automation system*, BAS) in the spirit of Avelon: everything in the browser, plant
graphics that look good, engineering that is fast. The difference: **100% open source**,
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

Installation, updates with backup and automatic rollback: section 21.
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

Templates are the heart of bmsio: a plant type is engineered **once** and used for many plants,
at many sites, by many people. Making, changing and sharing them must therefore be as easy as
using them.

### 5.1 What a template is

A template is a **package**, not just a picture:

```
ahu-hr/
  template.json      # id, version, slots, options, modules, metadata
  graphics/
    overview.svg     # the plant graphic
    detail-*.svg     # optional detail graphics
  defaults.json      # trends, alarms, critical points, schedules for the slots
  locales/*.ftl      # all texts, per language (section 20)
  docs/
    description.md   # what the template is for, function description, notes
    preview.png      # picture for the library
  test/
    simulation.json  # simulated values, so the template can be tried without a plant
```

**Slots** say which points the template needs, by meaning (tags), never by address:

```json
{
  "id": "ahu-hr",
  "version": "3.1.0",
  "title": "template-title",
  "options": {
    "hasCooler":   { "type": "bool", "default": true },
    "recovery":    { "type": "enum", "values": ["plate", "wheel", "runaround"], "default": "wheel" }
  },
  "modules": {
    "supplyFan":   { "use": "fan@2",       "prefix": "supply" },
    "exhaustFan":  { "use": "fan@2",       "prefix": "exhaust" },
    "cooler":      { "use": "coil-cool@1", "if": "hasCooler" }
  },
  "slots": {
    "supplyTemp":   { "tags": "supply air temp sensor", "required": true },
    "supplyTempSp": { "tags": "supply air temp sp", "writable": true },
    "enable":       { "tags": "enable cmd", "writable": true, "critical": true }
  }
}
```

The **tags are the contract** between a template and a site. They come from the documented tag
set (section 4, Project Haystack subset). Own tags are allowed but must carry a prefix
(`acme:`) so they never clash. Because templates only speak tags, a template made at one site
works at any other site, whatever controllers it has.

### 5.2 Modules: templates made of templates

Real plants are made of the same parts again and again: fan, pump, valve, coil, damper, filter,
heat recovery, meter. These parts are **modules**: small templates with their own slots, symbol,
alarms and trends. A plant template is put together from modules plus its own slots.

- An improvement to the module `fan` (e.g. a better fault alarm) reaches every plant template
  that uses it, after the usual diff and confirmation (5.6).
- A new plant type is mostly assembled, not drawn: AHU = filter + 2 × fan + recovery + coils.
- Modules are versioned and shared exactly like templates.

### 5.3 Options and variants

One template covers many real plants through **options**: with or without cooler, plate or
wheel recovery, one or two stage fan. Options switch modules, slots and parts of the graphic
on and off. This keeps the library small: "AHU" with options instead of twenty AHU templates
that drift apart.

When the options are not enough, a template can be **derived** from another one: it inherits
everything and only changes or adds. Updates of the original reach the derived template.

### 5.4 Making a template

Four ways, all in the browser:

| Way | When |
|---|---|
| **From an existing plant** ("Make template from this plant") | The usual way. An engineer has set up one plant well; bmsio turns its points into slots (by their tags), its graphic into the template graphic, its trends and alarms into defaults. The engineer reviews the proposed slots and names, then saves. |
| **Assemble from modules** | A new plant type from known parts (5.2), then arrange the graphic in the editor. |
| **Copy or derive** an existing template | Small differences to a template from the library. |
| **From scratch** in the template editor | New modules, unusual plants. |

The **template editor** shows the template with simulated values (from `test/simulation.json`
or generated from the slots), so options, alarms and graphics can be tried at the desk. Every
text is entered once in the user's language and can be translated in the same place.

**Checks before a template can be saved or shared:**

- every slot has valid tags; every symbol in the graphic is bound to a slot that exists,
- every option combination renders (tested automatically with the simulation),
- texts exist at least in English and the author's language,
- description and preview exist (for sharing).

### 5.5 Applying a template to a plant

1. Pick the plant (an `equip`) and the template, choose the options.
2. bmsio fills the slots automatically: it searches the plant's points for the slot's tags.
3. The engineer sees a table: slot → point, with green (found), yellow (several candidates),
   red (missing). Missing optional slots hide their part of the graphic.
4. Save (as a draft, 18.7). The plant now has its graphic, trends and alarms.

The same works for **many plants at once**: select 12 AHUs, apply the template, check one table
with all of them.

**Vendor-neutral templates, vendor-specific rules.** Whether the slots fill themselves depends
on the points having the right tags. That is the job of **mapping rules** (section 7), which
are specific to a manufacturer or a naming code: "Kieback&Peter DDC4000, AHU program: object
name `*_TZu` → `supply air temp sensor`". Templates and mapping rules are separate packages,
so the same AHU template works with Kieback&Peter, LOYTEC or any other controller, and someone
who knows one manufacturer well can share just the rules.

### 5.6 Versions and updates

- Templates use **semantic versions**: `3.1.0 → 3.1.1` fixes, `3.2.0` adds (new optional slot,
  new alarm), `4.0.0` changes in a way that needs the engineer (slot removed or changed).
- Every plant remembers **which template and which version** it uses.
- **Local changes** on a plant (an extra trend, a moved symbol, a different alarm limit) are
  kept as a separate layer on top of the template, not mixed into it. A template update
  therefore does not overwrite them, and it is visible at a glance what a plant does
  differently from its template.
- **Update:** a new version shows a diff (graphic before/after, slots, alarms). "Apply to all
  12 plants" updates them as a draft; plants whose local changes conflict are listed and
  decided one by one.
- Plants never update on their own: the engineer decides, and only `engineering.publish`
  puts it into operation (18.7).

### 5.7 Sharing

Templates are shared at three levels, all with the same package:

| Level | Who sees it | How |
|---|---|---|
| **Site** | Only this installation | Default for every new template. |
| **Organisation** | An integrator's or owner's own library, used across all their sites | A Git repository or a folder share as a library source, or the hub (section 3). Typical: a company's standard templates. |
| **Community** | Everyone | The community library (section 12): **Share** checks the template (5.4), packs it and leads through the pull request. |

- A site can use **several library sources**: community, one or more organisation libraries,
  local. The library page shows them together, with the source of each template.
- **Search and filter** by plant type, tags, options, manufacturer rules available, language,
  rating "used at N sites" (only counted from sites that agree to report it; off by default).
- **Credit:** author, licence and the template it was derived from stay in the package and are
  shown in the library.
- **Improve together:** a user who changes a community template can offer the change back to
  the author (a pull request made from the diff), instead of only keeping a private copy.
- **Licence of shared templates:** to be decided (section 15); the default proposal is the same
  for all shared templates, so they can be combined freely.

### 5.8 What comes with bmsio

bmsio ships a **starter library**, maintained by the project, so the first site works without
building anything:

- Modules: fan, pump, valve, damper, heating coil, cooling coil, heat recovery (plate, wheel,
  runaround), filter, humidifier, sensor, meter.
- Plants: air handling unit, heating circuit (mixed, unmixed), boiler, heat pump, district
  heating substation, domestic hot water, chiller, room (temperature, CO₂, light, blinds),
  meter overview.
- Mapping rules for the pilot site's controllers (Kieback&Peter, LOYTEC) as the first examples.

The starter library is a normal library source; its templates can be copied, derived and
improved like any other.

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
| **Templates** | Library with several sources, make from a plant, modules, options, editor with simulation, apply to many plants, version diff, share (section 5). |
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
- **Users and rights:** roles with a scope in the tree, one rights check for UI, API, MQTT
  and plugins, audit log, remote service access (section 18).

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
Section 11.3 describes commands in detail.

Renaming a plant changes the readable path; the old path gets an empty retained message, the
`id` path stays. Integrations that must survive renames use the `id` path.

### 11.3 Commands: how switching a plant works

MQTT itself only knows "publish" and "subscribe"; it has no "write" and no "answer". A command
is therefore built from a few rules. They follow the common practice of MQTT 5 and of
Sparkplug B (`NCMD`/`DCMD` commands, birth and death messages), adapted to building automation.

**Example: an operator switches AHU 01 off from the graphic.**

```
Browser / Node-RED          bmsio core                    Kieback&Peter controller
      │                         │                                   │
 1    │ PUBLISH …/ahu01/enable/set                                  │
      │ {"v":false,"prio":8}  QoS 1, not retained, expiry 10 s      │
      │ response topic + correlation data                           │
      │────────────────────────►│                                   │
 2    │                         │ check: right? device online?      │
      │                         │ point writable? interlock?        │
 3    │                         │ BACnet WriteProperty              │
      │                         │ BV:12 Present_Value = inactive,   │
      │                         │ priority 8                        │
      │                         │──────────────────────────────────►│
 4    │                         │◄──────────────── SimpleACK ───────│
 5    │◄── response: {"ok":true} (to the response topic)            │
 6    │                         │◄── COV: fan feedback = off ───────│
 7    │◄── …/ahu01/enable  {"v":false,"q":"good"}  (retained)       │
      │◄── …/ahu01/supply-fan-feedback {"v":false}                  │
```

**The rules**

| Rule | Why |
|---|---|
| **State and command are separate topics.** `…/enable` is the state (published by bmsio only, retained); `…/enable/set` is the command (published by clients). | Nobody can fake a state; a client sees what the plant *is*, not what someone *wanted*. |
| **Commands are never retained.** | A retained command would be delivered again to every new subscriber and after every restart, and could switch a plant on hours later. |
| **Commands expire.** MQTT 5 *message expiry*, default 10 s. | A command stuck in a queue during a network outage is dropped instead of being executed late. |
| **Absolute values, no toggles.** `{"v":false}`, never `"toggle"`. | QoS 1 can deliver a message twice; writing "off" twice is harmless, toggling twice is not. |
| **QoS 1 for commands.** | Delivered at least once; together with absolute values that is safe. |
| **Answer per command.** MQTT 5 *response topic* and *correlation data*; the answer says `ok` or why not (`no right`, `device offline`, `not writable`, `interlock`, `device rejected: write access denied`). | The sender knows at once whether the command was accepted, without guessing from the state. |
| **The UI shows the feedback, not the wish.** After the command the switch shows "switching…", and only the real feedback (step 6–7) changes it. If command and feedback differ after a set time, an alarm "command/feedback mismatch" is raised. | That is how a professional BMS works: the operator sees the plant, not the button. |
| **bmsio is the only writer to the controllers.** MQTT clients never talk BACnet; they ask bmsio. | One place for rights, audit log, priorities and limits. |
| **Online state with last will.** Every driver and device has a retained `online`/`offline` topic; the broker publishes `offline` itself when the connection breaks. | A command to an offline device is refused at once instead of disappearing. |

**BACnet priorities: who has the say**

A commandable BACnet object has 16 priority levels (1 = highest). The controller uses the
highest one that is set; if none is set, the *Relinquish_Default*. The BACnet standard
recommends this use:

| Priority | Use |
|---|---|
| 1 | Manual life safety |
| 2 | Automatic life safety (fire, smoke) |
| 3, 4 | free |
| 5 | Critical equipment control |
| 6 | Minimum on / off time |
| 7 | free |
| **8** | **Manual operator** (an operator at a workstation) |
| 9–16 | free, often used by programs and schedules |

bmsio writes at **8 by default**, because that is exactly what an operator command from a BMS
is. Above it (1–7) stay life safety, critical equipment protection and minimum run times: a
command from bmsio must never override a fire control or switch a compressor off before its
minimum run time. Below it (9–16) are usually the controller's own programs and schedules, so
the operator's command wins over them.

The default is a site setting and can be changed per point. **At the pilot site this must be
checked:** which priorities the Kieback&Peter and LOYTEC programs and schedules really use.
If a program writes at 8 or higher, a command at 8 has no effect or is taken back.

Not every object is commandable. A setpoint is often an Analog Value without a priority array;
there bmsio writes the value directly, and "release" does not exist. The point details show
which kind a point is.

- **Release** (`{"release":true}`) removes bmsio's value at that level, and the controller goes
  back to its own program (*automatic*). The UI shows this as **Hand / Auto**: a point overridden
  by bmsio is marked "manual" until it is released.
- The full priority array is shown in the point details, so it is visible who holds the point
  (bmsio, a local panel, a fire control at priority 1–2).
- **Forgotten overrides** are a classic problem: a value at priority 8 stays until someone
  releases it, also for months. bmsio therefore keeps a list **"Manual overrides"** of every
  point it holds, with who, when and why. An override can be given a duration ("off for
  2 hours"); bmsio releases it on its own afterwards.

**Critical commands** (plant off, pumps, boilers) can require a confirmation at the button
and a reason, which goes into the audit log. Which points count as critical is set in the
template.

**Devices that speak MQTT themselves** (Shelly, Tasmota, ESPHome) have their own command topic.
The device profile maps it: bmsio receives `…/light-hall/set {"v":true}`, checks it as above and
publishes `shellies/hall/relay/0/command` → `on` to the device. For the operator and for
integrations there is no difference to a BACnet point.

### 11.4 Everything uses it

| Who | How it uses MQTT |
|---|---|
| **Browser UI** | Live values come over **MQTT over WebSocket**, with a short-lived token from the login. The UI subscribes to exactly the points on screen. There is only one live data path, so the MQTT interface is always complete: whatever the UI can show, an integration can read. |
| **Drivers in plugins** | Besides stdin/stdout (10.3), a driver plugin can simply publish to and subscribe from its own subtree `bmsio/v1/{site}/$drivers/{plugin}/…`. A driver can then also run on another machine. |
| **Apps in plugins** | Subscribe to points, publish to `…/set`; REST only for history and engineering data. |
| **Devices** | Shelly, Tasmota, Zigbee2MQTT, ESPHome, own ESP32 sensors publish straight to the built-in broker; auto-discovery or "listen and suggest" (10.2) turns them into points. |
| **Hub** (later) | The site bridges its tree to the hub's broker over one outgoing TLS connection, with a queue when the link is down. The hub sees all sites under `bmsio/v1/{site}/…`. |
| **Other systems** | Node-RED, Home Assistant, Grafana, Ignition, Python scripts: subscribe and done. |

### 11.5 Where MQTT is not used

The core does **not** route its internal work through the broker: drivers, history, alarms and
the scheduler talk inside the process. A broker problem or a misbehaving client must never stop
history or alarms. The broker is the window to the outside, fed by the core, not its heart.

History queries and engineering (point lists, templates, versions) go over the REST API,
because they are requests with large answers, not live data.

### 11.6 Tools to make MQTT easy

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
| **Driver** | Program, JSON over stdin/stdout (10.3) or over MQTT (11.4) | yes, as own process | "SNMP for UPS and IT racks" |
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
| **0 BACnet + MQTT** | Data model, tags subset, SQLite schema, driver interface; **BACnet/IP read** with COV, EDE import; **built-in broker and topic tree** (section 11) with ACLs derived from the rights; **users, built-in roles with scope, the one rights check and the audit log** (section 18); the foundations of section 19; language system with German and English (section 20); package, systemd service and setup page (21.1); manual backup and restore (22); **MQTT as driver** (devices publishing into bmsio, auto-discovery, "listen and suggest"); MQTT explorer in the browser; history | The pilot site's BACnet points appear under readable topics, an MQTT device (e.g. a Shelly) becomes a point next to them, and both can be watched in the browser and in any MQTT client. |
| **1 Viewer** | Live graphics from 3–5 built-in templates (AHU, heating circuit, boiler, meters) over MQTT over WebSocket, tag binding, trends; Modbus with profiles; M-Bus; one-click update with backup and rollback (21.2); scheduled backups to targets (22.2); French and Italian | The pilot plant runs on it, read-only, with graphics. |
| **2 Engineering + plugins** | Discovery for all drivers, mapping rules, point list table, apply templates, versions, HTTP driver; plugin format, data plugins (profiles, templates, symbols, rules) and driver plugins, community index | A new site is set up without editing a file by hand; someone outside the project publishes a device profile. |
| **3 Editor** | Graphic editor, symbol library, template editor with simulation, "make template from this plant", modules and options, template versions and diff, organisation libraries, widget plugins, scripts in the browser, "Export as plugin" and "Share" | An engineer builds a new plant type in the browser. |
| **4 Operation** | Writing (BACnet with priority, also over `…/set`), alarms with notification by scope and role, schedules, critical points with step-up, own roles, second factor, single sign-on (OIDC), four-eyes option, time-limited remote service access; KNX with ETS import; outputs (BACnet server, Modbus server, Home Assistant discovery, Sparkplug B) | It can run a building without a second system. |
| **5 Hub + more** | Hub across sites, organisations and integrator access across customers, on-call plans; OPC UA, LoRaWAN; app plugins, WebAssembly plugins | Multi-site customer. |

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
- **Roles:** do the built-in roles (18.4) match how sites are run in practice? To be checked
  with operators and integrators.
- **Performance targets** (section 19): realistic for the pilot and for larger sites?
- **Package hosting:** where the apt repository and the signed releases live (GitHub Releases
  plus a static apt repository, or a service such as Cloudsmith's free open source plan).
- **Controller backup** (22.4): which of the pilot's controllers support BACnet backup and
  restore?

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

## 18. Users, roles and rights

A BMS switches real plants. Who may see, operate, change and administer what must be clear,
enforced everywhere and traceable. This section is part of the foundation (section 19): it is
built in phase 0 and only extended later, never retrofitted.

### 18.1 Who acts: principals

Everything that can do something in bmsio is a **principal**. All of them go through the same
rights check.

| Principal | Example | Logs in with |
|---|---|---|
| **User** | Operator, technician, engineer | Password + optional second factor, or single sign-on |
| **Service account** | Node-RED, Home Assistant, a Shelly, Grafana | API token or MQTT user/password, created on the connection page (11.6) |
| **Plugin** | SNMP driver, energy report app | Own identity created on install, rights from its manifest (12.2) |
| **Script** | Calculated point | Own identity, may only touch the points it names (12.4) |
| **Kiosk** | Wall screen in the entrance | Device token bound to a few pages, read-only |

### 18.2 Rights are permissions on a part of the tree

A right is always **a permission on a scope**. The scope is a node of the model tree (section 4):
the whole installation, a site, a building, a floor, a plant. A permission on a node applies to
everything below it.

```
Anna   Operator       on  site "Headquarters"
Anna   Engineer       on  building "Headquarters / Annex"
Ben    Viewer         on  everything
Ben    Operator       on  plant "HQ / Main / AHU 01"
Node-RED  Reader      on  site "Headquarters"
```

A principal can have several such assignments; the rights add up. There is no "deny": rights
are only granted, which keeps them understandable. For a point, the effective rights are shown
in the point details ("Anna may: see, operate; why: Operator on site Headquarters").

### 18.3 Permissions

Permissions are fine-grained and fixed in the code; roles (18.4) bundle them.

| Area | Permission | Meaning |
|---|---|---|
| See | `view` | Graphics, values, point lists |
| | `history.view` | Trends and history, export |
| | `alarm.view` | Alarms and alarm history |
| Operate | `alarm.ack` | Acknowledge alarms |
| | `point.write` | Write normal points (setpoints, switches) |
| | `point.write.critical` | Write points marked critical (18.6) |
| | `override.release` | Release manual overrides of others |
| | `schedule.edit` | Change schedules and holidays |
| Engineer | `model.edit` | Points, plants, tags, mapping rules, drivers |
| | `graphic.edit` | Graphics and templates |
| | `engineering.publish` | Put an engineering draft into operation (18.7) |
| | `alarm.config` | Alarm limits, classes, notification |
| Administer | `user.manage` | Users, roles, assignments in the scope |
| | `plugin.install` | Install, update, remove plugins |
| | `plugin.trust` | Trust unsigned plugins |
| | `system.config` | Network, certificates, broker, backup, updates |
| | `audit.view` | Read the audit log |

New permissions only come with a new feature and are given to the built-in roles at the same
time, so an update never takes rights away or grants them by surprise.

### 18.4 Roles

**Built-in roles**, cannot be deleted, cover most sites:

| Role | Typical person | Permissions |
|---|---|---|
| **Viewer** | Caretaker, management, tenant | `view`, `history.view`, `alarm.view` |
| **Operator** | Facility manager, caretaker on duty | Viewer + `alarm.ack`, `point.write`, `schedule.edit` |
| **Technician** | Service technician | Operator + `point.write.critical`, `override.release`, driver diagnostics |
| **Engineer** | System integrator, planner | Technician + `model.edit`, `graphic.edit`, `engineering.publish`, `alarm.config` |
| **Administrator** | IT, owner of the installation | Everything, including `user.manage`, `plugin.*`, `system.config`, `audit.view` |
| **Reader** (for integrations) | Node-RED, Grafana | `view`, `history.view` |

**Own roles** can be put together from the permissions, e.g. "Operator without schedules" or
"Energy manager" (`view`, `history.view` only). A role can be copied from a built-in one.

### 18.5 One check, everywhere

There is **exactly one function in the core** that decides: *may this principal do this action
on this object?* Every way into bmsio calls it:

| Way in | How the check applies |
|---|---|
| Browser UI | The UI hides what is not allowed, but the server checks every request again. Hiding is comfort, not security. |
| REST API | Every request, by token. |
| MQTT | The broker's ACLs are **derived** from the assignments: subscribe to the subtrees in scope with `view`, publish to `…/set` only with `point.write`; `…/set` on critical points is checked again in the core with `point.write.critical`. Changes to rights take effect at once, also for open connections. |
| Plugins, scripts | Through their identity, the same as users. A plugin can never have more rights than its manifest says, and never more than the admin who installed it. |

This is the most important rule of this section: **no feature may check rights on its own
way.** Tests check every API endpoint and topic against the permission table.

### 18.6 Protection for critical actions

- **Critical points** are marked in the template or point (e.g. plant on/off, boiler, chiller,
  fire dampers if read). Writing them needs `point.write.critical`, a confirmation at the
  button and a reason.
- **Step-up:** critical actions and all of "Administer" ask for the password or second factor
  again if the last check is older than a few minutes.
- **Four-eyes principle (optional, per site):** publishing engineering changes or changing
  users can require approval by a second person with the same right.

### 18.7 Engineering as drafts

Engineering changes (points, templates, graphics, alarm limits) are made in a **draft**. Only
`engineering.publish` puts the draft into operation, as a new version (section 7, Versions).
Operators therefore never see half-finished changes, and every change in operation has an author,
a time and a comment. A version can be rolled back.

### 18.8 Logging in

- **Local users** with password (Argon2id, length over complexity rules, rate limit and
  temporary lock after failed attempts).
- **Second factor:** TOTP (authenticator app) and passkeys (WebAuthn). An admin can require it
  per role, recommended at least for Engineer and Administrator.
- **Single sign-on** with OpenID Connect (Microsoft Entra ID, Google, Keycloak, Authentik, …).
  Groups from the identity provider can be mapped to role assignments, so leaving the company
  also removes access to bmsio.
- **Sessions** expire after inactivity (setting per role; a kiosk never, an Administrator after
  15 minutes).
- **No default password.** On the first start bmsio shows a setup page that creates the first
  administrator; until then nothing else is reachable.
- **Emergency access:** whoever has a shell on the Linux machine can reset an administrator
  with `bmsio admin reset` (logged). There is no hidden master password.

### 18.9 Remote service access

A typical case: the integrator's technician must help a customer's site. Instead of a shared
password:

- The customer's administrator grants a **time-limited access** ("Technician for company X,
  site Headquarters, 8 hours"), or the technician requests it and the customer approves.
- The access ends on its own. Everything the technician does is in the audit log under their
  own name.
- On a hub (section 3), an integrator organisation can be given access to the sites of several
  customers, each customer deciding for themselves.

### 18.10 Audit log

- Logged: logins and failed logins, every write and release, alarm acknowledgements, schedule
  changes, published engineering versions, user and rights changes, plugin installs, system
  settings. Each entry with principal, time, object, old and new value, reason and way in (UI,
  API, MQTT, plugin).
- **Append-only and tamper-evident:** every entry contains a hash of the previous one, so a
  deleted or changed entry is noticed.
- Searchable and exportable (CSV), with a retention time per site.
- Viewing needs `audit.view`; users can always see their own entries.

### 18.11 Alarm notification follows the rights

Who is notified of an alarm is set by **scope and role**, not by a list of e-mail addresses per
alarm: "alarms of class A in building Annex go to the Operators of that building". Later
**on-call plans** (Pikett) on top: who is on duty when, with escalation if nobody acknowledges.
Only someone who may see a plant receives its alarms.

### 18.12 Personal data

User names, e-mail addresses, phone numbers and the audit log are personal data (GDPR, Swiss
revDSG). bmsio stores only what it needs, shows a user their stored data, and can anonymise
the entries of a deleted user in the audit log after the retention time.

## 19. Foundations to get right from the start

These decisions are hard or impossible to change later. They are built in phase 0, even where
the feature that needs them comes later.

| Foundation | Decision | Why now |
|---|---|---|
| **One rights check** | Section 18.5, from the first API endpoint | Retrofitting rights means touching every feature. |
| **Stable ids** | Every object (point, plant, template, user, version) has a UUIDv7; names are never keys | Renaming must never break history, graphics, topics or integrations. |
| **Multi-site in the model** | Every record carries its site id, also on a single-site box | A site can later join a hub without migration. |
| **Time** | Stored in UTC; every site has a time zone; schedules are in local time and handle daylight saving | Mixing times is the most common source of wrong trends and switching an hour too early. |
| **Units** | Every value has a unit (UCUM / QUDT codes); conversion only for display | Templates and plugins can rely on it; °C vs K vs °F never gets mixed up. |
| **Quality on every value** | good / stale / fault / manual, from the driver to the UI | Added later, every driver and widget would have to be touched again. |
| **Versioned interfaces** | `bmsio/v1/…` topics, `/api/v1/…`, plugin API version, database migrations from the first schema | Integrations and plugins of others must not break with an update. |
| **Template format** | Modules, options, tags as the contract, local changes as a separate layer (section 5), already for the first built-in templates | Templates shared by others cannot be migrated by us; a format change later breaks the libraries. |
| **Engineering as versions** | Drafts and published versions (18.7) from the first editable thing | Undo and audit for engineering cannot be added afterwards. |
| **Audit log** | Section 18.10, from the first write | Writes without a log cannot be explained later. |
| **Secure by default** | TLS on (own certificate, ACME or self-signed), no default password, broker without anonymous access, secrets encrypted at rest | Weak defaults stay in the field for years. |
| **Languages** | No visible text in code; Fluent files, codes and parameters in data (section 20) | Retrofitting translation means touching every screen. |
| **Backup** | Encrypted, scheduled, checked; restore with writing switched off until confirmed (section 22) | A BMS without tested restore is a risk for the building. |
| **Performance targets** | Per site at least 50 000 points and 2 000 value changes per second on a Raspberry Pi 5 class device; tested in CI with simulators | The data paths are designed for it from the start instead of being rewritten. |
| **Simulators in CI** | BACnet device and MQTT devices simulated, every driver tested against them | Without them every change needs a real plant. |

## 20. Languages

bmsio is multilingual from the first line of code. **No text that a person reads is written in
the code**, in any part: interface, server messages, alarms, e-mails, reports, logs meant for
users, templates, plugins. mbusio shows what happens otherwise: texts written as `tr("deutsch",
"english")` pairs in the code had to be collected afterwards to add more languages
([concept-languages.md](concept-languages.md)).

### 20.1 Rules

| Rule | How |
|---|---|
| **Keys, not texts, in code** | Code only uses message ids: `alarm-ack-done`, `point-write-denied`. |
| **One format for server and browser** | [Project Fluent](https://projectfluent.org/) files (`.ftl`), read by `fluent-rs` in Rust and `@fluent/bundle` in the browser. Fluent handles plurals, gender and variables in the translation, not in the code. |
| **Neutral data, texts at the edge** | The core, the database, the API and MQTT carry **codes and parameters**, not sentences: an alarm is `{"code":"limit-high","value":31.2,"limit":30}`. The text is made when a person reads it, in *their* language. |
| **Audit log and messages in the reader's language** | Stored as code + parameters, so the same entry reads in German for one user and French for the next. |
| **E-mails and notifications** | In the language of each recipient, not of the sender or the server. |
| **Formats from the locale** | Numbers, dates, times, decimal comma and units through the browser's `Intl` and CLDR data, never by hand. |
| **Tests fail** | CI fails on a missing key in any language that is in the menu, on an unused key, and on visible text written directly in code or markup (lint). |

### 20.2 What is translated, and what is not

| Content | Translated? |
|---|---|
| Interface, server messages, alarms, reports, e-mails, setup and update pages | Always, by the project |
| **Templates, symbols, device profiles** | Yes: they carry their own `.ftl` files, so "Supply air temperature" in a template appears in each user's language. |
| **Plugins** | Yes, same mechanism; the plugin starter (12.3) already contains the language files. |
| Names given by the engineer (point names, plant names) | Optional: a name can have translations; without, it is shown as entered. Names from templates are translated automatically. |
| Tags, codes, topics, API | Never: they are the neutral language of the system (English). |

### 20.3 Languages and review

- Start with **German and English**; **French and Italian** early, because the pilot is in
  Switzerland; more through the community.
- A language enters the menu only after **review by someone from the trade**, as in mbusio.
  A BMS has its own words (*Sollwert*, *Rückmeldung*, *Störung*, *Pikett*), and a wrong word
  in an alarm text is a risk.
- A **glossary** per language fixes these words, so all translators use the same terms.
- Community translation through **Weblate** (open source, free hosting for open source
  projects), with the glossary and the review step.
- Layout uses CSS logical properties, so right-to-left languages are not blocked later.

## 21. Installation and updates

Goal: **installed in five minutes, updated with one click, and an update can never leave a
building without its BMS.**

### 21.1 Installation

| Way | Steps |
|---|---|
| **Debian, Ubuntu, Raspberry Pi OS** (recommended) | Add the bmsio package repository (one documented command, also as a short script that does only that) and `apt install bmsio`. Starts at once as a systemd service. |
| **Raspberry Pi image** | Flash to an SD card or SSD, plug in, open `http://bmsio.local`. |
| **Container** | `docker run` or a ready `compose.yaml`; also on a NAS. |
| **Other Linux** | Static binary, copy and start; a systemd unit file is included. |

Then, in the browser, the **setup page** (18.8) does the rest in one flow:

1. Language
2. First administrator (with a second factor offered)
3. Site name and time zone
4. Network: which network interface is the BACnet network; certificate (self-signed, own, or
   Let's Encrypt if reachable)
5. Backup target (21.3), with "later" allowed but reminded
6. Or instead of all this: **"Restore from backup"** (22.3)

After the setup, the next step is discovery (section 10). Nobody edits a configuration file;
for automated installs every setup answer can also be given as a file or environment variables.

### 21.2 Updates

- **One click in the browser.** bmsio shows that an update is available, what changes
  (changelog, in the user's language) and whether it contains database changes. Checking for
  updates is a setting (on by default, can be switched off); it only reads the public release
  list, it sends nothing about the installation.
- **Offline update:** upload the update file in the browser, for sites without internet.
- **Signed:** releases and packages are signed; bmsio refuses an unsigned or changed file.
- **Maintenance window:** an update can be scheduled ("tonight at 02:00") or run now.
- **What happens on an update:**
  1. Automatic backup (section 22) before anything is changed.
  2. The new version is installed next to the old one.
  3. bmsio restarts on the new version and migrates the database.
  4. **Health check:** web interface, broker, drivers connected, values flowing.
  5. If the check fails within a few minutes: **automatic rollback** to the old version and
     the backup from step 1, and an alarm saying why.
- **Short break only:** the controllers run on their own, so a restart of bmsio of a few
  seconds does not affect the plants. Values arriving during the restart are taken from the
  devices again (BACnet COV re-subscribe, retained MQTT values).
- **Channels:** *stable* (default) and *beta*. Several sites can be updated together from a hub
  later, one after another, stopping at the first failure.
- **Plugins** are updated separately, with the same backup and rollback.

Technically, bmsio runs as an unprivileged user. Installing a package needs root, so a very
small separate service (`bmsio-updater`, root) does only one thing: install a **signed bmsio
package** it is pointed to, and roll back. For containers, the update page shows the new image
tag and the command, because a container should be updated from outside.

### 21.3 Versions and compatibility

- Semantic versioning. A minor update never breaks plugins, topics or the API (`v1`). A major
  version keeps the old interfaces working for at least one major version, with warnings.
- Every database migration is tested in CI against a database of each earlier release.
- Downgrade is only through the backup taken before the update; the rollback (21.2) does this
  automatically.

## 22. Backup and restore

A BMS without a tested restore is a risk for the building. Backup is therefore part of the
setup, runs on its own, and a restore is as simple as an installation.

### 22.1 What is in a backup

| Part | Included |
|---|---|
| Model, points, tags, plants | always |
| Engineering versions, templates, graphics, symbols | always |
| Drivers and their settings, device profiles, mapping rules | always |
| Users, roles, assignments, service accounts (passwords only as hashes) | always |
| Certificates, keys, secrets (e.g. MQTT bridge passwords) | always, encrypted |
| Alarm configuration, schedules, notification and on-call plans | always |
| Installed plugins with their data | always |
| Audit log | always |
| **History** (trends) | selectable: none, last 30 days, all; large, so it can also be backed up separately |

Not included, because it is rebuilt: live values, MQTT retained messages, caches.

### 22.2 Backups on their own

- **Scheduled:** daily by default, plus before every update (21.2) and before a restore.
- **Targets:** local disk, USB stick, network share (SMB, NFS), SFTP, S3-compatible storage,
  WebDAV (e.g. Nextcloud); later the hub (section 3). Several targets at once are possible.
- **Retention:** e.g. 7 daily, 4 weekly, 12 monthly; set per target.
- **Encrypted:** AES-256-GCM with a key from a backup password (as in mbusio). The password
  is never stored in the backup. bmsio warns clearly that a lost password means a lost backup.
- **Checked:** every backup is read back and its checksum checked. A failed or missing backup
  is an **alarm in bmsio**, like a plant fault, so it is noticed.
- **Consistent:** taken while bmsio runs, from a consistent database snapshot (SQLite online
  backup), without stopping the drivers.
- **Open format:** a documented archive (manifest, JSON, SQLite), so the data is readable even
  without bmsio (section 17).

### 22.3 Restore

- **On new hardware** (the Raspberry Pi died): install bmsio, choose **"Restore from backup"**
  on the setup page, pick the file or the backup target, enter the password. Done.
- **On the same machine:** settings → backup → choose a backup → restore. A backup of the
  current state is taken first, so a restore can itself be undone.
- **Writing is off after a restore.** The restored bmsio reads from all devices but **does not
  write** (no commands, no schedules sent) until an administrator confirms. A restored system
  must not switch plants on its own based on an old state.
- **Partial restore:** only engineering (one version, one plant, one template), only users, or
  only history; e.g. to undo an engineering mistake without touching anything else. Within
  engineering, versions (18.7) are usually the quicker way.
- **Restore test:** a backup can be opened in a **read-only test mode** to see what is in it,
  without changing the running system.
- **Move to other hardware** is the same as a restore: backup on the old machine, restore on
  the new one, including site name and certificates.

### 22.4 Backups of the controllers (later)

The BMS knows the plant, but the programs live in the controllers. Where controllers support
the BACnet backup and restore procedure (configuration files read and written through BACnet),
bmsio can back up their configuration on a schedule and keep it with the site backup.
Support differs between manufacturers and models; for others bmsio at least keeps the
discovered object list and EDE data, so a replaced controller can be checked against the old
state. To be checked with the Kieback&Peter and LOYTEC controllers at the pilot site.
