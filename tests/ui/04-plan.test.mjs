import { assert, simLine, tab, rowTexts } from "./helpers.mjs";

export default {
  name: "Ausleseplan: Projekt, Linie, Zähler, Uhrzeit",
  args: ["--untrusted-clock"],
  async setup(api) {
    await simLine(api);
    for (const a of ["1", "2", "3"]) await api("meters", "POST", { address: a, name: "Z" + a });
  },
  async run({ page }) {
    await tab(page, "project");
    await page.click("#project-edit");
    await page.selectOption("#tree-form [name=plan_kind]", "daily");
    assert.equal(await page.inputValue("#tree-form [name=plan_time]"), "00:05");
    // immer 24 h: Kurzform wird ergänzt, Ungültiges abgelehnt
    await page.fill("#tree-form [name=plan_time]", "25:00");
    await page.click("#tree-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#toast").textContent.includes("24 h"));
    await page.fill("#tree-form [name=plan_time]", "1730");
    await page.locator("#tree-form [name=plan_time]").blur();
    assert.equal(await page.inputValue("#tree-form [name=plan_time]"), "17:30");
    await page.fill("#tree-form [name=plan_time]", "5");
    await page.locator("#tree-form [name=plan_time]").blur();
    assert.equal(await page.inputValue("#tree-form [name=plan_time]"), "05:00");
    await page.fill("#tree-form [name=plan_time]", "0:05");
    await page.click("#tree-form [type=submit]");
    await page.waitForFunction(() => document.querySelector(".ln-plan")?.textContent.includes("täglich 00:05"));
    // Linie: eigener Plan
    await page.locator(".ln").first().locator("[data-ln-edit]").click();
    await page.selectOption("#tree-form [name=plan_kind]", "15min");
    await page.click("#tree-form [type=submit]");
    await page.waitForFunction(() => document.querySelector(".ln-plan")?.textContent.includes("alle 15 min"));
    // Zählerliste zeigt den geerbten Plan; einen Zähler ausschalten
    await tab(page, "meters");
    await page.waitForFunction(() => !document.querySelector("#th-plan").hidden);
    await page.locator("[data-meter-edit]").nth(1).click();
    await page.selectOption("#mf-plan [name=plan_kind]", "off");
    await page.click("#meter-form [type=submit]");
    await page.waitForTimeout(600);
    const rows = await rowTexts(page, "#meters-table tbody tr");
    assert.match(rows[0], /alle 15 min/);
    assert.match(rows[1], /\baus\b/);
    // ohne Zeitquelle: Hinweis unten, Uhrzeit übernehmen
    await page.waitForSelector("#busbar-time:not([hidden])", { timeout: 15000 });
    await page.click("#busbar-time");
    await page.waitForSelector("#busbar-time[hidden]", { state: "attached", timeout: 10000 });
  },
};
