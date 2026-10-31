# mbusio

**English** · [Deutsch](README.de.md)

M-Bus master with a web UI for commissioning and service: find meters, find and fix problems, read
them, document the job. A single file for Windows, Linux and Raspberry Pi, no installation; the UI
runs in the browser, phones included.

## How it works

The UI is a tool: a header, areas as tabs, below them a list on the left and details on the right.
What an element is and what state it is in is shown on the element itself; the next step is always
exactly one button (see [docs/konzept-oberflaeche-neu.md](docs/konzept-oberflaeche-neu.md), German).

- **Header:** project › line, connection with exactly the matching action (set connection, connect,
  change), the line's **Continuous operation** switch with its plan, "N in continuous operation"
  across all projects, ⚙ (language, theme, keyboard shortcuts, backup).
- **Areas** (keys 1–6) in three groups:
  - set up: **Site** (projects, gateways and lines, plant status, report, export/import)
  - the workflow: **Scan** (search bar, hits with their state in words, resolve and fix duplicate
    primary addresses, remember new meters), **Meters** (list, edit in place, multi-select, read,
    history, replacement, import list, generate names), **Pass on** (MQTT and Modbus outputs with a
    switch, datapoints)
  - helpers: **Tools** (one target for reading, address, baud rate, reset, telegrams), **Simulator**
- **Status bar:** what the bus is doing, issues, outputs. The **next step** is the button right of the tabs. Click
  the bus to open the **bus console** (live telegrams; click one to have it explained in Tools),
  click the issues to see them in Scan.
- **Keyboard:** ↑↓ in lists, space selects, Ctrl+A all, Enter, Escape, `/` searches, `R` reads, `S`
  scans, `?` shows all keys. No browser pop-ups for confirmations: they appear at the button.

## Features

### Connection and search

- Serial port (COM port, `/dev/ttyUSB0`, …) or M-Bus gateway on the network
  (`tcp://192.168.1.50:10001`), 300–38400 baud, 8E1
- Primary scan (range, retries, estimated maximum duration) and secondary scan (search by ID)
- Buses with mixed baud rates: check where devices answer and scan only there
- Sortable hit list; secondary address shown decoded or as 16 hex characters

### Find and fix problems

- Detects collisions, duplicate primary addresses, duplicate IDs (a note if only the manufacturers
  differ), meters on a different baud rate and meters without usable data
- "Why?" names the cause in plain words (checksum, truncated, wrong address, error reported by the
  meter …) with a tip on what to do
- "Change PA" finds a free address on the bus; "Change baud rate" switches to the bus baud rate
- "Fix all": a plan to tick off; every change is verified on the meter and logged
- Safety checks: no address change to an occupied address or when several meters answer; reading
  via a primary address shared by several meters is refused

### Meters, reading, history

- Reads all values with unit, storage number, tariff; multi-telegram answers; variable and fixed
  data structure (old meters), error telegrams from the meter
- Meter list per line, remembered **by secondary or primary address** (set per line, no mixed lists)
- Sortable by column, search and filters (e.g. faulty, unnamed)
- "Read all", also with different baud rates on one bus
- History per meter as chart or table with difference and consumption per day, as CSV
- Find and switch baud rate, application reset, send raw telegrams (experts)

### Fill the meter list fast

- **Import list:** the customer's or installer's list as a CSV file, or copy the cells in Excel and
  paste them. Columns are recognised (ID/meter number, name, location/flat, note, PA) and can be
  assigned. Matching is by the **ID on the name plate**: missing meters are created, existing ones
  named; empty cells change nothing. mbusio shows beforehand what is new and what changes, and warns
  about IDs not found in the last scan or already on another line
- **Export:** meter list as CSV for Excel (ID, PA, manufacturer, medium, name, location, note,
  secondary address, last value), e.g. to have it filled in and import it again
- **Naming:** name, location and note directly in the table; Enter jumps to the next meter, saved
  immediately. Location suggestions and "same as above"
- **Generate names** from a pattern, e.g. `{ort} {medium}` → "Flat 3 Cold water" (placeholders
  `{ort}` = location, `{medium}`, `{id}`, `{pa}`, `{nr}`, `{name}`), for all or only unnamed meters

### Scheduled reading (continuous operation)

