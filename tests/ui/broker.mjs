// Kleiner MQTT-Broker für den MQTT-Test (CI und lokal): node tests/ui/broker.mjs [port]
// Dann: MBUSIO_TEST_BROKER=127.0.0.1:<port> node tests/ui/run.mjs
import { createRequire } from "node:module";
import net from "node:net";

const require = createRequire(import.meta.url);
const aedes = require("aedes")();
const port = Number(process.argv[2] || 1883);
net.createServer(aedes.handle).listen(port, "127.0.0.1", () => console.log(`MQTT-Broker auf 127.0.0.1:${port}`));
