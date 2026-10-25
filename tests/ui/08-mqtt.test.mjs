import { assert, simLine, tab } from "./helpers.mjs";

// nur mit Broker: MBUSIO_TEST_BROKER=127.0.0.1:1883
const broker = process.env.MBUSIO_TEST_BROKER;

export default {
  name: "MQTT: Ausgang einrichten, Datenpunkt senden, Home Assistant",
  skip: () => !broker,
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
    // weitergegeben wird nur aus Linien im Dauerbetrieb
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, api }) {
    const [host, port] = broker.split(":");
    await tab(page, "transfer");
    await page.click("[data-out-settings=mqtt]");
    const f = "[data-out-form=mqtt]";
    await page.check(`${f} [name=enabled]`);
    await page.fill(`${f} [name=host]`, host);
    await page.fill(`${f} [name=port]`, port || "1883");
    await page.click(`${f} [type=submit]`);
    await page.waitForFunction(() => document.querySelector("#tr-outputs")?.textContent.includes("verbunden"), null, { timeout: 15000 });
    await page.locator("[data-dp-out=mqtt]").first().check();
    // bestätigt gesendet: Warteschlange leer
    const sentAtLeast = async (n, what) => {
      for (let i = 0; i < 50; i++) {
        const s = (await api("mqtt")).status;
        if (s.sent >= n && s.queued === 0) return s.sent;
        await page.waitForTimeout(200);
      }
      assert.fail(what);
    };
    const sent = await sentAtLeast(1, "Datenpunkt nicht gesendet");
    // Home Assistant: Präfix erst mit Häkchen sichtbar, dann eine Konfig-Nachricht mehr
    await page.click("[data-out-settings=mqtt]");
    assert.ok(await page.locator(`${f} [data-field=ha_prefix]`).isHidden());
    await page.check(`${f} [name=ha]`);
    assert.ok(await page.locator(`${f} [data-field=ha_prefix]`).isVisible());
    await page.click(`${f} [type=submit]`);
    await page.waitForSelector(f, { state: "detached" });
    await sentAtLeast(sent + 1, "Konfig für Home Assistant nicht gesendet");
    assert.equal((await api("mqtt")).config.ha, true);
  },
};