- **Read plan** per project, line or meter: off, monthly, daily, hourly, every 15 min. Lines and
  meters inherit the plan from above unless they have their own. **The default is off everywhere**;
  reading only starts once the line's **continuous operation** is switched on (switch in the header
  or under Site)
- Terms on the clock grid; each line has its own connection, lines run in parallel. Manual work
  (scan, read, change address) always comes first, never "bus busy"
- Missed terms (mbusio off, line disconnected) are caught up once, with the real timestamp
- A failure is retried once; after 3 missed terms a meter counts as **disturbed**. If a meter
  refuses (busy, too many readings) it is paused until the next day
- Warning when a line's planned readings no longer fit into the bus time
- **Clock:** without a time server (NTP) or RTC module the time counts as unconfirmed; values are then
  flagged "time unsure". "Take over time" in the plant status uses the clock of the laptop or phone
- **Storage:** scheduled readings are stored only as the raw telegram (about 100–250 bytes instead
  of ~2 KB) and decoded when shown; timestamps also in UTC. Readings are never deleted on their own;
  optionally per project "History: after 30/90/365 days keep 1 value per day"
- **Storage protection** (settings ⚙, on by default): when space runs low, mbusio thins out by
  itself, first readings older than 365 days, if needed older than 90 days, to one per meter and day
  (the last one, i.e. the end-of-day reading billing needs), and frees the space in the database. If
  it is still almost full, new readings are no longer stored (meter list and display keep working)
  so database and device stay intact
- The database survives crashes and power cuts (SQLite WAL, transactions); after a restart it carries
  on by itself
- Concepts (German): [docs/konzept-dauerbetrieb.md](docs/konzept-dauerbetrieb.md), outputs in
  general: [docs/konzept-ausgaenge.md](docs/konzept-ausgaenge.md)
