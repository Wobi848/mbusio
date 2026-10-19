# Übergabe an die nächste Session (Stand 2026-10-19)

Lies das zuerst. Repo: Wobi848/mbusio, Branch `claude/mbusio-context-w51dtz`, letzter Commit `38380b7`.
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
- `cargo test` → 104 passed, 1 ignored.
- UI: `PLAYWRIGHT=$(ls -d ~/.npm/_npx/*/node_modules/playwright|head -1) MBUSIO_TEST_BROKER=127.0.0.1:18830 node tests/ui/run.mjs` → 12 Tests, alle grün.
- Server lokal: `cargo run -- --no-browser` (Port 8080; der Debug-Build liest web/ direkt von der Platte).
- Commits enden mit `Co-Authored-By: Claude …`; nach jedem fertigen Schritt committen und pushen.

## Zuletzt erledigt
- `38380b7` Port-Auswahl: `link::list_ports()` liefert `{port,label}`, USB-Wandler mit Namen zuerst, leere `/dev/ttyS*` ausgeblendet, Simulatoren am Ende (Rust, CLI `--list-ports`, web/js/scan.js `loadPorts`). Abgeschlossen und getestet.
- Davor: Buttons (tonal/quiet), orange/rote Problemzeilen, MQTT-Topic-Muster + Liste für den Integrator. MQTT hat der Nutzer mit einem echten Broker getestet: Daten kommen bis MQTT an.

## Hardware
- Der M-Bus-Master **Relay MR003 USB** (CP210x, 10c4:ea60) erscheint als `/dev/ttyUSB0` und funktioniert.
- `mbusio bus-bauds --port /dev/ttyUSB0` sendet auf allen Baudraten, aber **es ist kein Zähler angeschlossen**, daher keine Antworten.
- Nächster echter Test: Zähler anklemmen, in der Oberfläche den Port wählen, Primärscan starten.
- Port 1 am externen USB-Hub des Nutzers ist kaputt (kein Softwareproblem).

## Offen (in dieser Reihenfolge)
1. ~~Busleiste „Linie in Ordnung“ trotz gestörter Zähler~~: erledigt 19.10. (Nächster Schritt „N Zähler antworten nicht“). Weitere Punkte aus dem Simulator-Test hat der Nutzer noch nicht genannt; bei Gelegenheit nachfragen.
2. ~~Schnellscan~~: umgesetzt 19.10. als Variante A schlank (Konzept Bedienung §12): „Leeren …“ je Projekt, Startprojekt „Schnellscan“.
3. Echter Test mit dem MR003, sobald ein Zähler dran ist (der Nutzer hat evtl. im Geschäft ein Gerät).
4. ~~Zwei Linien am selben Port~~: erledigt 19.10. (`line::Ports`, `link::port_key`).
5. 7-Tage-Test auf dem Raspberry Pi, danach erst Release v0.5.0 (CHANGELOG.md pflegen).

## Wo was steht
- Konzepte: docs/konzept-bedienung.md (Bedienung, Design v2, Button-Ebenen §11.1, Farben §11.2), docs/konzept-scan.md, docs/konzept-dauerbetrieb.md (Roadmap, MQTT).
- Rust: src/link.rs (Ports, Simulator), src/web/ (axum-API), Linien-Dienste mit `with_bus` / `start_job`.
- Web: web/js/ (ui.js-Bausteine, scan.js, guide.js, busbar.js), web/css/ (aufgeteilt nach Reiter).
