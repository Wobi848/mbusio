import { assert, simLine, tab, primaryScan } from "./helpers.mjs";

export default {
  name: "Simulator: Vorlage verwenden, kopieren, Fehler einbauen",
  setup: (api) => simLine(api),
  async run({ page }) {
    await tab(page, "sim");
    await page.waitForFunction(() => document.querySelectorAll("#sim-select option").length > 0);
    assert.equal(await page.locator("#sim-select option").count(), 4);
    await page.selectOption("#sim-select", "ok10");
    await page.click("#sim-use");
    await page.waitForFunction(() => document.querySelector("#sim-line").textContent.includes("SIMULATOR:ok10"));
    assert.equal(await primaryScan(page, 0, 12), 10);
    await tab(page, "sim");
    page.answers.push("Meine Anlage");
    await page.click("#sim-copy");
    await page.waitForSelector(".sim-table input");
    await page.fill("#sim-n", "2");
    await page.click("[data-sim-add-many]");
    await page.locator(".sim-table tbody tr").last().locator("[name=fault]").selectOption("checksum");
    await page.click("[data-sim-save]");
    await page.waitForFunction(() => document.querySelector("#toast").textContent.includes("gespeichert"));
    await page.click("#sim-use");
    await page.waitForFunction(() => document.querySelector("#sim-line").textContent.includes("meine-anlage"));
    assert.equal(await primaryScan(page, 0, 15), 12);
    assert.match(await page.textContent("#busbar-issues"), /1 Auffälligkeit/);
  },
};
