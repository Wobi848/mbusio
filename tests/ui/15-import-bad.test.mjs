import { assert } from "./helpers.mjs";

// Kaputte Dateien beim Import: jede Stelle sagt, was sie erwartet hat
export default {
  name: "Import: unlesbare Projektdatei und Zählerliste mit passender Meldung",
  async run({ page }) {
    const bad = { name: "kaputt.json", mimeType: "application/json", buffer: Buffer.from("{ kein json") };
    await page.setInputFiles("#project-import", bad);
    await page.waitForSelector("#toast.error:not([hidden])");
    assert.match(await page.textContent("#toast"), /keine Projektdatei/);
    // Zählerliste als JSON (Reiter Zähler): gültiges JSON, aber keine Liste
    await page.evaluate(() => ($("toast").hidden = true));
    await page.setInputFiles("#meters-import", { name: "liste.json", mimeType: "application/json", buffer: Buffer.from('{"x": 1}') });
    await page.waitForSelector("#toast.error:not([hidden])");
    assert.match(await page.textContent("#toast"), /keine Zählerliste/);
  },
};
