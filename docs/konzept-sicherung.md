# Konzept: Sicherung und Wiederherstellung

Stand: 29.10.2026 · Status: **Schritt 1 und 3 umgesetzt**, Schritt 2 offen

## 1. Ziel

Das Gerät (Pi, PC) fällt aus: neues Gerät, mbusio installieren, **Sicherung einspielen, alles läuft
wieder**: Projekte, Linien mit Anschluss, Zähler, Messwerte (für die Abrechnung), Datenpunkte,
Ausgänge (MQTT, Modbus, E-Mail mit Passwörtern), Ausleseplan und Dauerbetrieb.

Zweiter Fall: ein **weiteres Gerät gleich einrichten**: nur die Geräte-Einstellungen (MQTT, Modbus,
E-Mail, Speicherschutz) übertragen, ohne Projekte und ohne Passwörter.

## 2. Was zum Zustand gehört

Alles steht in **einer** Datei, `mbusio.db` (SQLite) im Datenordner: Projekte, Zähler, Messwerte,
Einstellungen samt Passwörtern, Simulationen. Daneben liegen nur die automatisch abgelegten
Abrechnungsdateien (`exporte/`), die sich aus den Messwerten neu erzeugen lassen. Eine Sicherung
ist darum eine saubere Kopie der Datenbank, keine eigene Formatbeschreibung.

## 3. Schritt 1: Komplett-Sicherung (von Hand)

- **Sichern:** Einstellungen (⚙) → „Sicherung herunterladen“. Der Server schreibt mit
  `VACUUM INTO` eine in sich stimmige, kompakte Kopie (auch während der Plan liest) und liefert
  sie als `mbusio-sicherung-<gerät>-<datum>.db`. Die Datei enthält die Passwörter: wie einen
  Schlüssel aufbewahren (steht beim Knopf).
- **Einspielen:** Einstellungen → „Sicherung einspielen …“ → Datei wählen. Der Server prüft sie
  (SQLite, `integrity_check`, Tabellen von mbusio, nicht von einer neueren Version) und zeigt, was
  drin ist: Projekte, Zähler, Messwerte, Zeitraum, Gerät. Erst „Einspielen und neu starten“ ersetzt
  den heutigen Stand.
- **Ersetzen beim Start:** die geprüfte Datei liegt als `mbusio.db.restore` bereit; mbusio startet
  neu und setzt sie ein, **bevor** irgendetwas die Datenbank öffnet. Der bisherige Stand bleibt als
  `mbusio.db.vorher` liegen (eine Rückfalldatei, die nächste ersetzt sie). Warum Neustart statt
  Umschalten im Betrieb: Zeitplan, MQTT und Modbus laufen als Dauerschleifen; ein Neustart nimmt
  den bewährten Startweg, ohne Sonderfälle.
- **Neu starten:** als Dienst (systemd, Windows-Dienst) beendet sich mbusio, der Dienst startet es
  neu. Im Fenster startet mbusio sich selbst neu. Die Oberfläche zeigt „mbusio antwortet nicht“ und
  lädt danach von selbst.
- **Version:** mbusio merkt sich in der Datenbank seine Version. Eine Sicherung einer neueren
  Version wird abgelehnt (die ältere kennt ihre Daten nicht vollständig); ältere werden beim Start
  wie immer nachgeführt.

## 4. Schritt 2: Einstellungen übertragen

Einstellungen → „Einstellungen sichern“ / „Einstellungen einlesen“: JSON mit MQTT, Modbus, E-Mail
und Speicherschutz, **ohne Passwörter**. Beim Einlesen Vorschau → Übernehmen (bestehender Baustein),
danach fragt mbusio die fehlenden Passwörter ab. Projekte gehen weiter über den Projekt-Export.

## 5. Schritt 3: automatisch sichern (umgesetzt)

Eine Sicherung von Hand ist beim Ausfall oft alt, und eine Sicherung **auf dem Gerät** hilft nicht,
wenn das Gerät ausfällt. Darum automatisch und **auf einen anderen Rechner**.

### 5.1 Wann und wie viele

- Täglich zu einer einstellbaren Zeit (Vorgabe 02:30, 24 h), dazu „Jetzt sichern“ zum Ausprobieren.
- Am Ziel bleiben die letzten **7** (einstellbar); ältere `mbusio-sicherung-<gerät>-*.db` löscht
  mbusio dort selbst. Nur eigene Dateien dieses Geräts, nichts anderes im Ordner.
- Gleiche Datei wie „Herunterladen“ (`VACUUM INTO`), also mit „Einspielen …“ direkt verwendbar.

### 5.2 Ziele

| Ziel | Wie | Bemerkung |
| --- | --- | --- |
| **Ordner** | Pfad auf dem Gerät | USB-Stick; ein Netzlaufwerk, das das Betriebssystem eingebunden hat |
| **SMB (Windows-Freigabe, NAS)** | Windows: Pfad `\\server\freigabe\mbusio` direkt als Ordner. Raspberry Pi: Freigabe einmal einbinden (`/etc/fstab`, Anleitung im README), dann als Ordner | kein eigener SMB-Client: dafür gibt es in Rust nichts Verlässliches; das Betriebssystem kann es |
| **FTP / FTPS** | eigener kleiner Client (wie beim E-Mail-Versand): Server, Port, Benutzer, Passwort, Ordner, Verschlüsselung (FTPS explizit, Vorgabe) oder ohne | passiver Modus; ohne Verschlüsselung geht das Passwort im Klartext übers Netz (Hinweis) |

### 5.3 Zustand und Meldungen

- Einstellungen → Sicherung: Ziel, Zeit, Anzahl, „Jetzt sichern“, darunter das letzte Ergebnis
  („heute 02:30 · 1,2 MB · gespeichert“ oder der Fehler).
- Scheitert die Sicherung, meldet mbusio das über die bestehenden **Meldungen** (Statusleiste,
  E-Mail-Warnung, falls eingerichtet), damit niemand erst beim Ausfall merkt, dass seit Wochen
  nichts gesichert wurde.

### 5.4 Entschieden (29.10., Nutzer)

1. **SMB über das Betriebssystem** statt eines eigenen SMB-Clients: Windows-Pfad bzw. auf dem Pi
   einmal in `/etc/fstab` einbinden (Anleitung im README). Kein eigener SMB-Client in mbusio.
2. **Verschlüsseln:** anders als zuerst vorgeschlagen (oben, historisch) doch verschlüsseln, mit
   einem eigenen **Sicherungs-Passwort** (AES-256-GCM, Schlüssel aus PBKDF2-SHA256 200 000 Runden;
   Format `MBUSIOV1` | Runden | Salt 16 | Nonce 12 | Daten, `src/backup/crypt.rs`). Gilt auch für
   „Sicherung herunterladen“/„Einspielen …“ von Hand (Schritt 1): ist ein Sicherungs-Passwort
   eingestellt, verschlüsselt das Herunterladen automatisch; Einspielen erkennt die Kennung und
   fragt im Feld nach dem Passwort.

Umsetzung: `src/backup/` als Modul (`crypt.rs`, `ftp.rs` auf `src/textproto.rs`, `auto.rs` für
Konfiguration/Zustand/Ablegen), Zeitplan-Hook in einem eigenen Thread (`scheduler::upkeep`), Art
`backup` in `warn::KINDS`, Oberfläche als Karte bei den Ausgängen (Weitergabe), API
`/api/backup/auto` (siehe `docs/api.md`).
