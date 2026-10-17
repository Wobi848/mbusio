# mbusio

M-Bus-Master mit Weboberfläche: Zähler suchen, auslesen, adressieren und exportieren.
Eine einzige Binärdatei für Windows, Linux und Raspberry Pi, die Oberfläche ist eingebaut.

*English below.*

## Funktionen

- Serielle Schnittstelle (COM-Port, `/dev/ttyUSB0`, …), Baudrate 300–38400, 8E1, oder
  **M-Bus-Gateway im Netzwerk** (`tcp://192.168.1.50:10001`)
- **Primärscan** (Adressbereich wählbar, mit Wiederholungen); erkennt Kollisionen, wenn mehrere Zähler dieselbe Adresse haben
- **Sekundärscan** (Wildcard-Suche über die ID), findet auch Zähler, die alle auf Adresse 0 stehen
- **Auslesen**: alle Werte mit Einheit, Hersteller, Medium, Speicher-/Tarifnummer; Multi-Telegramm-Antworten werden zusammengeführt
- **Primäradresse setzen** über Primär- oder Sekundäradresse, mit anschliessender Kontrolle;
  bricht ab, wenn die neue Adresse belegt ist oder mehrere Zähler auf der alten stehen
- **Baudrate** eines Zählers suchen (300–38400 Bd durchprobieren) und am Zähler umstellen
- **Application Reset** senden
- **Zählerliste**: Zähler merken, mit Name, Ort und Notiz versehen, gesammelt auslesen
  (auch mit unterschiedlichen Baudraten auf einem Bus), als JSON exportieren und importieren
- **Telegramm senden** (Experten): beliebige Bytes senden, Antwort wird decodiert
- **Export** als CSV (Semikolon, Excel-tauglich) oder JSON
- **Telegramm-Log** mit allen gesendeten und empfangenen Bytes, wichtig für die Fehlersuche an echter Hardware
- **Kommandozeile** für Skripte, Ergebnis als JSON (siehe unten)
- **Simulator** mit neun virtuellen Zählern zum Testen ohne Hardware
- Oberfläche auf **Deutsch und Englisch**, auch auf dem Handy bedienbar

## Hardware

Ein normaler USB-Seriell-Adapter genügt nicht: M-Bus braucht einen **Pegelwandler / M-Bus-Master**
(z.B. USB-M-Bus-Master mit 3–60 Zählern), der die Busspannung liefert. mbusio spricht diesen
Wandler über seine serielle Schnittstelle an.

## Starten

Fertige Binärdateien gibt es unter *Actions → Build → Artifacts* bzw. bei Releases (Tag `v*`).

```sh
mbusio                      # Weboberfläche auf http://localhost:8080
mbusio --port SIMULATOR     # direkt mit dem Simulator verbinden
mbusio --port /dev/ttyUSB0 --baud 2400
mbusio --list-ports
```

| Option | Bedeutung |
|---|---|
| `--http-port 8080` | Port der Weboberfläche |
| `--lan` | auch aus dem Netzwerk erreichbar (sonst nur `127.0.0.1`) |
| `--token GEHEIM` | Zugriffsschlüssel für die API, empfohlen mit `--lan` (auch per `MBUSIO_TOKEN`) |
| `--port`, `--baud` | beim Start direkt verbinden (`--port tcp://IP:Port` für Netzwerk-Gateways) |
| `--data-dir` | Ordner für die Zählerliste `meters.json` (Standard: `~/.local/share/mbusio`, unter Windows `%APPDATA%\mbusio`) |
| `--timeout-ms` | Antwort-Timeout, `0` = automatisch aus der Baudrate |

### Kommandozeile

Wie die Tools von libmbus, aber ein Programm. Ergebnis als JSON auf stdout, Telegramm-Log auf stderr.

```sh
mbusio --port /dev/ttyUSB0 scan --from 1 --to 50
mbusio --port /dev/ttyUSB0 scan-secondary
mbusio --port /dev/ttyUSB0 read 12345678          # Primäradresse, ID oder Sekundäradresse
mbusio --port /dev/ttyUSB0 set-address 12345678 5
mbusio --port /dev/ttyUSB0 detect-baud 5
mbusio --port /dev/ttyUSB0 switch-baud 5 9600
mbusio --port /dev/ttyUSB0 reset 5
mbusio --port tcp://192.168.1.50:10001 raw "10 40 05 45 16"
```

### Raspberry Pi (headless)

