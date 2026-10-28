# Konzept: Sicherung und Wiederherstellung

Stand: 29.10.2026 · Status: **Schritt 1 umgesetzt**, Schritt 2 und 3 offen

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

## 5. Schritt 3 (zur Entscheidung): automatisch sichern

Eine Sicherung von Hand ist beim Ausfall oft alt. Vorschlag: täglich nach dem ersten Plan-Termin
eine Sicherung in einen wählbaren Ordner (USB-Stick, Netzlaufwerk), die letzten 7 behalten. Erst
nach Schritt 1 und 2 entscheiden.
