import { assert, simLine } from "./helpers.mjs";

// Bereich Zähler (Liste links, Details rechts, Auswahl, Tastatur)

const rows = (page) => page.locator("[data-list] [role=option]");

export default {
  name: "Zähler (Liste, Details, Bearbeiten, Auswahl, Auslesen, Datenpunkt)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    for (const a of ["1", "2", "3", "4"]) await api("meters", "POST", { address: a, name: a === "4" ? "" : "Whg " + a });
    await api("meters/1/read", "POST");
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/zaehler");
    await page.waitForFunction(() => document.querySelectorAll("[data-list] [role=option]").length === 4);

    // Zustand am Element: gelesen mit Wert, sonst „noch nicht ausgelesen“; ohne Namen sichtbar
    assert.match(await rows(page).nth(0).textContent(), /kWh/);
    assert.match(await rows(page).nth(1).textContent(), /noch nicht ausgelesen/);
    assert.match(await rows(page).nth(3).textContent(), /ohne Namen/);

    // Suche und Filter mit Anzahl
    await page.keyboard.press("/");
    await page.keyboard.type("whg 2");
    await page.waitForFunction(() => document.querySelectorAll("[data-list] [role=option]").length === 1);
    await page.fill("[data-search]", "");
    await page.click("[data-filter=noname]");
    assert.equal(await rows(page).count(), 1);
    assert.match(await page.textContent("[data-filter=noname]"), /1/);
    await page.click("[data-filter=all]");

    // Tastatur: Liste fokussieren, ↓ wählt, Details folgen
    await page.focus("[data-list]");
    await page.keyboard.press("ArrowDown");
    await page.waitForFunction(() => document.querySelector("[data-field=name]")?.value === "Whg 2");
    // Feld: Escape setzt zurück, Verlassen speichert
    await page.fill("[data-field=name]", "falsch");
    await page.press("[data-field=name]", "Escape");
    assert.equal(await page.inputValue("[data-field=name]"), "Whg 2");
    await page.fill("[data-field=location]", "Keller");
    await page.press("[data-field=location]", "Enter");
    await page.waitForFunction(() => document.querySelector("[data-list] .row.active")?.textContent.includes("Keller"));
    assert.equal((await api("meters")).find((m) => m.address === "2").location, "Keller");

    // Auswahl mit Leertaste, Leiste erscheint, Escape hebt auf
    await page.focus("[data-list]");
    await page.keyboard.press(" ");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press(" ");
    await page.waitForSelector("[data-selbar]:not([hidden])");
    assert.match(await page.textContent("[data-selbar]"), /2 ausgewählt/);
    await page.keyboard.press("Escape");
    await page.waitForSelector("[data-selbar][hidden]", { state: "attached" });

    // R liest den gewählten Zähler aus
    await page.focus("[data-list]");
    await page.keyboard.press("r");
    await page.waitForFunction(() => document.querySelector("[data-list] .row.active")?.textContent.includes("kWh"), null, { timeout: 20000 });

    // Wert als Datenpunkt wählen
    await page.waitForSelector("[data-detail] [data-dp]");
    await page.locator("[data-detail] [data-dp]").first().check();
    await page.waitForFunction(() => document.querySelector(".tabs #n-dp")?.textContent === "1", null, { timeout: 20000 });

    // Verlauf: zweimal gelesen → Diagramm im Detailbereich (Taste V)
    await page.focus("[data-list]");
    await page.keyboard.press("r");
    await page.waitForTimeout(1500);
    await page.keyboard.press("v");
    await page.waitForSelector("section[data-area=zaehler] [data-chart] svg", { timeout: 20000 });

    // Zähler getauscht: Feld statt prompt, Rückfrage am Ort
    await page.click("section[data-area=zaehler] [data-exchange]");
    await page.fill("section[data-area=zaehler] [data-exchange-addr]", "9");
    await page.click("section[data-area=zaehler] [data-exchange-go]");
    await page.click(".ask button.danger");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("getauscht"), null, { timeout: 20000 });

    // Namen nach Muster mit Vorschau
    await page.click("section[data-area=zaehler] [data-more]");
    await page.click("section[data-area=zaehler] [data-panel=names]");
    await page.fill("section[data-area=zaehler] [data-pattern]", "Zähler {pa}");
    await page.click("section[data-area=zaehler] [data-names-preview]");
    await page.click("section[data-area=zaehler] [data-cs-apply]");
    await page.waitForFunction(() => document.querySelector("[data-list]")?.textContent.includes("Zähler 1"));

    // Liste vom Kunden einfügen, Spalten werden erkannt, Vorschau, übernehmen
    await page.click("section[data-area=zaehler] [data-more]");
    await page.click("section[data-area=zaehler] [data-panel=import]");
    const id = (await api("meters")).find((m) => m.meter_id)?.meter_id;
    await page.fill("section[data-area=zaehler] [data-paste]", `ID\tName\tOrt\n${id}\tVom Kunden\tKeller`);
    await page.click("section[data-area=zaehler] [data-list-check]");
    await page.click("section[data-area=zaehler] [data-cs-apply]");
    await page.waitForFunction(() => document.querySelector("[data-list]")?.textContent.includes("Vom Kunden"));

    // Hinzufügen im Detailbereich
    await page.click("[data-add]");
    await page.fill("[data-new=address]", "7");
    await page.fill("[data-new=name]", "Neu");
    await page.press("[data-new=name]", "Enter");
    await page.waitForFunction(() => document.querySelectorAll("[data-list] [role=option]").length === 5);
    assert.equal(await page.inputValue("[data-field=name]"), "Neu");

    // Löschen nur nach Rückfrage am Ort; Abbrechen lässt ihn stehen
    await page.click("[data-detail] [data-del]");
    await page.click(".ask button.quiet");
    assert.equal(await rows(page).count(), 5);
    await page.click("[data-detail] [data-del]");
    await page.click(".ask button.danger");
    await page.waitForFunction(() => document.querySelectorAll("[data-list] [role=option]").length === 4);
  },
};
