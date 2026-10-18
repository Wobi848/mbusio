import { assert, simLine, scanDone, tab } from "./helpers.mjs";

const title = (page) => page.textContent("#guide .guide-title");

export default {
  name: "Nächster Schritt: Service bis zum Bericht, dann Dauerbetrieb bis zum Ausgang, nur mit der Karte",
  async run({ page, api, ctx }) {
    await tab(page, "project");
    // neu: Anschluss fehlt; das zeigt die Linien-Leiste mit ihrem Knopf, nicht die Leiste unten
    assert.equal(await page.textContent("#lb-action"), "Anschluss festlegen");
    assert.ok(await page.$eval("#lb-action", (b) => b.classList.contains("go")));
    assert.ok(await page.isHidden("#guide"));
    // ohne Simulation kein Reiter Simulator, aber über Werkzeuge erreichbar
    assert.ok(await page.isHidden(".gnav[data-tab-go=sim]"));
    await tab(page, "tools");
    await page.click("#sec-sim-link [data-tab-go=sim]");
    assert.ok(await page.isVisible(".gnav[data-tab-go=sim]"));
    await tab(page, "project");
    assert.ok(await page.isHidden(".gnav[data-tab-go=sim]"));
    assert.ok(await page.isVisible(".ln-noconn"));
    // Knopf der Leiste öffnet das Verbindungsformular darunter
    await page.click("#lb-action");
    assert.ok(await page.isVisible("#sec-conn"));
    // über das Verbindungsfeld verbinden: die Linie hat danach ihren Anschluss (ohne Neuladen)
    await page.selectOption("#port", "SIMULATOR");
    await page.click("#connect");
    await tab(page, "project");
    await page.waitForFunction(() => !document.querySelector(".ln-noconn"));
    // Linie mit Simulator verbunden: suchen, übernehmen, auslesen, weitergeben
    await simLine(api, "SIMULATOR:ok10");
    await page.reload();
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Zähler suchen");
    assert.ok(await page.isVisible(".gnav[data-tab-go=sim]"));
    assert.ok(await page.$eval(".gnav[data-tab-go=project]", (b) => b.classList.contains("done")));
    assert.ok(await page.$eval("#scan-primary", (b) => b.classList.contains("go")));
    await tab(page, "scan");
    await page.fill("#scan-to", "12");
    await page.fill("#scan-retries", "0");
    await page.click("[data-guide-go]");
    await scanDone(page);
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Gefundene Zähler übernehmen");
    await page.click("[data-guide-go]");
    // weiter in der Zählerliste
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Zähler auslesen");
    await page.waitForFunction(() => !document.querySelector("#sec-meters").hidden);
    await page.click("[data-guide-go]");
    // Service: fertig mit Bericht; Weitergabe ist (noch) kein Schritt, Reiter ohne Nummer
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Linie in Ordnung", null, { timeout: 60000 });
    assert.ok(!(await page.$eval(".ltab[data-tab-go=transfer]", (b) => b.classList.contains("next"))), "Weitergabe kein Schritt im Service");
    assert.ok(await page.isHidden("#plant-state"));
    assert.equal(await page.getAttribute("#guide .guide-count", "title"), "Fertig");
    const [rep] = await Promise.all([ctx.waitForEvent("page"), page.click("[data-guide-go]")]);
    await rep.waitForSelector("#table tbody tr");
    await rep.close();
    // Dauerbetrieb einrichten: Plan, Werte, Ausgang
    await page.click("[data-guide-alt]");
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Ausleseplan festlegen");
    assert.equal(await page.textContent("#guide .guide-count"), "1/4", "Dauerbetrieb: vier Schritte");
    await page.click("[data-guide-go]");
    await page.selectOption("#tree-form [name=plan_kind]", "daily");
    await page.click("#tree-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Werte zum Weitergeben wählen");
    assert.ok(await page.isVisible("#plant-state"));
    await page.click("[data-guide-go]");
    await page.waitForFunction(() => !document.querySelector("#sec-transfer").hidden);
    await page.click("[data-guide-go]");
    await page.waitForSelector(".tr-pick .dp-pick-row");
    await page.click("[data-pick-save]");
    // Werte gewählt, Ausgang fehlt noch: Karte öffnet die MQTT-Einstellungen
    await page.waitForFunction(() => document.querySelector("#guide .guide-title")?.textContent === "Ausgang einrichten", null, { timeout: 10000 });
    await page.click("[data-guide-go]");
    await page.waitForSelector('[data-out-form="mqtt"]');
    assert.equal(await page.$$eval(".gnav.done, .ltab.done", (l) => l.length), 3);
  },
};
