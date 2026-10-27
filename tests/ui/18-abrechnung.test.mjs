import { assert, simLine } from "./helpers.mjs";

// Anlage → Projekt → Abrechnung „Öffnen“ führt in die Weitergabe → Abrechnung: Vorgaben, Zeitraum, weitere Werte, Dezimalzeichen,
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
    // Vorgabe 1 Vorjahr: eigene Spalte in der Vorschau; ohne Empfänger kein Senden
    await page.waitForFunction(() => /Verbrauch \d\d\/\d{4}/.test(document.querySelector(".btable thead")?.textContent));
    assert.ok(await page.isDisabled("[data-b-mail-now]"), "ohne Empfänger gesperrt");

    // Vorquartal: Bezeichnung in Spalte und Betreff
    await page.selectOption("[data-b-period]", "prev_quarter");
    await page.waitForFunction(() => /Verbrauch Q[1-4]\/\d{4}/.test(document.querySelector(".btable thead")?.textContent));
    assert.match(await page.textContent(".bset"), /→ Abrechnung .+ Q[1-4]\/\d{4}/);
    await page.fill("[data-b-subject]", "{projekt}: {zeitraum}");
    await page.press("[data-b-subject]", "Tab");
    await page.waitForFunction(() => /→ \S.*: Q[1-4]\/\d{4}/.test(document.querySelector(".bset").textContent));
    // keine Vorjahre: Spalte weg
    await page.selectOption("[data-b-compare]", "0");
    await page.waitForFunction(() => !/Verbrauch Q/.test(document.querySelector(".btable thead")?.textContent));
    await page.selectOption("[data-b-period]", "prev_month");
    await page.waitForFunction(() => document.querySelector("[data-b-period]").value === "prev_month" && document.querySelector("[data-b-compare]")?.value === "0");

    // automatisch: Vormonat ist vorbei und schon gelesen → Datei liegt bald in der Liste
    await page.check("[data-b-auto]");
    for (let i = 0; !(await api("projects/1/billing")).files.length; i++) {
      assert.ok(i < 30, "keine Datei nach 30 s");
      await page.waitForTimeout(1000);
    }
    await page.click("[data-b-close]");
    await page.click("[data-open-billing]");
    const file = await page.getAttribute("[data-b-file]", "href");
    assert.match(file, /abrechnung-\d{4}-\d\d-01-\d{4}-\d\d-\d\d\.csv/);
    assert.match(await page.evaluate(async (h) => (await fetch(h)).text(), file), /^Projekt;Linie;Name/);
    assert.equal(await page.evaluate(async () => (await fetch("/api/projects/1/billing/files/..%2Fmbusio.db")).status), 404);
    await page.uncheck("[data-b-auto]");
    for (let i = 0; (await api("projects/1/billing")).settings.auto; i++) {
      assert.ok(i < 10);
      await page.waitForTimeout(200);
    }

    // Zeitraum bis heute: beide Zähler mit Hauptwert (Energie, Volumen)
    const today = new Date().toISOString().slice(0, 10);
    await page.selectOption("[data-b-period]", "custom");
    await page.waitForSelector("[data-b-auto]:disabled");
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
    assert.match(lines[0], /^Projekt;Linie;Name;Ort;ID;PA;Medium;Wert;Einheit;Stand Anfang;Quelle Anfang;Stand Ende;Quelle Ende;Verbrauch;Hinweis$/, "Vorjahre auf 0 gestellt");
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

    // Linien wählen: nur bei mehreren Linien; die letzte bleibt gewählt
    assert.equal(await page.locator("[data-b-line]").count(), 0, "eine Linie: keine Auswahl");
    const gw = await api("projects/1/gateways", "POST", { name: "Haus B" });
    const line = await api(`gateways/${gw.id}/lines`, "POST", { name: "Linie 1", connection: "SIMULATOR", baud: 2400 });
    // Zähler kommen auf die aktive Linie: kurz umschalten
    await api(`lines/${line.id}/activate`, "POST");
    await api("meters", "POST", { address: "5", name: "Whg 5" });
    await api("lines/1/activate", "POST");
    await page.click("[data-b-close]");
    await page.click("[data-open-billing]");
    await page.waitForSelector("[data-b-form]");
    await page.selectOption("[data-b-form]", "billing");
    await page.waitForFunction(() => !document.querySelector("[data-b-step]"));
    await page.click("text=Linien: alle");
    await page.uncheck("[data-b-line] >> nth=0");
    for (let i = 0; !(await api("projects/1/billing")).settings.lines.length; i++) {
      assert.ok(i < 10);
      await page.waitForTimeout(200);
    }
    assert.deepEqual((await api("projects/1/billing")).settings.lines, [line.id]);
    await page.waitForFunction(() => /Whg 5/.test(document.querySelector(".btable tbody")?.textContent) && document.querySelectorAll(".btable tbody tr").length === 1);
    await page.click(`[data-b-line="${line.id}"]`);
    assert.ok(await page.isChecked(`[data-b-line="${line.id}"]`), "mindestens eine Linie");

    // schliessen: wieder die Details des Projekts
    await page.click("[data-b-close]");
    await page.waitForSelector("section[data-area=weitergabe] [data-list]");
  },
};
