import { assert, simLine } from "./helpers.mjs";

// Anlage → Projekt → Abrechnung exportieren: Vorgaben, Zeitraum, weitere Werte, Dezimalzeichen,
// Zeitreihe, Vorschau, CSV mit denselben Einstellungen, je Projekt gespeichert

const sec = "section[data-area=anlage]";

export default {
  name: "Abrechnung exportieren (Zeitraum, Werte, Vorschau, CSV, gespeichert)",
  async setup(api) {
    await simLine(api, "SIMULATOR:praxis");
    for (const [a, n] of [["1", "Whg 1 Wärme"], ["3", "Whg 1 Wasser"]]) {
      const m = await api("meters", "POST", { address: a, name: n });
      await api(`meters/${m.id}/read`, "POST");
    }
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/anlage");
    await page.locator(`${sec} [data-list] [role=option]`).first().click();
    await page.click("[data-billing]");
    await page.waitForSelector("[data-b-period]");
    // Vorgaben: Vormonat, Abrechnung, Hauptwert
    assert.equal(await page.inputValue("[data-b-period]"), "prev_month");
    assert.ok(await page.isChecked("[data-b-main]"));

    // Zeitraum bis heute: beide Zähler mit Hauptwert (Energie, Volumen)
    const today = new Date().toISOString().slice(0, 10);
    await page.selectOption("[data-b-period]", "custom");
    await page.fill("[data-b-from]", today.slice(0, 8) + "01");
    await page.press("[data-b-from]", "Tab");
    await page.fill("[data-b-to]", today);
    await page.press("[data-b-to]", "Tab");
    await page.waitForFunction(() => document.querySelectorAll(".btable tbody tr").length === 2);
    const rows = await page.$$eval(".btable tbody tr", (l) => l.map((r) => r.textContent));
    assert.match(rows.join("|"), /Whg 1 Wärme.*Energie.*kWh.*\|Whg 1 Wasser.*Volumen/);

    // weiterer Wert: Leistung beim Wärmezähler
    await page.click("details.more summary");
    await page.check('[data-b-key*="|power|"]');
    await page.waitForFunction(() => document.querySelectorAll(".btable tbody tr").length === 3);

    // CSV: dieselben Zeilen, Dezimalkomma, BOM für Excel
    const csv = async () =>
      page.evaluate(async () => {
        const b = new Uint8Array(await (await fetch(document.querySelector("[data-b-download]").href)).arrayBuffer());
        return [[...b.slice(0, 3)], new TextDecoder().decode(b)];
      });
    let [bom, text] = await csv();
    assert.deepEqual(bom, [0xef, 0xbb, 0xbf]);
    const lines = text.trim().split("\r\n");
    assert.equal(lines.length, 4, text);
    assert.match(lines[0], /^Projekt;Linie;Name;Ort;ID;PA;Medium;Wert;Einheit;Stand Anfang;Quelle Anfang;Stand Ende;Quelle Ende;Verbrauch;Hinweis$/);
    assert.match(text, /Whg 1 Wasser;.*;Volumen;m³;\d+,\d+;/);

    // Dezimalpunkt
    await page.selectOption("[data-b-decimal]", ".");
    await page.waitForFunction(() => /\d\.\d/.test(document.querySelector(".btable tbody")?.textContent));
    [, text] = await csv();
    assert.match(text, /Volumen;m³;\d+\.\d+;/);

    // Zeitreihe je Tag
    await page.selectOption("[data-b-form]", "series");
    await page.waitForSelector("[data-b-step]");
    await page.selectOption("[data-b-step]", "day");
    await page.waitForFunction(() => /^Datum/.test(document.querySelector(".btable thead")?.textContent));

    // je Projekt gespeichert
    const s = (await api("projects/1/billing")).settings;
    assert.deepEqual([s.period, s.form, s.step, s.decimal, s.keys.length], ["custom", "series", "day", ".", 1]);

    // schliessen: wieder die Details des Projekts
    await page.click("[data-billing-close]");
    await page.waitForSelector("[data-billing]");
  },
};
