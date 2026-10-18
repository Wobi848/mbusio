import { assert, simLine, primaryScan, secondaryScan, visibleSections } from "./helpers.mjs";

export default {
  name: "Scan, Auffälligkeiten, PA ändern, Alle beheben",
  setup: (api) => simLine(api),
  async run({ page }) {
    const n = await primaryScan(page, 0, 12);
    assert.ok(n >= 8, `Primärscan findet Zähler (${n})`);
    assert.match(await page.textContent("#busbar-issues"), /Auffälligkeit/);
    // Zeilen mit Problem hervorgehoben; nur Kollisionen und „keine Daten“: nichts automatisch zu beheben
    assert.ok((await page.locator("#hits tr.issue").count()) > 0);
    const step = () => page.textContent("#guide .guide-title");
    assert.equal(await step(), "Gefundene Zähler übernehmen");
    // Erklärung direkt an der Zeile, Filter „Auffällig“, Satz mit „N Zähler übernehmen“
    assert.ok((await page.locator("#hits tr.issue-strip").count()) > 0);
    assert.match(await page.textContent("#remember-all"), /\d+ Zähler übernehmen/);
    await page.click("[data-hit-filter=attention]");
    assert.equal(await page.locator("#hits tbody tr[class*=issue]:not(.issue-strip)").count(), await page.locator("#hits tbody tr:not(.issue-strip)").count());
    await page.click("[data-hit-filter=all]");
    // Auslesen aus der Trefferliste springt zu Werkzeuge
    await page.locator("#hits [data-read]").first().click();
    await page.waitForSelector(".reading-table", { timeout: 20000 });
    assert.ok((await visibleSections(page)).includes("sec-read"));
    // Sekundärscan, dann PA ändern für den Zähler ohne Adresse
    await secondaryScan(page);
    // doppelte PA lässt sich beheben: zuerst beheben, übernehmen geht trotzdem
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Auffälligkeiten beheben");
    assert.ok(await page.isVisible("#guide [data-guide-alt]"), "trotzdem übernehmen geht");
    await page.locator("#hits tbody tr", { hasText: "55500011" }).locator("[data-fix]").click();
    await page.waitForSelector("#callout");
    await page.click("#set-addr");
    await page.waitForSelector("#callout.done", { timeout: 20000 });
    assert.match(await page.textContent("#callout p"), /55500011/);
    await page.click("#callout a");
    await page.click("[data-fix-all]");
    await page.waitForSelector("#fix-plan [data-plan-run]", { timeout: 60000 });
    // nach dem Beheben: zur Kontrolle neu scannen
    await page.click("#fix-plan [data-plan-run]");
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Zur Kontrolle neu scannen", null, { timeout: 180000 });
  },
};
