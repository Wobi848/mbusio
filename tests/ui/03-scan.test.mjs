import { assert, scanDone, simLine } from "./helpers.mjs";

// Bereich Scan (Suchzeile, Treffer zeilenweise, Zustand am Treffer,
// ein Hauptknopf: beheben → merken → alle in Liste)

const rows = (page) => page.locator("section[data-area=scan] [data-list] [role=option]");
export default {
  name: "Scan (Treffer, Klick während des Scans, merken, beheben)",
  async setup(api) {
    await simLine(api, "SIMULATOR:praxis");
  },
  async run({ page, base, api }) {
    // ohne Adresse und ohne Zähler beginnt die Oberfläche im Scan
    await page.goto(base + "/");
    await page.waitForFunction(() => location.hash === "#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "20");
    await page.press("[data-to]", "Tab");

    // Klick auf eine Zeile während des Scans bleibt (Zeilen werden nicht ersetzt)
    await page.keyboard.press("s");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=scan] [data-list] [role=option]").length >= 2);
    await rows(page).nth(1).click();
    await page.waitForSelector("section[data-area=scan] [role=option].active");
    await scanDone(page);
    assert.equal(await rows(page).count(), 16);
    assert.ok(await page.isVisible("section[data-area=scan] [role=option].active"), "Auswahl während des Scans verloren");

    // Zustand am Treffer, Filter mit Anzahl
    assert.match(await rows(page).nth(0).textContent(), /Kollision/);
    assert.match(await page.textContent("[data-filter=attention]"), /8/);
    await rows(page).nth(0).click();
    await page.waitForSelector("section[data-area=scan] [data-detail] [data-secondary]");

    // ein Hauptknopf: neue merken → danach „✓ in Liste“ und „alle in der Zählerliste“
    await page.click("[data-merge]");
    await page.waitForFunction(() => document.querySelector("[data-main]")?.textContent.includes("Zählerliste"));
    assert.equal((await api("meters")).length, 10);
    assert.match(await rows(page).nth(1).textContent(), /in Liste/);

    // Beheben: Zähler mit anderer Baudrate auf die Bus-Baudrate umstellen
    await api("lines/1", "PUT", { name: "Haus A", connection: "SIMULATOR:bauds", baud: 2400 });
    await api("lines/1/activate", "POST");
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "8");
    await page.press("[data-to]", "Tab");
    await page.click("[data-baud-btn]");
    for (const b of ["300", "9600"]) await page.check(`[data-baud-menu] [data-baud][value="${b}"]`);
    await page.keyboard.press("Escape");
    await page.click("[data-start]");
    await scanDone(page);
    await page.waitForSelector("[data-fix]");
    assert.match(await page.textContent("[data-fix]"), /4 beheben/);
    await page.click("[data-fix]");
    await page.click("[data-plan-run]");
    await page.waitForSelector("[data-start-check]", { timeout: 60000 });
    assert.match(await page.textContent("section[data-area=scan] [data-detail]"), /4 von 4/);
    // Kontrolle: alle jetzt auf 2400 Bd, nichts mehr zu beheben
    await page.click("[data-start-check]");
    await scanDone(page);
    await page.waitForSelector("[data-merge]");
    assert.ok(!(await page.$("[data-fix]")), "nach dem Beheben noch etwas zu beheben");

    // Baudraten prüfen: während der Prüfung sichtbar (läuft, Sekunden), Knopf gesperrt
    await page.click("[data-baud-btn]");
    await page.click("[data-baud-check]");
    assert.ok(await page.isDisabled("[data-baud-check]"));
    assert.match(await page.textContent("[data-baud-result]"), /Prüft alle Baudraten/);
    await page.waitForFunction(() => /Antworten auf/.test(document.querySelector("[data-baud-result]")?.textContent), null, { timeout: 60000 });
  },
};
