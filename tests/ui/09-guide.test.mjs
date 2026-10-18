import { assert, simLine, scanDone, tab, visibleSections } from "./helpers.mjs";

const title = (page) => page.textContent("#guide h2");

export default {
  name: "Nächster Schritt: vom leeren Projekt bis zur Weitergabe nur mit dem Knopf der Karte",
  async run({ page, api }) {
    await tab(page, "project");
    // neu: Anschluss fehlt, Knopf führt zum Verbindungsfeld
    assert.equal(await title(page), "Anschluss festlegen");
    assert.ok(await page.isVisible(".ln-noconn"));
    await page.click("[data-guide-go]");
    await page.waitForTimeout(300);
    assert.ok((await visibleSections(page)).includes("sec-conn"));
    // ausblenden gilt nur für diesen Schritt
    await page.click("[data-guide-hide]");
    assert.ok(await page.isHidden("#guide"));
    // Linie mit Simulator verbunden: suchen, übernehmen, auslesen, weitergeben
    await simLine(api, "SIMULATOR:ok10");
    await page.reload();
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Zähler suchen");
    assert.ok(await page.$eval("[data-tab-go=project]", (b) => b.classList.contains("done")));
    assert.ok(await page.$eval("#scan-primary", (b) => b.classList.contains("go")));
    await page.fill("#scan-to", "12");
    await page.fill("#scan-retries", "0");
    await page.click("[data-guide-go]");
    await scanDone(page);
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Gefundene Zähler übernehmen");
    await page.click("[data-guide-go]");
    // weiter in der Zählerliste
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Zähler auslesen");
    await page.waitForFunction(() => !document.querySelector("#sec-meters").hidden);
    await page.click("[data-guide-go]");
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Werte zum Weitergeben wählen", null, { timeout: 60000 });
    await page.click("[data-guide-go]");
    await page.waitForFunction(() => !document.querySelector("#sec-transfer").hidden);
    await page.click("[data-guide-go]");
    await page.waitForSelector(".tr-pick .dp-pick-row");
    await page.click("[data-pick-save]");
    // alles eingerichtet: keine Karte mehr, alle vier Reiter erledigt
    await page.waitForFunction(() => document.querySelector("#guide").hidden, null, { timeout: 10000 });
    assert.equal(await page.$$eval(".tab.done", (l) => l.length), 4);
  },
};
