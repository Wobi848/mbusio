# Übergabe an die nächste Session (Stand 2026-10-25)

Lies das zuerst. Repo: Wobi848/mbusio, Branch `claude/mbusio-context-w51dtz`, letzter Commit `81da159`.
Mit dem Nutzer **auf Deutsch** reden, kurz und direkt.

## Harte Regeln des Nutzers
- **Phase 4 (Modbus) NICHT machen.**
- **Kein Release v0.5.0**, bevor der 7-Tage-Test auf dem Raspberry Pi gelaufen ist (Strom weg, USB ab, kleine SD-Karte).
- KISS: wenig Code, wiederverwendbare Bausteine (web/js/ui.js, base.js), nichts doppelt, kein Spaghetti.
- Grössere Änderungen: erst Konzept (docs/konzept-*.md), dann bauen.
- Buttons müssen klar sichtbar sein: primär / `.tonal` (sekundär) / `.quiet`. Menüpunkte (`.menu-items > button`) sind immer quiet.
- Problemzähler: orange = Hinweis, Zähler bleibt übernehmbar (`tr.issue`), rot = Fehler (`tr.issue.fault`).
- Plan-Vorgabe „Aus“, 24-h-Zeit, batteriebetriebene Zähler nicht zu oft auslesen.

## Tests
- `cargo test` → 115 passed, 1 ignored.
- UI: `PLAYWRIGHT=$(ls -d ~/.npm/_npx/*/node_modules/playwright|head -1) MBUSIO_TEST_BROKER=127.0.0.1:18830 node tests/ui/run.mjs` → 12 Tests, alle grün.
- Server lokal: `cargo run -- --no-browser` (Port 8080; der Debug-Build liest web/ direkt von der Platte).
- Commits **ohne** KI-Hinweis (globale Regel des Nutzers); nach jedem fertigen Schritt committen und pushen. Nicht mergen ohne ausdrückliches OK.

## Zuletzt erledigt
- 25.10.: Ausgänge allgemein (Trait `Output`, docs/konzept-ausgaenge.md), Home Assistant MQTT Discovery
  (docs/konzept-home-assistant.md, mit echtem HA getestet und wieder entfernt), Signal `Db.changes`,
  Node-RED-Schnittstelle `/api/nodered/tree|stream` und Paket `node-red/` (node-red-contrib-mbusio,
  nicht veröffentlicht; Veröffentlichen nur nach Freigabe). CI wieder grün (Tests nahmen an, nicht als
  Dienst zu laufen; der Runner ist ein systemd-Dienst).
- Node-RED-Tests: `cd node-red && npm install && npm test`. Echter Durchgang: mbusio mit Simulator,
  Node-RED mit `ln -s` auf `node-red/`, Editor per Playwright (telemetry und tours in settings.js aus).
- `38380b7` Port-Auswahl: `link::list_ports()` liefert `{port,label}`, USB-Wandler mit Namen zuerst, leere `/dev/ttyS*` ausgeblendet, Simulatoren am Ende (Rust, CLI `--list-ports`, web/js/scan.js `loadPorts`). Abgeschlossen und getestet.
- Davor: Buttons (tonal/quiet), orange/rote Problemzeilen, MQTT-Topic-Muster + Liste für den Integrator. MQTT hat der Nutzer mit einem echten Broker getestet: Daten kommen bis MQTT an.

## Hardware
- **Relay MR003 USB** (CP210x) als `/dev/serial/by-id/usb-Silicon_Labs_Relay_M-Bus_Micro-Master_USB_7890-if00-port0`
  (fester Name, in der Linie „Test 1 › Linie 1“ eingetragen).
- Zähler: **Itron Wärme-/Kältezähler, ID 20395066, PA 3, 2400 Bd**. Auf dem Tisch ohne Wasser: Status 0x10,
  Leistung/Durchfluss als Fehlerwert, Uhr im Zähler ca. 1 h vor. In der Zählerliste als „WMZ Itron“,
  Dauerbetrieb an, Plan alle 15 min, Datenpunkte ohne MQTT.
- 20.10.: Dauerbetrieb und USB abziehen am echten Gerät bestanden (Kriterium 4, konzept-dauerbetrieb.md §5.1).
- MQTT beim Nutzer: Broker 192.168.178.66:1883. Nicht ungefragt Werte dorthin senden.
- Port 1 am externen USB-Hub des Nutzers ist kaputt (kein Softwareproblem).

## Offen (in dieser Reihenfolge)
0. Node-RED: Entscheidung npm öffentlich ja/nein (mbusio-status ist gebaut).
1. Windows-Dienst auf einem Windows-PC prüfen (Checkliste docs/konzept-hintergrund.md §9; wichtig: COM-Port unter LocalService).
   Windows-Code hier prüfen: `rustup target add x86_64-pc-windows-gnu`, dann `cargo clippy --target x86_64-pc-windows-gnu` mit Platzhalter-C-Compiler (CC_x86_64_pc_windows_gnu=Skript, das nur die -Fo/-o-Datei anlegt; AR_…=ar).
2. 7-Tage-Test auf dem Raspberry Pi (Linien dort mit dem Schalter „Dauerbetrieb“ einschalten), danach erst Release v0.5.0.
3. Optional: USB wieder einstecken, bevor mbusio den Ausfall merkt (Linux vergibt dann ttyUSB1; fester Name sollte es abfangen).
4. Weitere Punkte aus dem Simulator-Test hat der Nutzer nie genannt; bei Gelegenheit nachfragen.

Erledigt 19./20.10.: Busleiste „N Zähler antworten nicht“, Projekt leeren + Startprojekt „Schnellscan“ (§12), ein Port eine Linie,
USB weg sofort `disturbed`, fester USB-Name, Windows-Dienst, Dauerbetrieb-Schalter je Linie (§13) als Block mit grossem
Schalter, Hintergrund-Erkennung über cgroup, Fehler-Bit im Zählerstatus nur für Messwerte, Kälteenergie eigener Schlüssel (`energy_cooling`),
Abweichung der Zähleruhr (`meters.clock_off`), USB-Ausfall nach 10 s erkannt (`line::device_gone`).

## Wo was steht
- Konzepte: docs/konzept-bedienung.md (Bedienung, Design v2, Button-Ebenen §11.1, Farben §11.2), docs/konzept-scan.md, docs/konzept-dauerbetrieb.md (Roadmap, MQTT).
- Rust: src/link.rs (Ports, Simulator), src/web/ (axum-API), Linien-Dienste mit `with_bus` / `start_job`.
- Web: web/js/ (ui.js-Bausteine, scan.js, guide.js, busbar.js), web/css/ (aufgeteilt nach Reiter).
