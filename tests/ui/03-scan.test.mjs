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

    // Kollisionen = doppelte PA: Hauptknopf „auflösen“, merken geht daneben trotzdem
    assert.match(await page.textContent("[data-main] [data-secondary]"), /2 doppelte PA auflösen/);
    await page.click("[data-main] [data-merge]");
    await page.waitForFunction(() => !document.querySelector("[data-main] [data-merge]"));
    assert.equal((await api("meters")).length, 10);
    assert.match(await rows(page).nth(1).textContent(), /in Liste/);

    // auflösen: Sekundärscan findet die Zähler einzeln, dann mit einem Klick umadressieren
    // (kurze Antwortzeit: der Sekundärscan dauert sonst fast 3 Minuten)
    await simLine(api, "SIMULATOR:praxis", 2400, 40);
    await page.waitForFunction(() => document.querySelector("#bus .dot.on"));
    await page.click("[data-main] [data-secondary]");
    await scanDone(page, 300000);
    await page.waitForSelector("[data-main] [data-fix]");
    assert.match(await page.textContent("[data-main] [data-fix]"), /3 beheben/);
    await page.click("[data-main] [data-fix]");
    // erst wenn die freien Adressen gefunden sind, lässt sich ausführen
    assert.ok(await page.isDisabled("[data-plan-run]"));
    await page.waitForSelector("[data-plan-run]:not([disabled])", { timeout: 120000 });
    await page.click("[data-plan-run]");
    await page.waitForSelector("[data-start-check]", { timeout: 120000 });
    assert.match(await page.textContent("section[data-area=scan] [data-detail]"), /3 von 3/);

    // Beheben: Zähler mit anderer Baudrate auf die Bus-Baudrate umstellen
    await simLine(api, "SIMULATOR:bauds", 2400, 40);
    await page.goto(base + "/#/scan");
    // die Art merkt sich der Scan je Linie: zuletzt war es der Sekundärscan
    await page.click("[data-kind=primary]");
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
