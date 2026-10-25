import { assert, simLine } from "./helpers.mjs";

// Neue Oberfläche, Simulator: Vorlage ansehen, kopieren (Feld statt prompt), Zähler ändern,
// speichern, mit der aktiven Linie verwenden, löschen mit Rückfrage am Ort

const sec = "section[data-area=simulator]";

export default {
  name: "Neue Oberfläche: Simulator (Vorlage, kopieren, ändern, speichern, verwenden, löschen)",
  async setup(api) {
    await simLine(api, "SIMULATOR");
  },
  async run({ page, base, api }) {
    let dialogs = 0;
    page.on("dialog", () => dialogs++);
    await page.goto(base + "/neu/#/simulator");
    await page.waitForSelector(`${sec} [data-list] [role=option]`);
    // Vorlage: nur ansehen
    assert.match(await page.textContent(`${sec} [data-chips]`), /Vorlage/);
    await page.locator(`${sec} [data-list] [role=option]`).nth(2).click();
    assert.equal(await page.locator(`${sec} [data-detail] [data-f]`).count(), 0, "Vorlage darf nicht änderbar sein");

    // Kopieren mit Feld an Ort und Stelle
    await page.click(`${sec} [data-copy-btn]`);
    await page.fill(`${sec} [data-copy-menu] [data-new-name]`, "Meine Praxis");
    await page.press(`${sec} [data-copy-menu] [data-new-name]`, "Enter");
    await page.waitForFunction(() => document.querySelector("section[data-area=simulator] [data-sim] option:checked")?.textContent === "Meine Praxis");

    // Zähler ändern → ungespeichert → speichern
    await page.locator(`${sec} [data-list] [role=option]`).nth(0).click();
    await page.selectOption(`${sec} [data-detail] [data-f=fault]`, "checksum");
    await page.waitForSelector(`${sec} [data-save]`);
    await page.click(`${sec} [data-save]`);
    await page.waitForFunction(() => !document.querySelector("section[data-area=simulator] [data-save]"));
    const mine = (await api("sim")).scenarios.find((s) => s.name === "Meine Praxis");
    assert.equal(mine.meters[0].fault, "checksum");

    // mit der aktiven Linie verwenden
    await page.click(`${sec} [data-use]`);
    await page.waitForFunction(() => document.querySelector("section[data-area=simulator] [data-chips]")?.textContent.includes("benutzt"));
    assert.equal((await api("projects")).active.line.connection, `SIMULATOR:${mine.key}`);

    // benutzt die Linie die Simulation, ist Löschen gesperrt; Linie zurück auf die Vorlage
    assert.ok(await page.isDisabled(`${sec} [data-del]`));
    await page.selectOption(`${sec} [data-sim]`, "praxis");
    await page.click(`${sec} [data-use]`);
    await page.waitForFunction(async () => document.querySelector("section[data-area=simulator] [data-chips]")?.textContent.includes("benutzt"));
    await page.selectOption(`${sec} [data-sim]`, mine.key);
    // löschen nur nach Rückfrage am Ort
    await page.click(`${sec} [data-del]`);
    await page.click(".ask button.danger");
    await page.waitForFunction(() => document.querySelector("section[data-area=simulator] [data-sim] option:checked")?.textContent !== "Meine Praxis");
    assert.ok(!(await api("sim")).scenarios.some((s) => s.name === "Meine Praxis"));
    assert.equal(dialogs, 0, "Browser-Dialog benutzt");
  },
};
