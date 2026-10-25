import { assert, tab } from "./helpers.mjs";

// Ausgeblendete Felder (TLS-Zertifikat, Präfix für Home Assistant) behalten ihren Wert, wenn man
// TLS bzw. Home Assistant ausschaltet und speichert
export default {
  name: "MQTT: ausgeblendete Einstellungen bleiben beim Speichern erhalten",
  async run({ page, api }) {
    const f = "[data-out-form=mqtt]";
    const open = async () => {
      await page.click("[data-out-settings=mqtt]");
      await page.waitForSelector(f);
    };
    const save = async () => {
      await page.click(`${f} [type=submit]`);
      await page.waitForSelector(f, { state: "detached" });
    };
    await tab(page, "transfer");
    await open();
    await page.check(`${f} [name=tls]`);
    await page.check(`${f} [name=insecure]`);
    await page.fill(`${f} [name=ca]`, "-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----");
    await page.check(`${f} [name=ha]`);
    await page.fill(`${f} [name=ha_prefix]`, "ha-test");
    await save();
    // TLS und Home Assistant aus: Felder ausgeblendet
    await open();
    await page.uncheck(`${f} [name=tls]`);
    await page.uncheck(`${f} [name=ha]`);
    await save();
    const c = (await api("mqtt")).config;
    assert.equal(c.tls, false);
    assert.equal(c.ha, false);
    assert.match(c.ca, /TEST/, "CA-Zertifikat erhalten");
    assert.equal(c.insecure, true, "Zertifikat nicht prüfen erhalten");
    assert.equal(c.ha_prefix, "ha-test", "Präfix erhalten");
    // wieder einschalten: die alten Werte stehen wieder da
    await open();
    await page.check(`${f} [name=tls]`);
    await page.check(`${f} [name=ha]`);
    assert.match(await page.inputValue(`${f} [name=ca]`), /TEST/);
    assert.equal(await page.inputValue(`${f} [name=ha_prefix]`), "ha-test");
  },
};
