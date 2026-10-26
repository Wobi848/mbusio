import { assert, simLine } from "./helpers.mjs";

// MQTT: Einstellungen bleiben beim Aus- und Einschalten von TLS und Home Assistant
// erhalten; mit Broker (MBUSIO_TEST_BROKER=127.0.0.1:1883) zusätzlich senden und Home Assistant

const broker = process.env.MBUSIO_TEST_BROKER;
const detail = "section[data-area=weitergabe] [data-detail]";

async function settings(page, values) {
  await page.click("[data-out-settings=mqtt]");
  for (const [name, v] of Object.entries(values)) {
    if (typeof v === "boolean") await page.setChecked(`${detail} [name=${name}]`, v);
    else await page.fill(`${detail} [name=${name}]`, v);
  }
  await page.click(`${detail} [type=submit]`);
  await page.waitForSelector(`${detail} [data-out-form]`, { state: "detached" });
}

export default {
  name: "MQTT (Einstellungen bleiben, mit Broker senden und Home Assistant)",
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
    // weitergegeben wird nur aus Linien im Dauerbetrieb
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/weitergabe");
    await settings(page, { tls: true, insecure: true, ca: "-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----", ha: true, ha_prefix: "ha-test" });
    await settings(page, { tls: false, ha: false });
    const c = (await api("mqtt")).config;
    assert.deepEqual([c.tls, c.ha, c.insecure, c.ha_prefix], [false, false, true, "ha-test"]);
    assert.match(c.ca, /TEST/, "CA-Zertifikat erhalten");
    if (!broker) return;

    // mit Broker: einschalten, Datenpunkt an MQTT, bestätigt gesendet
    const [host, port] = broker.split(":");
    await settings(page, { host, port: port || "1883" });
    await page.click("[data-out-toggle=mqtt]");
    await page.waitForFunction(() => document.querySelector("[data-outputs]")?.textContent.includes("läuft"), null, { timeout: 15000 });
    await page.focus("section[data-area=weitergabe] [data-list]");
    await page.keyboard.press("Control+a");
    await page.click("[data-bulk='out:mqtt:on']");
    const sentAtLeast = async (n, what) => {
      for (let i = 0; i < 50; i++) {
        const s = (await api("mqtt")).status;
        if (s.sent >= n && s.queued === 0) return s.sent;
        await page.waitForTimeout(200);
      }
      assert.fail(what);
    };
    const sent = await sentAtLeast(1, "Datenpunkt nicht gesendet");
    // im Protokoll als gesendet
    await page.click("[data-open-log]");
    await page.waitForFunction(() => /gesendet/.test(document.querySelector("[data-log-rows]")?.textContent), null, { timeout: 10000 });
    await page.click("[data-log-back]");
    // Home Assistant ein: eine Konfig-Nachricht mehr
    await settings(page, { ha: true });
    await sentAtLeast(sent + 1, "Konfig für Home Assistant nicht gesendet");
  },
};
