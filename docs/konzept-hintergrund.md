# Konzept: mbusio im Hintergrund (Windows-Dienst)

Stand: 19.10.2026 · Status: **umgesetzt** (Entscheidungen in Abschnitt 9, Prüfung auf Windows offen: Abschnitt 8)

## 1. Ausgangslage

mbusio hat zwei Einsätze (Konzept Bedienung §9): **Service** (Laptop am Strang: scannen, debuggen,
Bericht) und **Dauerbetrieb** (regelmässig auslesen und weitergeben). Der Dauerbetrieb braucht ein
mbusio, das immer läuft: nach dem Neustart des Rechners, ohne Anmeldung, ohne offenes Fenster.

| | Heute |
| --- | --- |
| Linux / Raspberry Pi | ✓ systemd-Dienst (Anleitung im README): startet mit dem Rechner, ohne Anmeldung, nach Absturz neu |
| Windows | ✗ läuft nur, solange das Konsolenfenster offen ist. Fenster zu, Abmelden, Neustart, Energiesparmodus: Plan und Weitergabe stehen still, ohne dass es jemand merkt |

Für den Laptop des Technikers ist das richtig. Für einen Windows-PC als Gateway in der Anlage
nicht, und es verletzt Grundsatz 5 im Konzept Dauerbetrieb („Neustart-fest“).

## 2. Grundsatz

**Ein Programm, eine Datei, kein Modus.** Die .exe bleibt dieselbe. Ob sie als Fenster oder als
Dienst läuft, entscheidet nur, wie sie gestartet wird. Oberfläche, Plan, MQTT, Datenbank: alles
gleich. Das Einrichten als Dienst ist ein einmaliger Schritt, kein Schalter in der Oberfläche.

## 3. Windows-Dienst

### 3.1 Einrichten, Aktualisieren, Entfernen

```
mbusio.exe --install-service [--lan --token GEHEIM]     (als Administrator)
mbusio.exe --uninstall-service                          (als Administrator)
```

`--install-service` macht der Reihe nach:

1. prüft Adminrechte; ohne: klare Meldung „Rechtsklick → Als Administrator ausführen“, nichts geändert
2. hält einen schon eingerichteten Dienst an
3. kopiert die laufende .exe nach `C:\Program Files\mbusio\mbusio.exe`; so geht der Dienst nicht
   kaputt, wenn die heruntergeladene Datei verschoben oder gelöscht wird
4. legt den Dienst „mbusio“ an (oder ändert ihn): Start automatisch, bei Absturz nach 10 s neu
   starten, mit `--no-browser --data-dir C:\ProgramData\mbusio` und den angegebenen `--lan`/`--token`
5. übernimmt die Daten (siehe 3.2) und startet den Dienst
6. öffnet den Browser auf `http://localhost:8080`

**Aktualisieren** = die neue .exe mit `--install-service` aufrufen. Dieselben Schritte, die Daten
bleiben. Kein eigener Befehl.

`--uninstall-service` hält den Dienst an und entfernt ihn. Programm und Daten bleiben liegen
(Pfade werden ausgegeben), damit nichts aus Versehen verloren geht.

### 3.2 Daten

Heute liegen die Daten unter Windows in `%APPDATA%\mbusio`, im Profil des angemeldeten Benutzers.
Ein Dienst hat ein anderes Profil und würde mit einer **leeren** Datenbank starten. Darum:

- Der Dienst benutzt immer `C:\ProgramData\mbusio` (für alle Benutzer, bleibt bei Updates).
- Beim Einrichten: Ist dort noch keine Datenbank, wird die bisherige kopiert (aus `mbusio-data`
  neben der .exe, sonst aus `%APPDATA%\mbusio`). Kopiert, nicht verschoben: das Original bleibt
  als Sicherung. Die Meldung sagt, woher die Daten kamen.
- Ist dort schon eine Datenbank (Update), bleibt sie unverändert.

### 3.3 Doppelklick, während der Dienst läuft

