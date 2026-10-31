import { assert } from "./helpers.mjs";

// Jede geladene Sprache hat dieselben Texte wie Englisch, Funktionen mit denselben Argumenten; eine
// unbekannte Sprache wird Englisch

const AREAS = ["anlage", "scan", "zaehler", "weitergabe", "werkzeuge", "simulator"];

export default {
  name: "Texte (alle Sprachen vollständig, unbekannte Sprache wird Englisch)",
  async run({ page, base }) {
    // die Bereiche laden ihre Texte erst beim Öffnen; Sprachdateien nur für ihre Sprache
    const open = async (query = "") => {
      for (const area of AREAS) {
        await page.goto(`${base}/${query}#/${area}`);
        await page.waitForSelector(`section[data-area=${area}]`);
      }
    };
    await open();
    for (const code of await page.evaluate(() => Object.keys(LANG_PREVIEW))) {
      await open(`?lang=${code}`);
      assert.equal(await page.evaluate(() => document.documentElement.lang), code, "Vorschau über ?lang=");
    }
    const wrong = await page.evaluate(() => {
      const out = [];
      for (const [code, texts] of Object.entries(UI)) {
        for (const key of Object.keys(UI.en)) {
          if (texts[key] === undefined) out.push(`${code}: fehlt ${key}`);
          else if (typeof UI.en[key] === "function" && (typeof texts[key] !== "function" || texts[key].length !== UI.en[key].length)) out.push(`${code}: Argumente ${key}`);
        }
        for (const key of Object.keys(texts)) if (UI.en[key] === undefined) out.push(`en: fehlt ${key} (${code})`);
      }
      return [...new Set(out)];
    });
    assert.deepEqual(wrong, []);

    // gespeichert oder vom Browser: unbekannt wird Englisch, nie Deutsch und nie ein leeres Menü
    await page.addInitScript(() => localStorage.setItem("lang", "sv"));
    await page.goto(`${base}/#/zaehler`);
    await page.reload();
    await page.waitForSelector("#lang option", { state: "attached" });
    assert.equal(await page.evaluate(() => lang), "en");
    await page.click("#settings");
    assert.equal(await page.inputValue("#lang"), "en");
    assert.deepEqual(await page.$$eval("#lang option", (o) => o.map((x) => x.value)), await page.evaluate(() => Object.keys(LANGUAGES)));
  },
};
