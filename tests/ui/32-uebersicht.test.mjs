import { assert, simLine } from "./helpers.mjs";

// Startseite: zeigt den Stand der Linie, erklärt den nächsten Schritt und führt zu den Bereichen

const card = (page, title) => page.locator("section[data-area=uebersicht] .card", { has: page.locator(`h2:text-is("${title}")`) });

export default {
  name: "Übersicht (Startseite, Karten, nächster Schritt)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters", "POST", { address: "99", name: "Fehlt" });
  },
  async run({ page, base, api }) {
    // ohne Adresse beginnt mbusio hier
    await page.goto(base + "/");
    await page.waitForFunction(() => location.hash === "#/overview");
    await page.waitForSelector("section[data-area=uebersicht]:not([hidden]) .card");
    assert.equal(await page.getAttribute(".tabs [data-area=uebersicht]", "aria-current"), "page");

    // nächster Schritt mit Erklärung; auf anderen Seiten bleibt nur der Knopf
    await page.waitForFunction(() => document.querySelector("#next:not([hidden])")?.textContent.includes("Zähler auslesen"));
    assert.match(await page.textContent("#next-why"), /2 Zähler stehen in der Liste/);
    assert.match(await card(page, "Zähler").textContent(), /2 ungelesen/);

    // Stand nach dem Auslesen: Zahlen wie im Filter „Gestört“
    for (const m of await api("meters")) await api(`meters/${m.id}/read`, "POST").catch(() => {});
    await page.reload();
    await page.waitForFunction(() => document.querySelector("section[data-area=uebersicht] .card")?.textContent.includes("1 gestört"));
    assert.match(await card(page, "Zähler").textContent(), /1 in Ordnung/);

    // Karten führen an die Stelle, an der man es ändert
    await card(page, "Zähler").locator("[data-show-faulty]").click();
    await page.waitForFunction(() => location.hash === "#/meters");
    await page.waitForSelector("[data-filter=err][aria-pressed=true], [data-filter=err].on, [data-filter=err][aria-current]");

    // Taste 0 kommt zurück, die anderen Tasten behalten ihre Bereiche
    await page.keyboard.press("0");
    await page.waitForFunction(() => location.hash === "#/overview");
    await page.keyboard.press("3");
    await page.waitForFunction(() => location.hash === "#/meters");
    assert.ok(await page.isHidden("#next-why"));
    await page.keyboard.press("0");
    await page.waitForFunction(() => location.hash === "#/overview");
    await card(page, "Weitergabe").locator("a").click();
    await page.waitForFunction(() => location.hash === "#/transfer");

    await page.keyboard.press("0");
    await page.locator("section[data-area=uebersicht] [data-open-settings]").click();
    await page.waitForSelector("#settings-menu:not([hidden])");
  },
};
