import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assert, simLine, tab, rowTexts } from "./helpers.mjs";

export default {
  name: "Datenpunkte: Werte wählen, Vorlage, bearbeiten, CSV",
  async setup(api) {
    await simLine(api);
    for (const a of ["1", "7"]) await api("meters", "POST", { address: a, name: "Z" + a });
    for (const id of [1, 2]) await api(`meters/${id}/read`, "POST");
  },
  async run({ page }) {
    await tab(page, "meters");
    await page.locator("[data-meter-values]").first().click();
    await page.waitForSelector("#dp-picker .dp-pick-row");
    // Hauptwert vorgeschlagen
    assert.deepEqual(await page.$$eval("#dp-picker .dp-pick input:checked", (l) => l.map((x) => x.value)), ["instantaneous|0|0|0|energy|kWh"]);
    await page.check("#pick-tpl");
    await page.click("[data-pick-save]");
    // Vorlage gilt auch für den zweiten Wärmezähler
    await page.waitForFunction(() => document.querySelectorAll("#dp-table tbody tr").length === 2);
    // bearbeiten: Faktor mit Komma
    await page.locator("[data-dp-edit]").first().click();
    await page.fill("#dpf-unit", "MWh");
    await page.fill("#dpf-factor", "0,001");
    await page.click("#dp-form [type=submit]");
    await page.waitForFunction(() => document.querySelector("#dp-table tbody tr")?.textContent.includes("MWh"));
    // CSV exportieren, ändern, mit Vorschau wieder einlesen
    const [dl] = await Promise.all([page.waitForEvent("download"), page.click("#dp-export")]);
    const lines = fs.readFileSync(await dl.path(), "utf8").replace(/^﻿/, "").split(/\r?\n/);
    assert.match(lines[0], /^Nr;Linie;Zähler/);
    const c = lines[1].split(";");
    c[5] = "Wärme Whg 1";
    lines[1] = c.join(";");
    const file = path.join(os.tmpdir(), "mbusio-dp-test.csv");
    fs.writeFileSync(file, lines.join("\r\n"));
    await page.setInputFiles("#dp-import", file);
    await page.waitForSelector("[data-imp-apply]");
    assert.match(await page.textContent("#dp-import-preview h3"), /1 Änderungen/);
    await page.click("[data-imp-apply]");
    await page.waitForFunction(() => document.querySelector("#dp-table tbody tr")?.textContent.includes("Wärme Whg 1"));
    // Massenbearbeitung: alle auf 1× täglich
    await page.check("#dp-all");
    await page.selectOption("#dp-bulk-send", "daily");
    await page.waitForFunction(() => [...document.querySelectorAll("#dp-table tbody tr")].every((r) => r.textContent.includes("1× täglich")));
    assert.equal((await rowTexts(page, "#dp-table tbody tr")).length, 2);
  },
};