```sh
sudo usermod -aG dialout $USER     # Zugriff auf /dev/ttyUSB0, danach neu anmelden
./mbusio-raspberrypi-arm64 --lan --token GEHEIM
```

Dann im Handy-Browser `http://<IP des Pi>:8080` öffnen. Als Dienst mit systemd:

```ini
# /etc/systemd/system/mbusio.service
[Unit]
Description=mbusio
After=network.target

[Service]
ExecStart=/usr/local/bin/mbusio --lan --token GEHEIM
User=pi
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

`arm64` ist für Raspberry Pi OS 64-Bit, `armv7` für 32-Bit.

## Simulator

Port `SIMULATOR` wählen. Belegung:

| Adresse | Zähler | Besonderheit |
|---|---|---|
| 1 | Wärmezähler, ID 12345678 | Antwort in zwei Telegrammen |
| 2 | Elektrizitätszähler, ID 87654321 | Tarife, Spannung, Strom, Firmware-Text |
| 3 | Wasserzähler, ID 30000001 | |
| 0 | zwei Wasserzähler, ID 12345699 und 55500011 | Kollision, nur per Sekundäradresse erreichbar |
| 4 | Wasserzähler, ID 40000004 | falsche Prüfsumme |
| 5 | Wasserzähler, ID 50000005 | abgeschnittenes Telegramm |
| 6 | Wasserzähler, ID 60000006 | läuft mit 9600 Bd (bzw. 2400 Bd, wenn mit 9600 verbunden) |
| 7 | alter Wärmezähler, ID 70000007 | feste Datenstruktur (CI 73) |

Baudratenwechsel und Application Reset werden wie bei echten Zählern quittiert.

## Erster Test mit echter Hardware

1. Verbinden mit 2400 Baud (manche Zähler: 300 oder 9600)
2. Telegramm-Log aufklappen
3. Einen bekannten Zähler über seine Primäradresse auslesen
4. Bei Problemen das Log (TX/RX-Zeilen) mitschicken

## Entwicklung

```sh
cargo test            # Frames, Decoder, Scans und Adressierung gegen den Simulator
cargo run -- --port SIMULATOR
```

Aufbau:

- `src/mbus/frame.rs`: Telegramme bauen und prüfen (EN 13757-2)
- `src/mbus/decode.rs`, `tables.rs`: Datensätze decodieren (EN 13757-3)
- `src/mbus/master.rs`: Scan, Auslesen, Adressierung
- `src/link.rs`: serielle Schnittstelle und TCP; `src/sim.rs`: Simulator, alle hinter demselben `Link`-Trait
- `src/project.rs`: Zählerliste
- `src/web.rs`: HTTP-API; `web/`: Oberfläche (wird in die Binärdatei eingebettet)
- `src/i18n.rs`: Übersetzungen der Messgrössen; UI-Texte in `web/app.js`

Der Decoder ist eine Eigenentwicklung. Das Crate `m-bus-parser` war vor Version 1.0 und
auf mehrere Crates mit wechselnder API verteilt, deshalb habe ich es nicht übernommen.
Getestet wird der Decoder gegen das Referenztelegramm aus der M-Bus-Dokumentation.

Funktionsumfang wie libmbus: Primär-/Sekundärscan, Auslesen mit mehreren Telegrammen,
Primäradresse setzen, Baudrate umstellen, Application Reset, Rohtelegramme, serielle und
TCP-Verbindung, variable und feste Datenstruktur. Nicht unterstützt: Telegramme mit
umgekehrter Byte-Reihenfolge (CI 76/77), die in der Praxis kaum vorkommen.

---

## English

M-Bus master with a built-in web UI: scan for meters, read them, set addresses, export.
One binary for Windows, Linux and Raspberry Pi.

- Primary scan (with collision detection) and secondary scan (wildcard search)
- Read all values with units, manufacturer and medium; multi-telegram responses are merged
- Set the primary address (via primary or secondary address), verified afterwards
- Detect and switch a meter's baud rate, application reset, raw telegrams
- Meter list with names and locations, read all at once (mixed baud rates supported)
- Serial port or network gateway (`tcp://host:port`), command line with JSON output
- CSV/JSON export, telegram log, built-in simulator, UI in German and English

```sh
mbusio                                  # http://localhost:8080
mbusio --port SIMULATOR                 # try it without hardware
mbusio --lan --token SECRET             # reachable from the LAN, protected by a token
```

You need an M-Bus level converter / master. A plain USB-serial adapter is not enough.
