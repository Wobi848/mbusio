import { assert, simLine } from "./helpers.mjs";

// Zählerliste: Häkchen im Kopf wählt alle gezeigten, nochmals keinen; teilweise gewählt zeigt „–“

const zl = "section[data-area=zaehler]";
const all = `${zl} [data-select-all]`;

export default {
  name: "Alle Zähler wählen (Häkchen im Kopf)",
  async setup(api) {
    await simLine(api);
    for (const address of ["1", "2", "3"]) await api("meters", "POST", { address, name: `Whg ${address}` });
  },
  async run({ page, base }) {
    // erster Klick auf ein einzelnes Kästchen wählt genau diese Zeile
    await page.goto(`${base}/#/meters`);
    await page.waitForSelector(all);
    await page.locator(`${zl} [data-list] [data-box]`).nth(1).click();
    assert.equal(await page.locator(`${zl} [data-list] [role=option]`).nth(1).getAttribute("aria-selected"), "true");
    await page.keyboard.press("Escape");
    const selected = () => page.locator(`${zl} [data-list] [role=option][aria-selected=true]`).count();
    for (const width of [1250, 390]) {
      await page.setViewportSize({ width, height: 800 });
      await page.goto(`${base}/#/meters`);
      await page.reload();
      await page.waitForSelector(all);
      assert.equal(await page.getAttribute(all, "aria-checked"), "false");

      await page.click(all);
      assert.equal(await selected(), 3, "alle");
      assert.equal(await page.getAttribute(all, "aria-checked"), "true");
      assert.ok(await page.isVisible(`${zl} [data-sel-read]`), "Aktionen für die Auswahl");

      await page.locator(`${zl} [data-list] [data-box]`).first().click();
      assert.equal(await selected(), 2);
      assert.equal(await page.getAttribute(all, "aria-checked"), "mixed");

      await page.click(all);
      assert.equal(await selected(), 3, "teilweise: wählt alle");
      await page.click(all);
      assert.equal(await selected(), 0, "nochmals: keinen");
    }
    // Filter: nur die gezeigten
    await page.setViewportSize({ width: 1250, height: 800 });
    await page.fill(`${zl} [data-search]`, "Whg 2");
    await page.waitForFunction((z) => document.querySelectorAll(`${z} [data-list] [role=option]`).length === 1, zl);
    await page.click(all);
    assert.equal(await selected(), 1);
    await page.fill(`${zl} [data-search]`, "");
    await page.waitForFunction((z) => document.querySelectorAll(`${z} [data-list] [role=option]`).length === 3, zl);
    assert.equal(await selected(), 1, "die übrigen bleiben ungewählt");
  },
};
