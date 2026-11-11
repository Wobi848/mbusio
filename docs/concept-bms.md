# Concept: a simple, free, web-based building management system

Date: 11.11.2026 · Status: **draft, idea stage** · Working title: **bmsio**

A building management system (BMS) in the spirit of Avelon: everything in the browser, plant
graphics that look good, engineering that is fast. The difference: **free and open source (MIT)**,
self-hosted, and **simple enough that one person can set up a small site in an afternoon**.

This would be its own project and repository. mbusio stays a standalone tool and becomes the
first driver (see section 10).

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
  │  drivers: M-Bus, Modbus, BACnet/IP,│
  │           MQTT, later KNX, OPC UA  │
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
| **Discovery** | Scans BACnet/IP (Who-Is, object list), Modbus (from a device profile), M-Bus (mbusio's scan), MQTT topics. Shows what was found and what is new. |
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
| Drivers | M-Bus (from mbusio), Modbus (`tokio-modbus`), MQTT (`rumqttc`), BACnet/IP, later KNX and OPC UA (`opcua` crate) | BACnet/IP is required for a real BMS. The Rust crates are still thin; if needed, a proven C stack through FFI. |
| Frontend | **TypeScript + Svelte 5** | Fine-grained updates for hundreds of live values on one graphic; built to static files and embedded in the binary. |
| Graphics | **SVG**, symbols as SVG files with CSS states, editor on maxGraph or own | Section 6. |
| Live data | WebSocket, subscription per open graphic | Only the values on screen are sent. |
| Reports | HTML → PDF in the browser (print) | No PDF library on the server, as in mbusio. |

## 10. Relation to mbusio

mbusio **stays** a standalone tool for M-Bus commissioning and service. bmsio reuses:

- the **M-Bus driver** and decoder (as a library crate split out of mbusio),
- the **data point idea** (stable number, quality, unit, how values are passed on;
  [konzept-ausgaenge.md](konzept-ausgaenge.md), [konzept-dauerbetrieb.md](konzept-dauerbetrieb.md)),
- the **simulator**, the **report** and the **UI principles** (one next step, tables for mass
  work, keyboard; [konzept-oberflaeche-neu.md](konzept-oberflaeche-neu.md)),
- the **languages** set-up ([concept-languages.md](concept-languages.md)).

Nothing is removed from mbusio for this.

## 11. Phases

| Phase | Content | Done when |
|---|---|---|
| **0 Model** | Data model, tags subset, SQLite schema, API, M-Bus + Modbus drivers | Points of a test bench are read and kept with history. |
| **1 Viewer** | Live graphics from 3–5 built-in templates (AHU, heating circuit, boiler, meters), tag binding, trends | A real small plant runs on it, read-only. |
| **2 Engineering** | Discovery, mapping rules, point list table, apply templates, versions | A new site is set up without editing a file by hand. |
| **3 Editor** | Graphic editor, symbol library, own templates, template diff | An engineer builds a new plant type in the browser. |
| **4 Operation** | Writing, alarms, schedules, users and rights, audit log | It can run a building without a second system. |
| **5 BACnet + hub** | BACnet/IP read/write, hub across sites | Multi-site customer. |

BACnet/IP could move before phase 4 if the first real site needs it; it decides whether bmsio
is usable in most commercial buildings.

## 12. Open questions

- **Name** of the project and repository.
- **BACnet stack:** own Rust implementation or a C stack through FFI?
- **Editor:** maxGraph or own SVG editor? Decide after a one-week prototype of both.
- **Symbol style:** which symbol conventions do the engineers in the target market expect?
  To be reviewed by someone from the trade, like the languages.
- **First pilot site:** which building, which controllers, which protocols?
- **Licence of templates and symbols** in a shared library (MIT as well, or CC BY?).

## 13. Existing free tools, and why another one

| Tool | Strength | Why not enough here |
|---|---|---|
| **Home Assistant**, **openHAB** | Huge driver choice, free | Made for homes: no plant templates, no BMS-style point lists, alarms or commissioning. |
| **FUXA** (web SCADA, MIT) | Web-based SVG editor, Modbus/BACnet/OPC UA | General SCADA: graphics are bound to addresses, no template-to-many-plants workflow, no semantic model. Worth studying for its editor. |
| **Node-RED** | Flexible glue | A tool for integrators, not an operator interface. |

The gap bmsio fills: **semantic model + templates + engineering in the browser**, free.
