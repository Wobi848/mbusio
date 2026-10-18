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
    // Zusammenfassung: wie viele antworten
    await page.waitForFunction(() => document.querySelector("#meters-job").textContent.includes("Alle 10 Zähler antworten"));
    // Suche und Filter an der Tabelle
    await page.fill("#meters-search", "1000000");
    await page.waitForFunction(() => document.querySelectorAll("#meters-table tbody tr[data-row]").length < 10);
    await page.fill("#meters-search", "");
    assert.match(await page.textContent("[data-filter=noname]"), /10/);
    await page.click("[data-filter=err]");
    assert.equal(await page.locator("#meters-table tbody tr[data-row]").count(), 0);
    await page.click("[data-filter=all]");
    // Bearbeiten klappt in der Zeile auf, gleicher Knopf schliesst
    await page.locator("[data-meter-edit]").nth(2).click();
    assert.ok(await page.isVisible("#meters-table tr.edit-row #meter-form"));
    assert.equal(await page.locator("#meters-table tr.sel").count(), 1);
    await page.locator("[data-meter-edit]").nth(2).click();
    assert.ok(await page.isHidden("#meter-form"));
    // Werkzeuge: ohne Ziel gesperrt, Ziel aus der Zählerliste wählen
    await tab(page, "tools");
    assert.ok(await page.isDisabled("#set-addr"));
    await page.click("details.target-menu > summary");
    await page.locator("[data-target]").first().click();
    assert.ok(await page.isVisible("#target-meter .mtr"));
    assert.ok(await page.isEnabled("#set-addr"));
    assert.match(await page.textContent("#cur-pa"), /PA \d+/);
    await tab(page, "meters");
    await page.locator("[data-meter-hist]").first().click();
    await page.waitForSelector("#history svg, #history table", { timeout: 10000 });
    // Bericht öffnet sich mit den Zählern
    const [rep] = await Promise.all([ctx.waitForEvent("page"), page.click("#meters-report")]);
    await rep.waitForSelector("#table tbody tr");
    assert.equal(await rep.locator("#table tbody tr").count(), 10);
    assert.ok(base);
  },
};
