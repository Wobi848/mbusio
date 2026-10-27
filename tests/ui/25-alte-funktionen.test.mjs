import { assert, scan, simLine } from "./helpers.mjs";

// Funktionen der bisherigen Oberfläche, die nach dem Umbau gefehlt haben: Verlauf als Tabelle mit
// Differenz und pro Tag, Scan-Treffer sortieren, „ausgesetzt bis“ am Zähler, Erklärungen anzeigen

const zl = "section[data-area=zaehler]";
const pas = (page) => page.$$eval("section[data-area=scan] [data-list] [role=option] > :first-child", (els) => els.map((e) => Number(e.textContent)));

export default {
  name: "Alte Funktionen (Verlauf-Tabelle, Treffer sortieren, ausgesetzt, Erklärungen)",
  async setup(api) {
    const sim = `SIMULATOR:${(await api("sim", "POST", { name: "Alt", from: "ok10" })).key}`;
    await simLine(api, sim, 2400, 50);
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
    await api(`meters/${m.id}/read`, "POST");
  },
  async run({ page, base }) {
    // Verlauf: Zusammenfassung, Tabelle neueste zuerst mit Differenz, zurück zum Diagramm
    await page.goto(base + "/#/zaehler");
    await page.locator(`${zl} [data-list] [role=option]`).first().click();
    await page.click(`${zl} [data-detail] [data-history]`);
    await page.waitForSelector(`${zl} .hsum`);
    assert.match(await page.textContent(`${zl} .hsum`), /Erste.*Letzte.*Differenz/s);
    await page.click(`${zl} [data-hist-view=table]`);
    await page.waitForSelector(`${zl} .htable tbody tr`);
    const rows = await page.$$eval(`${zl} .htable tbody tr`, (trs) => trs.map((tr) => [...tr.cells].map((c) => c.textContent.trim())));
    assert.equal(rows.length, 2);
    assert.equal(rows[1][2], "", "älteste Auslesung ohne Differenz");
    assert.notEqual(rows[0][2], "", "neueste mit Differenz");
    await page.click(`${zl} [data-hist-view=chart]`);
    await page.waitForSelector(`${zl} [data-chart] svg`);

    // ausgesetzter Zähler: Hinweis in der Zeile (Zustand wie vom Server nach „zu viele Auslesungen“)
    await page.route("**/api/meters", async (route) => {
      const r = await route.fetch();
      const list = await r.json();
      list[0].plan_info = { ...list[0].plan_info, paused_until: "2026-10-28 00:05" };
      await route.fulfill({ response: r, json: list });
    });
    await page.reload();
    await page.waitForFunction(() => document.querySelector("section[data-area=zaehler] [data-list]")?.textContent.includes("ausgesetzt bis"));
    await page.unroute("**/api/meters");

    // Scan: Klick auf die Spalte sortiert auf, ab, dann wieder wie gescannt; bleibt nach dem Neuladen
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "12");
    await page.press("[data-to]", "Tab");
    await scan(page);
    const scanned = await pas(page);
    assert.ok(scanned.length >= 3, `Treffer: ${scanned}`);
    const asc = [...scanned].sort((a, b) => a - b);
    await page.click("[data-sort=pa]");
    await page.click("[data-sort=pa]");
    assert.deepEqual(await pas(page), [...asc].reverse(), "absteigend");
    await page.reload();
    await page.waitForSelector("[data-sort=pa].on");
    assert.deepEqual(await pas(page), [...asc].reverse(), "Sortierung bleibt");
    await page.click("[data-sort=pa]");
    assert.deepEqual(await pas(page), scanned, "wie gescannt");

    // Weitergabe: Abrechnung, Protokoll und Einstellungen eines Ausgangs sind nie gleichzeitig offen
    await page.goto(base + "/#/weitergabe");
    await page.click("[data-open-billing]");
    await page.waitForSelector("[data-billview]:not([hidden])");
    // zwei Felder schnell nacheinander: beide gespeichert, das Getippte bleibt im Feld
    await page.selectOption("[data-b-period]", "custom");
    await page.waitForSelector("[data-b-from]");
    await page.fill("[data-b-from]", "2026-10-01");
    await page.press("[data-b-from]", "Tab");
    await page.fill("[data-b-to]", "2026-10-15");
    await page.press("[data-b-to]", "Tab");
    await page.fill("[data-b-subject]", "Monat {zeitraum}");
    await page.waitForFunction(async () => {
      const s = (await (await fetch("/api/projects/1/billing")).json()).settings;
      return s.from === "2026-10-01" && s.to === "2026-10-15";
    });
    assert.equal(await page.inputValue("[data-b-subject]"), "Monat {zeitraum}");
    await page.click("[data-open-log]");
    await page.waitForSelector("[data-logview]:not([hidden])");
    assert.equal(await page.isVisible("[data-billview]"), false, "Abrechnung zu, wenn das Protokoll aufgeht");
    await page.click("[data-out-settings=mqtt]");
    await page.waitForSelector("[data-out-form=mqtt]");
    assert.equal(await page.isVisible("[data-logview]"), false, "Protokoll zu, wenn Einstellungen aufgehen");

    // Erklärungen: aus, bis man sie in den Einstellungen einschaltet; bleibt eingeschaltet
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-sort=pa]");
    const explain = "section[data-area=scan] .explain";
    assert.equal(await page.isVisible(explain), false);
    await page.click("#settings");
    await page.check("#help-toggle");
    assert.match(await page.textContent(explain), /Primäradresse/);
    assert.ok(await page.isVisible(explain));
    await page.goto(base + "/#/zaehler");
    await page.reload();
    await page.waitForSelector(`${zl} .explain`, { state: "visible" });
    await page.click("#settings");
    await page.uncheck("#help-toggle");
    assert.equal(await page.isVisible(`${zl} .explain`), false);
  },
};