Geht schon heute: Port 8080 ist belegt, also öffnet die .exe nur den Browser auf das laufende
mbusio und beendet sich. Der Techniker arbeitet damit in derselben Oberfläche wie der Dienst; der
Linien-Dienst sorgt dafür, dass Bedienung und Plan sich nicht stören (§9.4).
CLI-Befehle (`mbusio scan --port COM3`) gegen einen Port, den der Dienst gerade hält, bekommen
„Port belegt“ (COM-Ports sind unter Windows exklusiv).

### 3.4 Was ein Dienst zusätzlich braucht

| Thema | Lösung |
| --- | --- |
| **Kein Fenster für Fehler** | Start, Stopp und Fehler, die den Start verhindern (z.B. Datenbank nicht lesbar), zusätzlich in `C:\ProgramData\mbusio\mbusio.log` (klein, rollt bei 1 MB). Alles andere steht wie bisher im Protokoll der Oberfläche |
| **Energiesparmodus** | Solange ein Plan aktiv ist, meldet mbusio Windows „System wird gebraucht“ (`SetThreadExecutionState`), damit der PC nicht einschläft. Bildschirm darf aus |
| **USB-Energiesparen** | Windows schaltet USB-Anschlüsse gern ab („selektives Energiesparen“), dann ist der Wandler weg. Hinweis im README; mbusio kann das nicht selbst ändern |
| **Herunterfahren** | Der Dienst bekommt „Stopp“ bzw. unter systemd SIGTERM und beendet die Weboberfläche geordnet. Eine gerade laufende Auslesung kann abbrechen; die Datenbank bleibt dank WAL und einer Transaktion je Zähler heil, der Zähler wird nachgeholt (Kriterium 3) |
| **Konto** | `LocalService` (wenig Rechte, der Webserver ist im LAN erreichbar). COM-Ports dürfen normale Benutzer öffnen, also auch dieses Konto; auf Windows prüfen. Schreibrechte auf den Datenordner setzt das Einrichten (`icacls`, über die SID, unabhängig von der Sprache) |
| **Zugriffsschlüssel** | steht in der Befehlszeile des Dienstes; die können Benutzer des PCs lesen. Für einen Anlagen-PC in Ordnung, sonst ohne `--token` nur lokal betreiben |
| **Firewall** | Mit `--lan` beim Einrichten eine Regel für Port 8080 anlegen (Adminrechte sind ja da), sonst ist die Oberfläche im LAN nicht erreichbar |

## 4. Hinweis in der Oberfläche

`/api/status` meldet zusätzlich `background: true|false`:
- Windows: läuft als Dienst
- Linux: läuft unter systemd (Umgebungsvariable `INVOCATION_ID`)

Ist eine Linie im Dauerbetrieb (Konzept Bedienung §13) und `background` falsch, zeigt der **Zustand der
Anlage** orange: „Läuft nur, solange mbusio geöffnet ist. Für den Dauerbetrieb als Dienst
einrichten“ mit Link auf die Anleitung im README. Im Service-Fall erscheint nichts.

## 5. Linux

Bleibt wie heute (systemd-Unit aus dem README). Ein `--install-service` für Linux (Unit schreiben,
`systemctl enable`) wäre symmetrisch, ist aber nicht nötig; nur wenn es sich ohne Aufwand ergibt.

## 6. Bewusst nicht

- kein Installer (MSI), keine Registry-Einstellungen ausser dem Dienst selbst
- kein Tray-Symbol, keine Fenster-Variante ohne Konsole
- kein Knopf „als Dienst einrichten“ in der Oberfläche (bräuchte UAC aus dem Browser heraus);
  siehe Frage 9.3
- keine automatische Aktualisierung

## 7. Umsetzung

1. Crate `windows-service` nur für Windows (`[target.'cfg(windows)'.dependencies]`), Pure Rust,
   Cross-Compile wie bisher. `main` erkennt den Start durch den Dienst-Manager und läuft dann
   `serve()` mit Stopp-Signal statt Ctrl-C.
2. `--install-service` / `--uninstall-service` in eigener Datei `src/service.rs` (Kopieren, Daten
   übernehmen, Dienst anlegen, Firewall), alles andere unverändert.
3. `background` in `/api/status`, Hinweis im Zustand der Anlage (Texte DE/EN).
4. `SetThreadExecutionState`, solange `planner.any_planned()`.
5. Log-Datei für Start/Stopp/Startfehler.
6. README: Abschnitt „Windows als Dienst“ (Befehle, Daten, USB-Energiesparen), CHANGELOG.

