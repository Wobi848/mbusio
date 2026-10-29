# Übergabe an die nächste Session (Stand 2026-10-29)

Lies das zuerst. Repo: Wobi848/mbusio, Branch `claude/mbusio-context-w51dtz`, Stand siehe `git log` (der letzte Eintrag hier: Aufräumen vom 28.10.).
Mit dem Nutzer **auf Deutsch** reden, kurz und direkt.

## Harte Regeln des Nutzers
- Modbus TCP am 25.10. freigegeben (docs/konzept-modbus.md). Alles bleibt privat: nichts auf npm.
- v0.5.0 am 28./29.10. auf Wunsch des Nutzers ohne Pi-/Windows-Test getaggt (im CHANGELOG vermerkt). Releases nur nach ausdrücklichem OK.
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
- `cargo test` → 180 passed, 1 ignored; `cargo clippy --all-targets` ohne Warnungen.
- UI: `PLAYWRIGHT=$(ls -d ~/.npm/_npx/*/node_modules/playwright|head -1) MBUSIO_TEST_BROKER=127.0.0.1:18830 node tests/ui/run.mjs` → 25 Tests, alle grün (dauert ca. 20 min).
- Server lokal: `cargo run -- --no-browser` (Port 8080; der Debug-Build liest web/ direkt von der Platte).
- Commits **ohne** KI-Hinweis (globale Regel des Nutzers); nach jedem fertigen Schritt committen und pushen. Nicht mergen ohne ausdrückliches OK.

## Zuletzt erledigt
- 29.10. (nach v0.5.0): Sicherung des Geräts, Schritt 1 (docs/konzept-sicherung.md): `GET /api/backup`
  (VACUUM INTO), `POST /api/restore` prüft, `restore/apply` legt `mbusio.db.restore` bereit und
  startet neu (`service::restart`: als Dienst Exit 75, sonst startet sich mbusio selbst mit
  `MBUSIO_RESTARTED`); beim Start setzt `backup::restore_pending` sie vor dem Öffnen ein. Offen:
  Schritt 2 (nur Einstellungen übertragen) und 3 (automatisch sichern), Lasttest für den Pi.
- 28./29.10. (Durchsicht des ganzen Projekts, nach dem Tag v0.5.0): Sicherheit: ändernde
  Anfragen fremder Webseiten abgelehnt (`foreign_origin`, Origin ≠ Host → 403). Release mit
  `panic = "abort"` (Neustart durch systemd/Dienst statt vergifteter DB-Sperre). Verlauf über den
  ganzen Zeitraum (über 2000: letzte je Tag). Oberfläche: „mbusio antwortet nicht“ bei Ausfall,
  Zählerliste/Projektbaum laden bei `status.changes` neu, Nächster Schritt „Zähler auslesen“ bis
  `unread` = 0. Import 2000 Zähler 0,5 s statt 8,5 s. Bericht: ein Datumsformat (`dateTime` in
  base.js), Einzahl, keine „?“ im Änderungsprotokoll. Kontrast hell (Grün), Anlage ohne
  abgeschnittene Wörter, gemeinsame Listentasten (`selectionKeys` in keys.js). Windows-Teil mit
  clippy (Platzhalter-CC) ohne Warnungen.
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
- **IN ARBEIT: automatische Sicherung (docs/konzept-sicherung.md §5)**. Entschieden vom Nutzer:
  Ziele Ordner (USB, eingebundenes Netzlaufwerk), **SMB über das Betriebssystem** (Windows-Pfad
  `\\server\freigabe`, Pi per /etc/fstab einbinden, Anleitung ins README) und **FTP/FTPS** (eigener
  Client); **verschlüsselt mit Sicherungs-Passwort** (Nutzerwunsch). Plan:
  1. `src/textproto.rs` ist geschrieben, aber noch NICHT in main.rs eingebunden: gemeinsame
     Verbindung (Conn Plain/TLS, `connect`, `tls(config, host, tcp)`, Session reply/cmd/expect) aus
     `src/mail/smtp.rs` herausgelöst. Nächster Schritt: `mod textproto;` in main.rs, smtp.rs darauf
     umstellen (Conn/Session/tls dort löschen; STARTTLS über `s.switched(...)`), Tests grün.
  2. `src/backup.rs` → `src/backup/mod.rs`; neu `crypt.rs` (ring ist schon im Lock: AES-256-GCM,
     PBKDF2-SHA256 200 000 Runden; Format `MBUSIOV1` | Runden u32 | Salt 16 | Nonce 12 | Daten),
     `ftp.rs` (USER/PASS, AUTH TLS + PBSZ 0 + PROT P, TYPE I, CWD/MKD, PASV, STOR, NLST, DELE; eine
     Arc<ClientConfig> für Steuer- und Datenverbindung wegen Sitzungs-Wiederaufnahme; Testserver im
     Rust-Test wie beim SMTP), `auto.rs` (Config in meta "backup": enabled, target folder|ftp,
     folder, host, port, user, password, dir, tls, insecure, time "02:30", keep 7, secret; Zustand
     meta "backup_state"; täglich fällig, im eigenen Thread, damit der Plan nicht wartet; Name
     `mbusio-sicherung-<gerät>-JJJJ-MM-TT_HHMM.mbusio` verschlüsselt bzw. `.db`; nur eigene Dateien
     über `keep` löschen).
  3. Hook in `scheduler::tick` (wie report/export_billing); Warnung: Art "backup" in
     `warn::KINDS` UND `web/app/weitergabe-ausgaenge.js` WARN_KINDS (Liste ist doppelt, später vom
     Server liefern) + Texte `warn_kind_backup`.
  4. Download/Restore: mit gesetztem Sicherungs-Passwort verschlüsselt; beim Einspielen erkennt die
     Oberfläche `MBUSIOV1` an den ersten Bytes und fragt das Passwort (Header `X-Backup-Password`).
  5. Oberfläche: Sicherung als Karte bei den Ausgängen in der Weitergabe (gleiche Bausteine wie
     MQTT/E-Mail: outputFields/formValues), „Jetzt sichern“, letztes Ergebnis; README, API, CHANGELOG
     (Abschnitt „Unveröffentlicht“).
- Danach: Schritt 2 Sicherung (nur Einstellungen übertragen, ohne Passwörter), Lasttest für den Pi.
- Später beim Ausbau teilen: web/app/zaehler.js, weitergabe.js, scan.js (je 520–580 Zeilen).
0. Node-RED: Entscheidung npm öffentlich ja/nein (mbusio-status ist gebaut).
1. Windows-Dienst auf einem Windows-PC prüfen (Checkliste docs/konzept-hintergrund.md §9; wichtig: COM-Port unter LocalService).
   Windows-Code hier prüfen: `rustup target add x86_64-pc-windows-gnu`, dann `cargo clippy --target x86_64-pc-windows-gnu` mit Platzhalter-C-Compiler (CC_x86_64_pc_windows_gnu=Skript, das nur die -Fo/-o-Datei anlegt; AR_…=ar).
2. 7-Tage-Test auf dem Raspberry Pi (Linien dort mit dem Schalter „Dauerbetrieb“ einschalten); steht für v0.5.0 noch aus.
   GitHub Actions war ab 25.10. gesperrt (Abrechnung/Kontingent): prüfen, ob der Tag v0.5.0 gebaut und als Release veröffentlicht wurde.
   Der Tag v0.5.0 wurde am 29.10. auf Wunsch des Nutzers auf den Stand mit allen Korrekturen verschoben
   (vorher 411c6a3; es war noch nichts gebaut oder veröffentlicht).
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
