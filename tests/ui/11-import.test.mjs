import { assert, simLine } from "./helpers.mjs";

// unlesbare Dateien beim Import sagen, was erwartet war

export default {
  name: "Import (kaputte Projektdatei, Zählerliste ohne ID)",
  setup: (api) => simLine(api),
  async run({ page, base, api }) {
    await page.goto(base + "/#/anlage");
    await page.setInputFiles("[data-import-file]", { name: "kaputt.json", mimeType: "application/json", buffer: Buffer.from("{ kein json") });
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("keine Projektdatei"));
    assert.equal((await api("projects")).projects.length, 1);

    // Zählerliste ohne erkennbare ID-Spalte: Hinweis statt Fehlversuch
    await page.goto(base + "/#/zaehler");
    await page.click("section[data-area=zaehler] [data-more]");
    await page.click("section[data-area=zaehler] [data-panel=import]");
    await page.setInputFiles("section[data-area=zaehler] [data-file]", { name: "liste.csv", mimeType: "text/csv", buffer: Buffer.from("Farbe;Grösse\nrot;3\n") });
    await page.click("section[data-area=zaehler] [data-list-check]");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("ID zuordnen"));
    assert.equal((await api("meters")).length, 0);
  },
};
