import { assert, scan, simLine } from "./helpers.mjs";

// Statusleiste zum Anklicken: Bus → Bus-Konsole mit den Telegrammen live (Telegramm → erklärt in
// Werkzeuge), Auffälligkeiten → im Scan gefiltert, die erste gewählt

export default {
  name: "Statusleiste (Bus-Konsole, Telegramm erklären, Auffälligkeiten zeigen)",
  setup: (api) => simLine(api, "SIMULATOR:praxis"),
  async run({ page, base, api }) {
    await page.goto(base + "/#/zaehler");
    await page.waitForSelector("#bus .dot");

    // Bus anklicken: Konsole geht auf, eine Auslesung erscheint Telegramm für Telegramm
    await page.click("#bus");
    await page.waitForSelector("#console:not([hidden])");
    assert.equal(await page.getAttribute("#bus", "aria-expanded"), "true");
    await api("meters", "POST", { address: "1" });
    await api("meters/1/read", "POST");
    await page.waitForSelector("#console-log .d-TX", { timeout: 10000 });
    await page.waitForSelector("#console-log .d-RX");

    // Telegramm anklicken: Werkzeuge erklären es, Konsole zu
    await page.locator("#console-log .d-RX[data-hex]").last().click();
    await page.waitForSelector(".tabs [data-area=werkzeuge][aria-current=page]");
    await page.waitForSelector("section[data-area=werkzeuge] [data-analysis]:not([hidden])");
    assert.ok(await page.isHidden("#console"));

    // Escape schliesst die Konsole
    await page.click("#bus");
    await page.waitForSelector("#console:not([hidden])");
    await page.keyboard.press("Escape");
    await page.waitForSelector("#console", { state: "hidden" });

    // Auffälligkeiten anklicken: im Scan gefiltert, die erste gewählt mit Ursache rechts
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "12");
    await page.press("[data-to]", "Tab");
    await scan(page);
    await page.goto(base + "/#/zaehler");
    await page.click("#issues [data-show-issues]");
    await page.waitForSelector("[data-filter=attention][aria-pressed=true]");
    await page.waitForSelector("section[data-area=scan] [role=option].active");
    assert.match(await page.textContent("section[data-area=scan] [data-detail]"), /Kollision|Keine Daten/);

    // mbusio antwortet nicht (beendet, Pi neu gestartet): Kopf und Statusleiste sagen es, statt
    // „Verbunden“ stehen zu lassen; antwortet er wieder, ist alles wie vorher
    await page.route("**/api/status", (r) => r.abort());
    await page.waitForFunction(() => document.querySelector("#bus")?.textContent.includes("mbusio antwortet nicht"), null, { timeout: 12000 });
    assert.match(await page.textContent("#conn-state"), /mbusio antwortet nicht/);
    await page.unroute("**/api/status");
    await page.waitForFunction(() => document.querySelector("#conn-state")?.textContent.includes("Verbunden"), null, { timeout: 12000 });
  },
};
