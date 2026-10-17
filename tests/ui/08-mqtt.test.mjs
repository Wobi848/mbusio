import { assert, simLine, tab } from "./helpers.mjs";

// nur mit Broker: MBUSIO_TEST_BROKER=127.0.0.1:1883
const broker = process.env.MBUSIO_TEST_BROKER;

export default {
  name: "MQTT: Broker einrichten, Datenpunkt senden",
  skip: () => !broker,
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
  },
  async run({ page, api }) {
    const [host, port] = broker.split(":");
    await tab(page, "meters");
    await page.check("#mq-enabled");
    await page.fill("#mq-host", host);
    await page.fill("#mq-port", port || "1883");
    await page.click("#mqtt-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#mqtt-chip").textContent === "verbunden", null, { timeout: 15000 });
    await page.locator("[data-dp-mqtt]").first().check();
    // bestätigt gesendet: Warteschlange leer
    for (let i = 0; i < 50; i++) {
      const s = (await api("mqtt")).status;
      if (s.sent >= 1 && s.queued === 0) return;
      await page.waitForTimeout(200);
    }
    assert.fail("Datenpunkt nicht gesendet");
  },
};
