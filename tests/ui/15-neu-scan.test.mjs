import { assert, simLine } from "./helpers.mjs";

// Neue Oberfläche, Schritt 2: Bereich Scan (Suchzeile, Treffer zeilenweise, Zustand am Treffer,
// ein Hauptknopf: beheben → merken → alle in Liste)

const rows = (page) => page.locator("section[data-area=scan] [data-list] [role=option]");
/// erst laufen, dann fertig (der Knopf zeigt kurz nach dem Klick noch „starten“)
const done = async (page) => {
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent === "Abbrechen", null, { timeout: 10000 }).catch(() => {});
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent === "Primärscan starten", null, { timeout: 120000 });
};

export default {
  name: "Neue Oberfläche: Scan (Treffer, Klick während des Scans, merken, beheben)",
  async setup(api) {
    await simLine(api, "SIMULATOR:praxis");
  },
  async run({ page, base, api }) {
    let dialogs = 0;
    page.on("dialog", () => dialogs++);
    // ohne Adresse und ohne Zähler beginnt die Oberfläche im Scan
    await page.goto(base + "/neu/");
    await page.waitForFunction(() => location.hash === "#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "20");
    await page.press("[data-to]", "Tab");

    // Klick auf eine Zeile während des Scans bleibt (Zeilen werden nicht ersetzt)
    await page.keyboard.press("s");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=scan] [data-list] [role=option]").length >= 2);
    await rows(page).nth(1).click();
    await page.waitForSelector("section[data-area=scan] [role=option].active");
    await done(page);
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
    await page.goto(base + "/neu/#/scan");
    await page.waitForSelector("[data-to]");
    await page.fill("[data-to]", "8");
    await page.press("[data-to]", "Tab");
    await page.click("[data-baud-btn]");
    for (const b of ["300", "9600"]) await page.check(`[data-baud-menu] [data-baud][value="${b}"]`);
    await page.keyboard.press("Escape");
    await page.click("[data-start]");
    await done(page);
    await page.waitForSelector("[data-fix]");
    assert.match(await page.textContent("[data-fix]"), /4 beheben/);
    await page.click("[data-fix]");
    await page.click("[data-plan-run]");
    await page.waitForSelector("[data-start-check]", { timeout: 60000 });
    assert.match(await page.textContent("section[data-area=scan] [data-detail]"), /4 von 4/);
    // Kontrolle: alle jetzt auf 2400 Bd, nichts mehr zu beheben
    await page.click("[data-start-check]");
    await done(page);
    await page.waitForSelector("[data-merge]");
    assert.ok(!(await page.$("[data-fix]")), "nach dem Beheben noch etwas zu beheben");
    assert.equal(dialogs, 0, "Browser-Dialog benutzt");
  },
};
