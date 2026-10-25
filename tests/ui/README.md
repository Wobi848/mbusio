# Browser-Tests der Oberfläche

Jeder Test startet ein eigenes mbusio mit dem Simulator und einem leeren Datenordner, bedient die
Oberfläche wie ein Benutzer und prüft das Ergebnis.

```sh
cargo build
npm i -D playwright && npx playwright install chromium   # einmalig
node tests/ui/run.mjs            # alle
node tests/ui/run.mjs liste      # nur Tests mit „liste“ im Namen oder Dateinamen
```

Schon installiertes Playwright woanders: `PLAYWRIGHT=/pfad/zu/node_modules/playwright`.
Der MQTT-Test läuft nur mit einem Broker: `MBUSIO_TEST_BROKER=127.0.0.1:1883`. Einen kleinen
Test-Broker startet `node tests/ui/broker.mjs 1883` (braucht `npm i -D aedes`).
Bei einem Fehler liegt ein Bildschirmfoto im Temp-Ordner (`mbusio-ui-<datei>.png`).
