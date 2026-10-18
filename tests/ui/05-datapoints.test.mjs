import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assert, simLine, tab, visibleSections } from "./helpers.mjs";

export default {
  name: "Weitergabe: Werte wählen, Vorlage, bearbeiten, CSV, Link aus Zählerliste und Auslesung",
  async setup(api) {
    await simLine(api);
    for (const a of ["1", "7"]) await api("meters", "POST", { address: a, name: "Z" + a });
    for (const id of [1, 2]) await api(`meters/${id}/read`, "POST");
  },
  async run({ page }) {
    // aus der Zählerliste in den Reiter Weitergabe, Auswahl des Zählers offen
    await tab(page, "meters");
    await page.locator("[data-meter-transfer]").first().click();
    await page.waitForSelector(".tr-pick .dp-pick-row");
    assert.ok((await visibleSections(page)).includes("sec-transfer"));
    // Hauptwert vorgeschlagen
    assert.deepEqual(await page.$$eval(".tr-pick .dp-pick input:checked", (l) => l.map((x) => x.value)), ["instantaneous|0|0|0|energy|kWh"]);
    await page.check("[data-pick-tpl]");
    await page.click("[data-pick-save]");
    // Vorlage gilt auch für den zweiten Wärmezähler
    await page.waitForFunction(() => document.querySelectorAll("[data-dp-edit]").length === 2);
    // bearbeiten: Faktor mit Komma
    await page.locator("[data-dp-edit]").first().click();
    await page.fill("#dp-form [name=unit]", "MWh");
    await page.fill("#dp-form [name=factor]", "0,001");
    await page.click("#dp-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#dp-table")?.textContent.includes("MWh"));
    // CSV exportieren, ändern, mit Vorschau wieder einlesen
    const [dl] = await Promise.all([page.waitForEvent("download"), page.click("#tr-export")]);
    const lines = fs.readFileSync(await dl.path(), "utf8").replace(/^\uFEFF/, "").split(/\r?\n/);
    assert.match(lines[0], /^Nr;Linie;Zähler.*;MQTT;Topic$/);
    const c = lines[1].split(";");
    c[5] = "Wärme Whg 1";
    lines[1] = c.join(";");
    const file = path.join(os.tmpdir(), "mbusio-dp-test.csv");
    fs.writeFileSync(file, lines.join("\r\n"));
    await page.setInputFiles("#tr-import", file);
    await page.waitForSelector("[data-cs-apply]");
    assert.match(await page.textContent("#tr-preview .cs-summary"), /1 Änderungen/);
    await page.click("[data-cs-apply]");
    await page.waitForFunction(() => document.querySelector("#dp-table")?.textContent.includes("Wärme Whg 1"));
    // Massenbearbeitung: alle auf 1× täglich
    await page.check("#tr-all");
    await page.selectOption("#dp-bulk-send", "daily");
    await page.waitForFunction(() => document.querySelectorAll("#dp-table tbody tr:not(.tr-meter)").length === 2 &&
      [...document.querySelectorAll("#dp-table tbody tr:not(.tr-meter)")].every((r) => r.textContent.includes("1× täglich")));
    // Auslesung in Werkzeuge: Link zur Weitergabe
    await tab(page, "tools");
    await page.fill("#read-addr", "7");
    await page.click("#read");
    await page.waitForSelector("[data-open-transfer]");
    await page.click("[data-open-transfer]");
    await page.waitForSelector(".tr-pick .dp-pick-row");
  },
};
