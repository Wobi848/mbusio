import { assert, settings, simLine, tab, visibleSections } from "./helpers.mjs";

export default {
  name: "Reiter, Hilfe, Sprache ohne fehlende Texte",
  setup: (api) => simLine(api),
  async run({ page }) {
    const expect = { project: "sec-project", scan: "sec-scan", meters: "sec-meters", transfer: "sec-transfer", tools: "sec-read", sim: "sec-sim" };
    for (const [name, sec] of Object.entries(expect)) {
      await tab(page, name);
      assert.ok((await visibleSections(page)).includes(sec), `Reiter ${name} zeigt ${sec}`);
    }
    // Reiter bleibt nach dem Neuladen
    await tab(page, "meters");
    await page.reload();
    await page.waitForTimeout(500);
    assert.ok((await visibleSections(page)).includes("sec-meters"));
    // Hilfe ein/aus
    await tab(page, "scan");
    assert.ok(await page.isHidden("[data-i18n=scan_primary_hint]"));
    await settings(page);
    await page.click("#help-toggle");
    assert.ok(await page.isVisible("[data-i18n=scan_primary_hint]"));
    await settings(page);
    await page.click("#help-toggle");
    // in beiden Sprachen kein Text, der nur ein Schlüssel ist (fehlende Übersetzung)
    for (const l of ["en", "de"]) {
      await settings(page);
      await page.click(`[data-lang=${l}]`);
      await page.waitForTimeout(400);
      for (const name of Object.keys(expect)) {
        await tab(page, name);
        const raw = await page.evaluate(() =>
          [...document.querySelectorAll("main *:not(script)")]
            .filter((el) => el.offsetParent && el.children.length === 0)
            .map((el) => el.textContent.trim())
            .filter((s) => /^[a-z]+(_[a-z0-9]+)+$/.test(s))
        );
        assert.deepEqual(raw, [], `fehlende Texte (${l}, ${name})`);
      }
    }
  },
};
