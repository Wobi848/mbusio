# mbusio

M-Bus-Master mit Weboberfläche: Zähler suchen, auslesen, adressieren und exportieren.
Eine einzige Binärdatei für Windows, Linux und Raspberry Pi, die Oberfläche ist eingebaut.

*English below.*

## Funktionen

- Serielle Schnittstelle wählen (COM-Port, `/dev/ttyUSB0`, …), Baudrate 300–38400, 8E1
- **Primärscan** (Adressbereich wählbar, mit Wiederholungen); erkennt Kollisionen, wenn mehrere Zähler dieselbe Adresse haben
- **Sekundärscan** (Wildcard-Suche über die ID), findet auch Zähler, die alle auf Adresse 0 stehen
- **Auslesen**: alle Werte mit Einheit, Hersteller, Medium, Speicher-/Tarifnummer; Multi-Telegramm-Antworten werden zusammengeführt
- **Primäradresse setzen** über Primär- oder Sekundäradresse, mit anschliessender Kontrolle
- **Export** als CSV (Semikolon, Excel-tauglich) oder JSON
- **Telegramm-Log** mit allen gesendeten und empfangenen Bytes, wichtig für die Fehlersuche an echter Hardware
- **Simulator** mit sieben virtuellen Zählern zum Testen ohne Hardware
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
| `--port`, `--baud` | beim Start direkt verbinden |
| `--timeout-ms` | Antwort-Timeout, `0` = automatisch aus der Baudrate |

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
- `src/link.rs`: serielle Schnittstelle; `src/sim.rs`: Simulator, beide hinter demselben `Link`-Trait
- `src/web.rs`: HTTP-API; `web/`: Oberfläche (wird in die Binärdatei eingebettet)
- `src/i18n.rs`: Übersetzungen der Messgrössen; UI-Texte in `web/app.js`

Der Decoder ist eine Eigenentwicklung. Das Crate `m-bus-parser` war vor Version 1.0 und
auf mehrere Crates mit wechselnder API verteilt, deshalb habe ich es nicht übernommen.
Getestet wird der Decoder gegen das Referenztelegramm aus der M-Bus-Dokumentation.

---

## English

M-Bus master with a built-in web UI: scan for meters, read them, set addresses, export.
One binary for Windows, Linux and Raspberry Pi.

- Primary scan (with collision detection) and secondary scan (wildcard search)
- Read all values with units, manufacturer and medium; multi-telegram responses are merged
- Set the primary address (via primary or secondary address), verified afterwards
- CSV/JSON export, telegram log, built-in simulator, UI in German and English

```sh
mbusio                                  # http://localhost:8080
mbusio --port SIMULATOR                 # try it without hardware
mbusio --lan --token SECRET             # reachable from the LAN, protected by a token
```

You need an M-Bus level converter / master. A plain USB-serial adapter is not enough.
