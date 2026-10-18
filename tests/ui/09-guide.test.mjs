import { assert, simLine, scanDone, tab, visibleSections } from "./helpers.mjs";

const title = (page) => page.textContent("#guide h2");

export default {
  name: "Nächster Schritt: Service bis zum Bericht, dann Dauerbetrieb bis zum Ausgang, nur mit der Karte",
  async run({ page, api, ctx }) {
    await tab(page, "project");
    // neu: Anschluss fehlt, Knopf führt zum Verbindungsfeld
    assert.equal(await title(page), "Anschluss festlegen");
    // ohne Simulation kein Reiter Simulator, aber über Werkzeuge erreichbar
    assert.ok(await page.isHidden(".tab[data-tab-go=sim]"));
    await tab(page, "tools");
    await page.click("#sec-sim-link [data-tab-go=sim]");
    assert.ok(await page.isVisible(".tab[data-tab-go=sim]"));
    await tab(page, "project");
    assert.ok(await page.isHidden(".tab[data-tab-go=sim]"));
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
    assert.ok(await page.isVisible(".tab[data-tab-go=sim]"));
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
    // Service: fertig mit Bericht; Weitergabe ist (noch) kein Schritt, Reiter ohne Nummer
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Linie in Ordnung", null, { timeout: 60000 });
    assert.ok(await page.isHidden("[data-tab-go=transfer] .tab-no"));
    assert.ok(await page.isHidden("#plant-state"));
    assert.match(await page.textContent("#guide .guide-eyebrow"), /Fertig/);
    const [rep] = await Promise.all([ctx.waitForEvent("page"), page.click("[data-guide-go]")]);
    await rep.waitForSelector("#table tbody tr");
    await rep.close();
    // Dauerbetrieb einrichten: Plan, Werte, Ausgang
    await page.click("[data-guide-alt]");
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Ausleseplan festlegen");
    assert.ok(await page.isVisible("[data-tab-go=transfer] .tab-no"));
    await page.click("[data-guide-go]");
    await page.selectOption("#tree-form [name=plan_kind]", "daily");
    await page.click("#tree-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Werte zum Weitergeben wählen");
    assert.ok(await page.isVisible("#plant-state"));
    await page.click("[data-guide-go]");
    await page.waitForFunction(() => !document.querySelector("#sec-transfer").hidden);
    await page.click("[data-guide-go]");
    await page.waitForSelector(".tr-pick .dp-pick-row");
    await page.click("[data-pick-save]");
    // Werte gewählt, Ausgang fehlt noch: Karte öffnet die MQTT-Einstellungen
    await page.waitForFunction(() => document.querySelector("#guide h2")?.textContent === "Ausgang einrichten", null, { timeout: 10000 });
    await page.click("[data-guide-go]");
    await page.waitForSelector('[data-out-form="mqtt"]');
    assert.equal(await page.$$eval(".tab.done", (l) => l.length), 3);
  },
};
