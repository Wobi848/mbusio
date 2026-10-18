// Gemeinsame Schritte der Browser-Tests
import assert from "node:assert/strict";
export { assert };

/// Linie 1 mit einer Simulation verbinden und aktiv machen
export async function simLine(api, connection = "SIMULATOR", baud = 2400) {
  await api("lines/1", "PUT", { name: "Haus A", connection, baud });
  await api("lines/1/activate", "POST");
}

export const visibleSections = (page) =>
  page.$$eval("main > section", (l) => l.filter((s) => !s.hidden).map((s) => s.id));

/// Einstellungen ⚙ öffnen (Hilfe, Design, Sprache)
export async function settings(page) {
  if (!(await page.$eval("details.settings", (d) => d.open))) await page.click("details.settings > summary");
}

export async function tab(page, name) {
  await page.click(`[data-tab-go=${name}]`);
  await page.waitForTimeout(150);
}

/// Primärscan im Reiter Scan, wartet bis er fertig ist; liefert die Anzahl Zeilen
export async function primaryScan(page, from = 0, to = 20) {
  await tab(page, "scan");
  await page.click("[data-kind=primary]");
  await page.fill("#scan-from", String(from));
  await page.fill("#scan-to", String(to));
  await page.fill("#scan-retries", "0");
  await page.click("#scan-primary");
  await scanDone(page);
  await page.waitForTimeout(300);
  // nur Treffer, nicht die Streifen mit Erklärungen und nicht „fehlt“ aus dem letzten Scan
  return page.locator("#hits tbody tr:not(.issue-strip):not(.missing)").count();
}

export const rowTexts = async (page, sel) => (await page.locator(sel).allInnerTexts()).map((x) => x.replace(/\s+/g, " ").trim());

/// Sekundärscan im Reiter Scan starten (Art umschalten, dann starten)
export async function secondaryScan(page) {
  await tab(page, "scan");
  await page.click("[data-kind=secondary]");
  await page.click("#scan-secondary");
  await scanDone(page);
}

/// Warten, bis ein gerade gestarteter Scan fertig ist (erst läuft er, dann nicht mehr)
export async function scanDone(page, timeout = 300000) {
  await page.waitForFunction(() => !document.querySelector("#scan-cancel").hidden, null, { timeout: 10000 }).catch(() => {});
  await page.waitForFunction(() => document.querySelector("#scan-cancel").hidden, null, { timeout });
}
