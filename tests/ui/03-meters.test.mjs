import { assert, simLine, primaryScan, tab } from "./helpers.mjs";

export default {
  name: "Zählerliste: merken, alle auslesen, Verlauf, Bericht",
  setup: (api) => simLine(api, "SIMULATOR:ok10"),
  async run({ page, ctx, base }) {
    assert.equal(await primaryScan(page, 0, 12), 10);
    assert.match(await page.textContent("#busbar-issues"), /Keine Auffälligkeiten/);
    await page.click("#remember-all");
    await tab(page, "meters");
    await page.waitForFunction(() => document.querySelectorAll("#meters-table tbody tr").length === 10);
    await page.click("#meters-read-all");
    await page.waitForFunction(() => [...document.querySelectorAll("#meters-table .val")].length === 10, null, { timeout: 60000 });
    await page.locator("[data-meter-hist]").first().click();
    await page.waitForSelector("#history svg, #history table", { timeout: 10000 });
    // Bericht öffnet sich mit den Zählern
    const [rep] = await Promise.all([ctx.waitForEvent("page"), page.click("#meters-report")]);
    await rep.waitForSelector("#table tbody tr");
    assert.equal(await rep.locator("#table tbody tr").count(), 10);
    assert.ok(base);
  },
};
