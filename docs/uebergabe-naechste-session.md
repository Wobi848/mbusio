# Übergabe an die nächste Session (Stand 2026-10-28)

Lies das zuerst. Repo: Wobi848/mbusio, Branch `claude/mbusio-context-w51dtz`, Stand siehe `git log` (der letzte Eintrag hier: Aufräumen vom 28.10.).
Mit dem Nutzer **auf Deutsch** reden, kurz und direkt.

## Harte Regeln des Nutzers
- Modbus TCP am 25.10. freigegeben (docs/konzept-modbus.md). Alles bleibt privat: nichts auf npm.
- **Kein Release v0.5.0**, bevor der 7-Tage-Test auf dem Raspberry Pi gelaufen ist (Strom weg, USB ab, kleine SD-Karte).
- KISS: wenig Code, wiederverwendbare Bausteine (web/app/view.js, web/js/base.js), nichts doppelt, kein Spaghetti.
  Dazu gelten die globalen Coding-Prinzipien des Nutzers (~/.claude/CLAUDE.md), u.a. Dateien über ~400 Zeilen teilen.
- Grössere Änderungen: erst Konzept (docs/konzept-*.md), dann bauen.
- Buttons müssen klar sichtbar sein: primär / `.tonal` (sekundär) / `.quiet`; Menüpunkte immer quiet.
- Problemzähler: orange = Hinweis, Zähler bleibt übernehmbar; rot = Fehler.
- Plan-Vorgabe „Aus“, 24-h-Zeit, batteriebetriebene Zähler nicht zu oft auslesen.

## Tests
- CI (GitHub Actions, privates Repo, Gratis-Plan 2000 Minuten/Monat): bei jedem Push nur Tests
  (Rust, UI, Node-RED); die Builds nur bei Tag `v*` oder von Hand. Am 25.10. war das Kontingent
  aufgebraucht (Jobs starten nicht) – dann lokal prüfen.
- `cargo test` → 178 passed, 1 ignored; `cargo clippy --all-targets` ohne Warnungen.
- UI: `PLAYWRIGHT=$(ls -d ~/.npm/_npx/*/node_modules/playwright|head -1) MBUSIO_TEST_BROKER=127.0.0.1:18830 node tests/ui/run.mjs` → 25 Tests, alle grün (dauert ca. 20 min).
- Server lokal: `cargo run -- --no-browser` (Port 8080; der Debug-Build liest web/ direkt von der Platte).
- Commits **ohne** KI-Hinweis (globale Regel des Nutzers); nach jedem fertigen Schritt committen und pushen. Nicht mergen ohne ausdrückliches OK.

## Zuletzt erledigt
- 28.10.: Aufräumen: Tastenverhalten bearbeitbarer Felder (Escape/Enter) einmal in `view.js`
  (`fieldKeys`), toter CSS-Rest entfernt, Übergabe nachgeführt. Doppelter Code laut jscpd 0,07 %.
- 26./27.10.: Neue Oberfläche (docs/konzept-oberflaeche-neu.md) ist die einzige, liegt in `web/app/` (bis 28.10. `web/neu/`)
  (Bereiche als ES-Module, Texte je Bereich in `*-texte.js`); die alte ist entfernt. Beim Umbau
  verlorene Funktionen zurückgeholt, Test `25-alte-funktionen` hält sie fest. E-Mail als Ausgang
  (Meldungen, Warnungen, Bericht; docs/konzept-weitergabe-email.md), Abrechnung mit Stichtagen,
  Quartal/Halbjahr, Zählertausch (docs/konzept-export.md), Weitergabe-Protokoll, Speicherschutz mit
  Ausdünnen. Grosse Rust- und JS-Dateien nach Aufgaben geteilt, ohne Verhaltensänderung.
- 25.10.: Modbus TCP (docs/konzept-modbus.md), geprüft mit eigenem Client und pymodbus. Simulator-
  Zählerstände laufen ab 1.10.2026 (Systemuhr) und überleben Neustarts. Repo hat nur diesen einen
  Branch (Standard-Branch), es gibt nichts zu mergen.
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
- Später beim Ausbau teilen: web/app/zaehler.js, weitergabe.js, scan.js (je 520–580 Zeilen).
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
- Konzepte: docs/konzept-oberflaeche-neu.md (heutige Oberfläche), docs/konzept-bedienung.md (Leitbild §14,
  Button-Ebenen §11.1, Farben §11.2), docs/konzept-scan.md, docs/konzept-dauerbetrieb.md (Roadmap, Abnahme §5.1),
  Ausgänge/Modbus/Node-RED/Home Assistant/E-Mail/Export je ein docs/konzept-*.md; HTTP-API: docs/api.md.
- Rust: Aufbau der Module im README; Linien-Dienst src/line.rs (+ src/line/), Ports src/link.rs.
- Web: web/index.html lädt web/app/app.js; Bausteine in web/app/view.js, Daten in store.js/data.js,
  gemeinsame Logik in web/js/ (base.js, texts.js, scanlogic.js, listlogic.js, plot.js).
- docs/uebergabe-design-v2.md ist abgelöst (historisch).