## 8. Tests

- Rust: Datenübernahme (leeres Ziel → kopiert; Ziel vorhanden → unverändert), `background` aus
  der Umgebung, Kommandozeile des Dienstes (Pfade mit Leerzeichen richtig gequotet).
- Windows-Dienst selbst lässt sich hier (Linux) nicht ausführen: Prüfung auf einem Windows-PC
  von Hand mit Checkliste (einrichten, Neustart ohne Anmeldung, Doppelklick öffnet Browser,
  Update, entfernen, USB ab/an, Energiesparmodus). Das CI baut die .exe weiterhin mit.
- UI-Test: Dauerbetrieb ohne `background` zeigt den Hinweis, Service-Projekt nicht.

## 9. Entschieden (19.10.2026)

Alle Empfehlungen übernommen: Windows-Dienst (kein Autostart bei der Anmeldung), Konto
`LocalService`, Doppelklick-Dateien `mbusio-dienst-einrichten.cmd` / `-entfernen.cmd` im Release
(Ordner `windows/`, CRLF), kein Linux-`--install-service`, Pi-Test und Windows-Dienst parallel.

**Umgesetzt:** `src/service.rs` (Erkennen `background`, Daten übernehmen, Log, Energiesparen,
Windows-Teil `win`), `--install-service` / `--uninstall-service` / `--service` in `main.rs`, `serve`
mit Stopp-Signal (Ctrl+C, SIGTERM, Dienst-Stopp), `background` und `os` in `/api/status`, Hinweis im
Zustand der Anlage, README, Release-Paket. Unter Linux erkennt `background` den Start durch
systemd am Elternprozess (PID 1).

**Prüfen auf einem Windows-PC (Checkliste):**
1. Fenster-Betrieb mit Plan: Zustand der Anlage zeigt „Läuft nur, solange mbusio geöffnet ist“.
2. `mbusio-dienst-einrichten.cmd` doppelklicken, Adminrechte bestätigen, „j“ und Schlüssel: Browser
   öffnet sich, Projekte von vorher sind da, Hinweis aus 1 ist weg.
3. PC neu starten, nicht anmelden: von einem anderen Gerät `http://<IP>:8080` öffnen, Plan liest,
   MQTT kommt an.
4. Doppelklick auf die .exe: öffnet nur den Browser.
5. COM-Port im Dienst: Verbinden und Auslesen klappt (sonst Konto auf `LocalSystem`, Abschnitt 3.4).
6. USB-Wandler ab und wieder an: Linie offline, MQTT `disturbed`, danach weiter ohne Eingriff.
7. Dienst im Task-Manager beenden: startet nach 10 s neu.
8. Neue .exe mit der .cmd einrichten (Update): Daten bleiben.
9. `mbusio-dienst-entfernen.cmd`: Dienst weg, `C:\ProgramData\mbusio` bleibt.

## 9a. Ursprüngliche Fragen

1. **Windows-Dienst wie in Abschnitt 3?** (Alternative ohne Adminrechte: Autostart bei der
   Anmeldung über den Autostart-Ordner. Läuft dann aber nur, solange jemand angemeldet ist, und
   mit sichtbarem Fenster; für eine Anlage zu wackelig. Empfehlung: nur der Dienst.)
2. **Konto `LocalService`** (wenn COM-Ports gehen), sonst `LocalSystem`?
3. Reicht die Kommandozeile (einmal „Als Administrator“ in einer Eingabeaufforderung), oder
   braucht es eine Datei `mbusio-dienst-einrichten.cmd` zum Doppelklicken neben der .exe im
   Release? (Empfehlung: die .cmd-Datei, sie fragt selbst nach Adminrechten.)
4. Linux-`--install-service` jetzt mitmachen oder weglassen? (Empfehlung: weglassen.)
5. Reihenfolge: vor oder nach dem 7-Tage-Test auf dem Pi? Der Pi-Test braucht das nicht; für
   v0.5.0 wäre es schön, ist aber kein Muss. (Empfehlung: Pi-Test zuerst starten, weil er 7 Tage
   läuft, und in der Zeit den Windows-Dienst bauen.)
