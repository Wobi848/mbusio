import { assert, simLine } from "./helpers.mjs";

// Adressen sind englisch; frühere deutsche Lesezeichen führen an dieselbe Stelle

const shown = (page) => page.evaluate(() => document.querySelector("section.pane:not([hidden])")?.dataset.area);

export default {
  name: "Adressen (englisch, alte Lesezeichen leiten weiter)",
  async setup(api) {
    await simLine(api);
  },
  async run({ page, base }) {
    for (const [now, area] of [["site", "anlage"], ["scan", "scan"], ["meters", "zaehler"], ["transfer", "weitergabe"], ["tools", "werkzeuge"], ["simulator", "simulator"]]) {
      await page.goto(`${base}/#/${now}`);
      await page.waitForSelector(`section[data-area=${area}]:not([hidden])`);
      assert.equal(await page.getAttribute(`.tabs a[data-area=${area}]`, "href"), `#/${now}`);
    }
    for (const [old, now, area] of [["anlage", "site", "anlage"], ["zaehler", "meters", "zaehler"], ["weitergabe", "transfer", "weitergabe"], ["werkzeuge", "tools", "werkzeuge"]]) {
      await page.goto(`${base}/#/${old}`);
      await page.waitForSelector(`section[data-area=${area}]:not([hidden])`);
      assert.equal(new URL(page.url()).hash, `#/${now}`, old);
      assert.equal(await shown(page), area);
    }
    // alte Adresse mit Teilen: Einstellungen der E-Mail gehen auf, wie bisher
    await page.goto(`${base}/#/weitergabe/email`);
    await page.waitForSelector("[data-out-form=email]");
    assert.equal(new URL(page.url()).hash, "#/transfer");
  },
};
