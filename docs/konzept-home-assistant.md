# Konzept: Home Assistant (MQTT Discovery)

Stand: 25.10.2026 · Status: **umgesetzt** (geprüft mit Test-Broker; echter Test mit Home Assistant offen) · Teil von [konzept-ausgaenge.md](konzept-ausgaenge.md), Schritt 3

Mit einem Häkchen in den MQTT-Einstellungen tauchen alle Datenpunkte mit MQTT-Häkchen in Home
Assistant von selbst auf: mit Name, Einheit, passendem Typ (Energie, Wasser, Temperatur …), je
Zähler als Gerät, und für das Energie-Dashboard verwendbar. Nichts muss in Home Assistant von Hand
angelegt werden.

## 1. Was mbusio sendet

Für jeden **aktiven Datenpunkt mit MQTT-Häkchen** eine retained Konfig-Nachricht:

```
<präfix>/sensor/<knoten>/p<projekt-id>-dp<nr>/config
```

- `<präfix>`: Einstellung, Vorgabe `homeassistant` (wie in Home Assistant).
- `<knoten>`: die Client-ID von mbusio als Topic-Teil (z.B. `mbusio-raspberrypi`), damit zwei
  mbusio am selben Broker sich nicht in die Quere kommen.
- Projekt-ID und Nummer `no` bleiben für immer gleich, auch wenn Namen oder Topics sich ändern.

```json
{
  "name": "Energie",
  "unique_id": "mbusio-raspberrypi-p1-dp1",
  "state_topic": "mbusio/schnellscan/gateway-1-linie-1/whg-1/energy",
  "value_template": "{{ value_json.value }}",
  "json_attributes_topic": "mbusio/schnellscan/gateway-1-linie-1/whg-1/energy",
  "unit_of_measurement": "kWh",
  "device_class": "energy",
  "state_class": "total_increasing",
  "availability_mode": "all",
  "availability": [
    { "topic": "mbusio/status" },
    { "topic": "mbusio/…/energy",
      "value_template": "{{ 'online' if value_json.quality in ['good', 'stale'] else 'offline' }}" }
  ],
  "device": { "identifiers": ["mbusio-raspberrypi-m12"], "name": "Whg 1",
              "manufacturer": "ITR", "model": "Wärme", "serial_number": "20395066" },
  "origin": { "name": "mbusio", "sw_version": "0.4.0", "support_url": "https://github.com/Wobi848/mbusio" }
}
```

- **Wert und Attribute** kommen aus der normalen MQTT-Nachricht des Datenpunkts; nichts wird
  doppelt gesendet. Qualität, Zeit und Nummer sind als Attribute sichtbar.
- **Nicht verfügbar** zeigt Home Assistant, wenn mbusio offline ist (Last Will) oder der Wert
  gestört bzw. ungültig ist. Ein veralteter Wert bleibt sichtbar (Attribut `quality: stale`).
- **Gerät = Zähler**, damit Home Assistant die Werte eines Zählers zusammenfasst. Home Assistant
  setzt den Gerätenamen vor den Sensornamen; beginnt der Name des Datenpunkts mit dem Zähler
  („Whg 1 · Energie“), wird dieser Teil weggelassen („Energie“).

## 2. Typ in Home Assistant

`device_class` nur, wenn die Einheit des Datenpunkts zu dem passt, was Home Assistant für diesen
Typ annimmt; sonst ein normaler Sensor ohne Typ (Home Assistant würde ihn sonst ablehnen).

| Grösse in mbusio | Einheiten | device_class |
|---|---|---|
| energy, energy_cooling | Wh, kWh, MWh, kJ, MJ, GJ | energy |
| volume (Medium Wasser) | m³, L | water |
| volume (Medium Gas) | m³, L | gas |
| volume (sonst) | m³, L | volume |
| power | W, kW, MW | power |
| volume_flow | m³/h, L/h, L/min | volume_flow_rate |
| flow/return/external_temperature | °C | temperature |
| pressure | bar | pressure |
| voltage | V | voltage |
| current | A | current |
| mass | kg | weight |
| on_time, operating_time | s, min, h, d | duration |

`state_class`: Zählerstände (Energie, Volumen, Masse, HKV-Einheiten) und Betriebszeiten
`total_increasing`, als Speicherwert (z.B. Stichtag) `total`; alles andere `measurement`.
Die Temperaturdifferenz bekommt bewusst keinen Typ (sie ist keine Temperatur).

## 3. Wann gesendet wird

mbusio merkt sich je Konfig-Topic, was zuletzt gesendet wurde (Tabelle `mqtt_ha`). Bei jedem
Abgleich (nach Änderungen an Datenpunkten oder Einstellungen und nach jedem Plan-Durchgang, also
überall, wo heute `outputs_refresh` läuft):

- neu oder geändert (Name, Einheit, Topic …) → Konfig senden
- nicht mehr gewünscht (MQTT-Häkchen weg, Datenpunkt inaktiv oder gelöscht, Home Assistant
  ausgeschaltet) → leere Nachricht senden; Home Assistant entfernt den Sensor
- unverändert → nichts

Die Nachrichten gehen durch die normale Warteschlange (Zwischenspeicher, Reihenfolge,
Bestätigung). Die Warteschlange bekommt dafür eine Spalte `abs`: Konfig-Topics stehen nicht unter
dem Basis-Topic.

**Client-ID nicht mehr ändern**, wenn Home Assistant eingeschaltet ist: Mit einer neuen ID legt
Home Assistant die Sensoren neu an (die alten entfernt mbusio), der Verlauf in Home Assistant
beginnt dann neu. Die Oberfläche sagt das beim Feld.

## 4. Einstellungen

In MQTT → Einstellungen: **„Home Assistant (MQTT Discovery)“** ein/aus (Vorgabe aus) und
**Präfix** (Vorgabe `homeassistant`). Gespeichert in `meta.mqtt` als neue Felder `ha` und
`ha_prefix`; ältere Einstellungen laden unverändert (ohne Home Assistant).

## 5. Prüfen

- Rust-Tests: Konfig beim Einschalten, Inhalt (Typ, Einheit, Gerät, Verfügbarkeit), nichts bei
  unverändert, Entfernen bei Häkchen weg und bei Home Assistant aus, Warteschlange mit `abs`.
- UI-Test mit Test-Broker: Häkchen setzen, Konfig-Nachricht kommt am Broker an.
- Echter Test mit Home Assistant erst nach Freigabe (Broker 192.168.178.66 nicht ungefragt).
