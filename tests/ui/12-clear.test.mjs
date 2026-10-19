import { assert, simLine, primaryScan, scanDone, tab } from "./helpers.mjs";

export default {
  name: "Projekt leeren: Zähler, Auslesungen und letzter Scan weg, Linie bleibt",
  async run({ page, api }) {
    // neue Datenbank: Startprojekt heisst Schnellscan
    assert.equal(await page.textContent("#h-project"), "Schnellscan");
    await simLine(api, "SIMULATOR:ok10");
    await page.reload();
    assert.equal(await primaryScan(page, 0, 12), 10);
    await page.click("#remember-all");
    await page.waitForFunction(() => document.querySelector("[data-tab-go=meters] .ltab-n")?.textContent === "10");
    // während eines Scans abgelehnt, nicht abgebrochen
    await page.fill("#scan-to", "250");
    await page.click("#scan-primary");
    await page.waitForFunction(() => !document.querySelector("#scan-cancel").hidden);
    const pid = (await api("projects")).active.project.id;
    await assert.rejects(api(`projects/${pid}/clear`, "POST"), /belegt/);
    await page.click("#scan-cancel");
    await scanDone(page);
    // Leeren mit Rückfrage
    await tab(page, "project");
    await page.click("#project-clear");
    await page.waitForFunction(() => document.querySelector("#h-project")?.textContent === "Schnellscan");
    await page.waitForFunction(() => !document.querySelector("[data-tab-go=meters] .ltab-n")?.textContent);
    assert.equal((await api("meters")).length, 0);
    assert.equal(await api("last-scan"), null);
    // Anschluss bleibt; neuer Scan im gleichen Bereich hat nichts zum Vergleichen
    assert.equal((await api("projects")).active.line.connection, "SIMULATOR:ok10");
    assert.equal(await primaryScan(page, 0, 12), 10);
    assert.ok(await page.isHidden("#hits-changes"));
  },
};
