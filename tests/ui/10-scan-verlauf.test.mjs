import { assert, scan, scanDone, simLine } from "./helpers.mjs";

// Scan über die Zeit: Vergleich mit dem letzten Scan, Sekundärscan eingrenzen,
// abbrechen und weitersuchen, Projekt leeren

const sec = "section[data-area=scan]";
const rows = `${sec} [data-list] [role=option]`;
const hitIds = (page) => page.$$eval(rows, (l) => l.map((r) => (r.textContent.match(/\b(\d{8})\b/) || [])[1]).filter(Boolean));

async function range(page, from, to) {
  await page.fill("[data-from]", String(from));
  await page.fill("[data-to]", String(to));
  await page.press("[data-to]", "Tab");
}

export default {
  name: "Scan-Vergleich, Sekundärscan mit Weitersuchen, Projekt leeren",
  async setup(api) {
    const r = await api("sim", "POST", { name: "Vergleich", from: "ok10" });
    await simLine(api, `SIMULATOR:${r.key}`, 2400, 40);
  },
  async run({ page, base, api }) {
    const key = (await api("projects")).active.line.connection.split(":")[1];
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-to]");

    // erster Scan: nichts zu vergleichen; gleicher Bus, gleicher Bereich: keine Änderung
    await range(page, 0, 12);
    await scan(page);
    assert.equal(await page.locator(rows).count(), 10);
    assert.equal(await page.textContent("[data-changes]"), "");
    await scan(page);
    assert.equal(await page.textContent("[data-changes]"), "");

    // am Bus: ein Zähler weg, einer hat eine andere PA → „1 fehlt“ (welcher: im Tooltip), „1 geändert“
    const sim = (await api("sim")).scenarios.find((s) => s.key === key);
    const gone = sim.meters[0];
    const meters = sim.meters.slice(1).map((m, i) => (i === 0 ? { ...m, primary: 12 } : m));
    await api(`sim/${key}`, "PUT", { name: sim.name, meters });
    await scan(page);
    const changes = await page.textContent("[data-changes]");
    assert.match(changes, /1 fehlt/, changes);
    assert.match(changes, /1 geändert/, changes);
    assert.match(await page.getAttribute("[data-missing]", "title"), new RegExp(gone.id));
    assert.match(await page.textContent(rows + ":has-text('→ 12')"), /PA \d+ → 12/);

    // anderer Bereich: kein Vergleich
    await range(page, 0, 20);
    await scan(page);
    assert.equal(await page.textContent("[data-changes]"), "");

    // Sekundärscan: nur IDs mit diesem Anfang
    const ids = meters.map((m) => m.id);
    await page.click("[data-kind=secondary]");
    const prefix = ids[0].slice(0, 3);
    await page.fill("[data-prefix]", prefix);
    await page.press("[data-prefix]", "Tab");
    await scan(page);
    assert.deepEqual((await hitIds(page)).sort(), ids.filter((id) => id.startsWith(prefix)).sort());

    // ganzer Bus, mittendrin abbrechen, dann weitersuchen: am Ende alle, keiner doppelt
    await page.fill("[data-prefix]", "");
    await page.press("[data-prefix]", "Tab");
    await page.keyboard.press("s");
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 3, rows, { timeout: 60000 });
    await page.click("[data-start]");
    await scanDone(page);
    await page.waitForSelector("[data-resume]:not([hidden])");
    const before = (await hitIds(page)).length;
    assert.ok(before < ids.length, `abgebrochen nach ${before}`);
    await page.click("[data-resume]");
    await scanDone(page, 300000);
    assert.deepEqual((await hitIds(page)).sort(), [...ids].sort());
    assert.ok(await page.isHidden("[data-resume]"));

    // Projekt leeren: während eines Scans abgelehnt (nicht abgebrochen)
    await page.click("[data-kind=primary]");
    await range(page, 0, 250);
    await page.keyboard.press("s");
    await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent === "Abbrechen");
    const pid = (await api("projects")).active.project.id;
    await assert.rejects(api(`projects/${pid}/clear`, "POST"), /belegt/);
    await page.click("[data-start]");
    await scanDone(page);
    // Leeren mit Rückfrage am Ort: Zähler und letzter Scan weg, Anschluss bleibt
    await api("meters", "POST", { address: "2", name: "weg" });
    await page.goto(base + "/#/anlage");
    await page.locator("section[data-area=anlage] [data-list] [role=option]").first().click();
    await page.click("[data-clear]");
    await page.click(".ask button.danger");
    await page.waitForFunction(() => !document.querySelector(".tabs #n-meters")?.textContent);
    assert.equal((await api("meters")).length, 0);
    assert.equal(await api("last-scan"), null);
    assert.equal((await api("projects")).active.line.connection, `SIMULATOR:${key}`);
    // neuer Scan hat nichts zum Vergleichen
    await page.goto(base + "/#/scan");
    await range(page, 0, 12);
    await scan(page);
    assert.equal(await page.textContent("[data-changes]"), "");
  },
};