- Node-RED nodes (pick project › … › datapoint, no broker): folder [node-red/](node-red/), on npm as
  [`node-red-contrib-mbusio`](https://www.npmjs.com/package/node-red-contrib-mbusio)
- HTTP API (everything the UI can do, also for your own programs): [docs/api.md](docs/api.md)

### Pass on: datapoints and outputs

Everything is in the **Pass on** tab: at the top the **outputs** (MQTT, Modbus) with state and
settings, below them the **datapoints** as a list (active line or whole project) with details on the
right and multi-select. Values are chosen in the Meters area at the meter.

- **Log** (button at the outputs, or "Log of this datapoint"): what went to MQTT, what is waiting and
  what was dropped (with the reason), filtered by period, result, datapoint and search, as CSV.
  30 days. For Modbus, the output's row shows which control system polled last.
- Per meter **"Choose values"**: tick any values with their current reading; the main value is
  suggested. Each chosen value is a datapoint; one tick per output. Name, unit and conversion (e.g.
  Wh → kWh with factor 0.001) can be set
- **Template:** the same selection for all meters of the same medium on the line, optionally also for
  meters added later, and optionally straight to MQTT
- **Stable key** from the meaning of the value (quantity, unit, storage, tariff, function), not from
  its position in the telegram; **stable number** per project, never reused
- **Quality** per datapoint: good, stale (older than 3 terms), disturbed (meter does not answer),
  invalid (meter reports an error, or the counter went down)
- **Replaced …** (when editing a meter): the new meter is read, the counters continue seamlessly
  (old final reading → new start reading), listed in the report
- **Replacement detected:** if a meter with another ID answers on the same primary address after
  mbusio has already read the old one, the replacement is recorded automatically (counters continue)
  and shown in the list as "replacement detected" to confirm. If only the imported list was wrong,
  mbusio takes over the ID with a note in the log. If a meter reports an error itself (status byte,
  error flags, low battery), this is shown at the meter
- Mark several and switch them active/inactive, set or clear outputs together
- Export as **CSV**, edit in Excel and import again; mbusio shows what changes first and applies it
  only after confirmation. The list also serves as documentation for the integrator; the report has a
  "Datapoints" page
- Passing on per datapoint: on every reading, only on change, or once a day

### MQTT

- Pass on → Outputs → MQTT → **Settings**: broker, port, user/password, TLS (with your own CA
  certificate or, for self-signed brokers, without verification), client ID and base topic
- Per datapoint the **MQTT** tick; the topic is created when switched on from the **topic pattern** in
  the settings (default `{projekt}/{gateway}-{linie}/{zaehler}/{wert}`, i.e.
  project/gateway-line/meter/value, e.g. `…/whg-1/energy`; also `{id}`, `{pa}`, `{ort}`, `{medium}`,
  `{nr}`) and then stays the same even if names change; can be overridden individually. **"Set all
  topics from pattern …"** recomputes all topics of the project, with preview
- **Topic list for the integrator** as CSV (Excel) or JSON: per datapoint the full topic, name, unit,
  meter, ID, PA, location, line; the JSON also has the status topic, QoS, retained and the message
  format
- JSON message, retained, QoS 1:
  `{"value": 123456, "unit": "kWh", "time": "2026-10-17T19:03:12Z", "quality": "good", "name": "Flat 1 · Energy", "no": 1}`,
  with an unconfirmed clock also `"time_unsure": true`. A change of quality alone (meter stops
  answering, value stale) is published too
- `mbusio/status` = `online`, `offline` on a crash or power cut (last will)
- **Node-RED** (no broker): `GET /api/nodered/tree` (project › gateway › line › meter › datapoints with
  value) and `GET /api/nodered/stream?project=1` (server-sent events: the current state first, then
  new values and changed quality); read only, with `X-Token` like the rest of the API (see
  [docs/konzept-node-red.md](docs/konzept-node-red.md))
- **Home Assistant:** tick "Home Assistant (MQTT discovery)" in the settings: all datapoints with the
  MQTT tick appear in Home Assistant by themselves, one device per meter, with unit and matching type
  (energy, water, gas, temperature …, for the energy dashboard). Disturbed values show as
  "unavailable". Do not change the client ID afterwards (see
  [docs/konzept-home-assistant.md](docs/konzept-home-assistant.md))
- **Buffer:** if the broker is unreachable, mbusio collects the messages (up to 7 days, also across a
  restart) and delivers them afterwards in the right order; a message is deleted only once the broker
  has acknowledged it
- Optionally the **time from the control system**: a message on `mbusio/time` (seconds or
  milliseconds since 1970, or `2026-10-17T12:00:00Z`)
- The password never appears in the UI, the log or the project export

### Modbus TCP

- mbusio is a Modbus TCP server: control system, PLC or Loxone read the values as registers (read
  only, functions 03 and 04 return the same registers, any unit ID)
- Pass on → Outputs → Modbus → **Settings**: switch on, address of the network interface (Modbus has
  no access control), port (502; on Linux a port below 1024 needs privileges, otherwise e.g. 1502),
  project, word order (ABCD or CDAB)
- Per datapoint the **Modbus** tick (several: select → **Modbus on**); start register =
  (number − 1) × 10, never changes
- 10 registers per datapoint: value as float64 (+0), float32 (+4), int32 × 100 (+6), quality (+8:
  0 good, 1 stale, 2 disturbed, 3 invalid, 4 no value), age in minutes (+9); from 60000 the overall
  state (heartbeat, number disturbed, number stale)
- **Register list for the integrator** as CSV or JSON, as for MQTT
  (see [docs/konzept-modbus.md](docs/konzept-modbus.md))

### Documentation

- Report per project with customer, site, technician, all lines and meters, optionally with all
  readings, open issues (can be hidden) and changes made; print or save as PDF
- **Export for billing** (Pass on → Export for billing; previous month, quarter, half-year, year or
  from–to, comparison with up to 10 previous years, by hand or by e-mail): meter readings on the
  reference date per meter as CSV for Excel, one row per meter and value with start reading, end
  reading and consumption, or as a time series (reading per day or month end). Defaults: previous
  month, main value per meter; period, further values and decimal separator can be set, saved per
  project, with preview. Values come from the stored readings, so also retroactively for values that
  were never chosen as datapoints. Reading on the reference date: the due-date value the meter stored
  itself, otherwise the last reading before; the source is shown next to it, gaps and unsure times as
  notes. A meter replacement within the period is taken into account. Notes also for a detected but
  unconfirmed replacement, not read in the period, not answering, disturbed in the period (from–to)
  and meter errors; individual lines selectable; summed up above the preview ("2 × not answering ·
  1 × replacement"). **Automatic:** after the end of the period (at the start of the month or in
  January, after the first reading) mbusio stores the file in the data folder under
  `exporte/<project-no>/`; missed periods (device off) are caught up, the files can be downloaded at
  the export. **By e-mail:** with recipients per project every new file is sent as an attachment (own
  small SMTP client, STARTTLS/TLS, mail server once per device, test mail button); if it fails, the
  reason is shown at the file and mbusio retries for 7 days
- Export readings as CSV (semicolon, Excel) or JSON; save projects as a file ("Save project") and
  import them (e.g. from the PC to the Raspberry Pi)
- Telegram log; click any TX/RX line to have the telegram explained byte by byte

### General

- Everything is stored (SQLite, one file `mbusio.db`)
- German and English UI, light/dark, adapted for phones
- Command line for scripts with JSON output
- Simulator with templates (real-world faults, all OK, apartment building, mixed baud rates) and your
  own freely assembled installations

## Hardware

A plain USB-serial adapter is not enough: M-Bus needs a **level converter / M-Bus master** that
supplies the bus voltage. mbusio talks to it via its serial port, or to an M-Bus gateway via TCP.
Small USB-powered masters typically supply about 10 meters; for more, use one with its own power
supply (e.g. 12 V or 24 V).

## Getting started

Ready-made programs are on the [releases page](https://github.com/Wobi848/mbusio/releases):
`mbusio-windows-x86_64.exe`, `mbusio-linux-x86_64`, `mbusio-raspberrypi-arm64` (Raspberry Pi OS
64-bit) and `mbusio-raspberrypi-armv7` (32-bit). On start the browser opens `http://localhost:8080`;
the last active line reconnects automatically.

```sh
mbusio                                  # http://localhost:8080
mbusio --port SIMULATOR                 # try it without hardware
mbusio --lan --token SECRET             # reachable from the LAN, protected by a token
```

### Windows without admin rights

No installation needed: put the .exe in any folder and double-click it. At the SmartScreen warning
choose "More info → Run anyway". A second double-click only opens the browser again.

- **Portable** (e.g. USB stick): put a folder `mbusio-data` next to the .exe, then all projects are
  stored there instead of `%APPDATA%\mbusio`.
- The **driver of the USB M-Bus master** needs admin rights to install. Windows recognises many
  adapters (FTDI) by itself; a COM port must appear in the Device Manager.
- Start without `--lan`, otherwise the Windows firewall asks for permission (admin rights).

### Windows service

For continuous operation (reading and passing on regularly) mbusio must run even when nobody is
logged in. Started in a window it stops as soon as the window is closed; the plant status then shows
this in orange.

1. Put `mbusio-dienst-einrichten.cmd` from the release next to the .exe and double-click it. Windows
   asks for admin rights; then choose whether the UI should be reachable from the network (then with
   an access key). Without the file, in a command prompt as administrator:
   `mbusio-windows-x86_64.exe --install-service [--lan --token SECRET]`
2. mbusio then runs as the service "mbusio": starts with Windows, without login and without a
   window, restarts after a crash. Double-clicking the .exe only opens the browser.

- The **program** is then in `C:\Program Files\mbusio`, the **data** in `C:\ProgramData\mbusio`. On
  the first setup the existing database is copied there (the original stays).
- **Update:** set up the new .exe the same way; data stays.
- **Remove:** `mbusio-dienst-entfernen.cmd` or `--uninstall-service`; program and data stay.
- If the service does not start: `C:\ProgramData\mbusio\mbusio.log`.
- The service runs as `LocalService` (few privileges). While a plan is running the PC does not go
  to sleep.
- **Turn off USB power saving**, otherwise Windows switches off the M-Bus converter: Power Options →
  Advanced settings → USB settings → disable "USB selective suspend", and in the Device Manager untick
  "Allow the computer to turn off this device" for the USB converter.

### Raspberry Pi as a service

```sh
sudo usermod -aG dialout pi        # access to /dev/ttyUSB0, log in again afterwards
sudo install -m 755 mbusio-raspberrypi-arm64 /usr/local/bin/mbusio
```

```ini
# /etc/systemd/system/mbusio.service
[Unit]
Description=mbusio
After=network.target

[Service]
ExecStart=/usr/local/bin/mbusio --lan --token SECRET --no-browser --data-dir /home/pi/mbusio-data
User=pi
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl enable --now mbusio
```

Then open `http://<IP of the Pi>:8080` in the browser of your phone or laptop.

**Is the Pi enough?** Run `mbusio lasttest` before deployment: it creates a site with 60 meters and
3 years of readings in its own folder (plan every 15 min, every reading in the last year, one per
day before that as with storage protection), starts mbusio on it and shows how long status, meter
list, history, report, billing, export and backup take. The folder is deleted afterwards; the load
test never touches real data. Size can be chosen: `mbusio lasttest --meters 120 --years 5 --minutes 15`.
For comparison on a PC: 480 MB database, status 1 ms, history 60 ms, billing 30 ms, backup download 1 s.

### Options

| Option | Meaning |
| --- | --- |
| `--http-port 8080` | port of the web UI |
| `--lan` | also reachable from the network (otherwise only `127.0.0.1`) |
| `--token SECRET` | access key, recommended with `--lan` (also via `MBUSIO_TOKEN`) |
| `--port`, `--baud` | connect to this port on start instead of the last active line |
| `--timeout-ms` | answer timeout, `0` = automatic from the baud rate |
| `--data-dir` | folder for `mbusio.db` (default: `mbusio-data` next to the program if present, otherwise `~/.local/share/mbusio` or `%APPDATA%\mbusio`) |
| `--no-browser` | do not open the browser on start |
| `--list-ports` | list serial ports |
| `--untrusted-clock` | never treat the system clock as confirmed (all values "time unsure" until the time is taken over in the browser) |

### Data and backup

All projects, meter lists, readings (with history), last scans, changes and settings (MQTT, Modbus,
e-mail including passwords) are in **one file** `mbusio.db` (SQLite).

**Device backup:** Settings (⚙) → Backup → "Download" delivers this file as a consistent copy, even
while mbusio is reading. If the device fails: start mbusio on the new one, ⚙ → Backup → "Restore …";
the confirmation shows projects, meters and period; mbusio then restarts and everything runs again
(lines connect, plan and outputs as before). The previous state is kept as `mbusio.db.vorher`. An
older mbusio refuses a backup from a newer version. The file contains the passwords: keep it safe.
Single projects can still be saved as a JSON file via "Save project" (without device settings). A
`meters.json` from version 0.3 is taken over into the project "Standard" on first start.

**Automatic backup** (Pass on → Automatic backup): daily at a set time to **another** device, so a
failure of the device itself does not take the backup with it. Target: a folder (USB stick or a
network drive mounted by the operating system, see below) or mbusio's own small FTP/FTPS client. Only
the last few backups of this device are kept at the target (configurable); mbusio deletes older ones
there itself, nothing else in the folder. With a **backup password** mbusio encrypts the file
(AES-256-GCM, key from PBKDF2-SHA256); the same password then also encrypts "Download" by hand.
Without the password the file can no longer be read: keep it like a key. If a backup fails, mbusio
reports it in the messages (status bar, warning e-mail).

**Network drive (SMB) as target:** mbusio has no SMB client of its own; the operating system does
this.
- **Windows:** enter the share (e.g. `\\server\share`) directly as the folder path.
- **Raspberry Pi / Linux:** mount it once in `/etc/fstab`, then enter the mount point as the folder:
  ```
  //server/share /mnt/backup cifs credentials=/etc/mbusio-smb-credentials,uid=pi,iocharset=utf8,x-systemd.automount 0 0
  ```
  plus `/etc/mbusio-smb-credentials` (readable by root only: `sudo chmod 600 /etc/mbusio-smb-credentials`):
  ```
  username=...
  password=...
  ```
  Then `sudo mount -a` and enter `/mnt/backup` as the folder in the settings.

### Command line

Like the libmbus tools, but one program. Result as JSON on stdout, telegram log on stderr.

```sh
mbusio --port /dev/ttyUSB0 scan --from 1 --to 50
mbusio --port /dev/ttyUSB0 scan-secondary
mbusio --port /dev/ttyUSB0 bus-bauds              # on which baud rates do devices answer?
mbusio --port /dev/ttyUSB0 scan --all-bauds       # scan on all of these baud rates
mbusio --port /dev/ttyUSB0 scan --bauds 2400,9600 # only on these baud rates
mbusio --port /dev/ttyUSB0 read 12345678          # primary address, ID or secondary address
mbusio --port /dev/ttyUSB0 set-address 12345678 5
mbusio --port /dev/ttyUSB0 detect-baud 5
mbusio --port /dev/ttyUSB0 switch-baud 5 9600
mbusio --port /dev/ttyUSB0 reset 5
mbusio --port tcp://192.168.1.50:10001 raw "10 40 05 45 16"
```

## Simulator

In the **Simulator** tab you can put together virtual installations without hardware: for practice,
testing and demos. The tab only appears while the active line uses a simulation; otherwise open it
via **Tools → Simulator**. Built-in templates:

| Simulation | Connection | Contents |
| --- | --- | --- |
| Real-world faults | `SIMULATOR` | 20 meters with faults from practice (table below) |
| All OK – 10 meters | `SIMULATOR:ok10` | heat, cold and hot water, electricity, gas, PA 1–10 |
| Apartment building – 30 meters OK | `SIMULATOR:mfh30` | 10 flats with heat, cold and hot water |
| Mixed baud rates | `SIMULATOR:bauds` | 8 meters on 2400, 9600 and 300 baud, otherwise OK |

**Copy** a template, then add meters freely (one at a time or "+ several"), with primary address,
ID, manufacturer, medium, own baud rate and, if wanted, a fault: wrong checksum, truncated telegram,
"busy", wrong address, loose contact, "too many readings". Duplicate addresses or IDs are shown while
editing (sometimes they are intended). **"Use for active line"** connects the active line to the
simulation (`SIMULATOR:<name>`); different lines can simulate different installations. Changes apply
immediately after saving.

The simulated counters (energy, volume, water, electricity) rise with the time since 1 Oct 2026
according to the system clock, so also across a restart of mbusio, like real meters.

Layout of `SIMULATOR` (real-world faults):

| Address | Meter | Special |
| --- | --- | --- |
| 1 | heat meter, ID 12345678 | answer in two telegrams |
| 2 | electricity meter, ID 87654321 | tariffs, voltage, current, firmware text |
| 3 | water meter, ID 30000001 | |
| 0 | two water meters, ID 12345699 and 55500011 | collision, reachable only by secondary address |
| 4 | water meter, ID 40000004 | wrong checksum |
| 5 | water meter, ID 50000005 | truncated telegram |
| 6 | water meter, ID 60000006 | runs at 9600 baud (or 2400 baud when connected at 9600) |
| 7 | old heat meter, ID 70000007 | fixed data structure (CI 73) |
| 9 | water meter, ID 90000009 | ancient, runs at 300 baud |
| 10 | two heat meters, ID 10000010 and 10000011 | same primary address |
| 11 | water meter, ID 11000011 | reports "busy" (error telegram CI 70) |
| 12 | water meter, ID 12000012 | answers with the wrong address (13) |
| 14, 15 | two water meters with the same ID 77777777 | different manufacturers |
| 16 | water meter, ID 16000016 | loose contact: only every second telegram arrives |
| 17, 18 | two water meters with the same ID 88888888 | same manufacturer: a real fault |
| 19 | water meter, ID 19000019 | "too many readings" after 2 readings (error telegram 09) |

Baud rate changes and application reset are acknowledged like on real meters.

## First test with real hardware

1. Create a line, enter the connection (e.g. `COM3` or `/dev/ttyUSB0`), baud rate 2400
2. In Scan, "Check": on which baud rates do devices answer?
3. Read a known meter by its primary address under "Tools"
4. If there are problems: open the telegram log, click the lines (analysis) and send the TX/RX lines

## Known limitations

- If two meters answer with an acknowledgement (`E5`) at exactly the same time, it sometimes looks
  like a single answer. The secondary scan is therefore the safe way to find all meters.
- Telegrams with reversed byte order (CI 76/77) are not supported.
- A meter's ID cannot be changed; only the manufacturer's software can do that.

## Development

```sh
cargo test                     # frames, decoder, analysis, scans, addressing, database, schedule
cargo run -- --port SIMULATOR
cargo run -- --time-scale 48   # time lapse for long-running tests: one day in 30 minutes
```

Code comments and the older concepts in `docs/` are in German; commit messages and new concepts are in English.

Structure:

- `src/mbus/frame.rs`: build and check telegrams (EN 13757-2)
- `src/mbus/decode/`, `tables.rs`: decode records (EN 13757-3): variable structure (mod), fixed
  structure and meter error telegrams (fixed), raw bytes to values (values)
- `src/mbus/master/`: connection, reading, causes of faults (mod), types (types), log (log), primary
  and secondary scan, baud rates on the bus (scan), address, baud rate, reset (config)
- `src/mbus/analyze/`: telegram analysis byte by byte, link layer (mod) and application layer
  (application)
- Tests live in `tests.rs` or `tests/` next to the module, large ones split by topic
- `src/link.rs`: serial port and TCP; `src/sim/`: simulator (bus, meter, scenarios, store), all
  behind the same `Link` trait
- `src/db/`: database by area (types, schema with migrations, projects, lines, meters, readings,
  schedule, report)
- `src/line.rs`: line service, one connection and queue per line (manual work before plan)
- `src/datapoint/`: datapoints: mod, key (key of a value), quality, values (new values from a
  reading), select, edit, list, live (state for Node-RED), outputs (outputs in general with the
  `Output` interface, one module per output: mqtt, home_assistant, modbus), templates, exchange
  (meter replacement), csv, project_file
- `src/main.rs`: start, web server and services; `src/cli.rs`: command line (JSON)
- `src/changeset.rs`: change set for "preview → apply" (imports, templates)
- `src/meterlist.rs`: import meter list (by ID), names from a pattern, CSV export
- `src/modbus/`: Modbus TCP server: settings and service (mod), register image (image), requests
  and answers (protocol)
- `src/mqtt/`: MQTT output: service (mod), settings and topics (config), settings and queue in the
  database (store); own small MQTT 3.1.1 client with TLS via rustls (client)
- `src/backup/`: device backup and restore (mod), encryption (crypt), automatic backup (auto), FTP
  client (ftp); `src/textproto.rs`: shared line-based TCP/TLS connection for SMTP, FTP, MQTT
- `src/plan.rs`, `src/scheduler/`: read plan and schedule (mod: what is due; reading: scheduled
  reading; upkeep: storage, thinning, clock, backup); `src/clock.rs`: clock and whether it is
  confirmed; `src/storage.rs`: protection against full storage
- `src/service/`: running in the background: detection (service, systemd), data takeover, log (mod);
  Windows service (win, Windows only)
- `src/web/`: HTTP API: mod (router, access key, files), state, api (errors, downloads, bus work),
  then by area (bus, scan, tools, meters, projects, schedule, datapoints, outputs, nodered,
  simulator, backup); `web/`: UI and report (embedded in the binary)
- `src/i18n/`: translations of the quantities, language per request; background work follows the
  language of the UI (last request with `X-Lang`); log messages are created with `both` in both
  languages and appear in the language that asks; all UI and report texts in `web/js/texts.js` or
  next to the area
- `web/index.html` and `web/app/`: UI as ES modules without a build step: `app.js` (start, tabs,
  settings), `app-kopf.js` and `app-statusleiste.js`, `store.js` (state), `view.js` (building blocks),
  `keys.js` (keyboard), `data.js` (loading), `buslog.js`, `sortieren.js` (sortable columns), then per
  area `<area>.js` with `<area>.css` (scan, zaehler, weitergabe, anlage, werkzeuge, simulator), next
  to them `<area>-texte.js` and separate parts (e.g. `zaehler-verlauf.js`)
- `web/js/`: shared by UI and report (`report.html`): `base.js`, `texts.js`, `icons.js`, `theme.js`,
  `scanlogic.js`, `listlogic.js`, `plot.js`; `web/css/`: `tokens.css`, `meter.css`, `plot.css`
- `tests/ui/`: browser tests of the UI (see `tests/ui/README.md`)

The decoder is a custom implementation and is tested against the reference telegram from the M-Bus
documentation. Feature set like libmbus: primary/secondary scan, multi-telegram reading, set primary
address, switch baud rate, application reset, raw telegrams, serial and TCP connection, variable and
fixed data structure.

## License

MIT, see [LICENSE](LICENSE).
