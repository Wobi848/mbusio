import { assert, simLine, tab, rowTexts } from "./helpers.mjs";

export default {
  name: "Zählerliste importieren, benennen, Namen erzeugen",
  setup: (api) => simLine(api, "SIMULATOR:mfh30"),
  async run({ page, api }) {
    await tab(page, "meters");
    await page.click("#list-open");
    const paste = [
      "Zählernummer\tBezeichnung\tWohnung",
      "71000001\tWhg 1 Wärme\tWhg 1",
      "72000001\t\tWhg 1",
      "73000001\t\tWhg 1",
      "99999999\tFehlt\tKeller",
    ].join("\n");
    await page.fill("#list-paste", paste);
    const map = await page.$$eval("[data-list-f]", (l) => Object.fromEntries(l.map((s) => [s.dataset.listF, s.selectedOptions[0].textContent])));
    assert.equal(map.id, "Zählernummer");
    assert.equal(map.location, "Wohnung");
    await page.click("#list-check");
    await page.waitForSelector("#list-result [data-cs-apply]");
    assert.match(await page.textContent("#list-result .cs-summary"), /4 neu/);
    await page.click("#list-result [data-cs-apply]");
    await page.waitForFunction(() => document.querySelectorAll("#meters-table tbody tr").length === 4);
    // auslesen, damit das Medium bekannt ist; dann Namen erzeugen für Zähler ohne Namen
    for (const m of await api("meters")) await api(`meters/${m.id}/read`, "POST").catch(() => {});
    await page.click("#quick-toggle");
    await page.check("#names-only");
    await page.click("#names-preview");
    await page.waitForSelector("#names-result [data-cs-apply]");
    await page.click("#names-result [data-cs-apply]");
    await page.waitForTimeout(500);
    // Benennen direkt in der Tabelle, Enter = nächster
    const loc = page.locator("input[data-q=location]");
    await loc.nth(3).fill("Keller · Technik");
    await loc.nth(3).press("Enter");
    await page.waitForTimeout(500);
    const meters = await api("meters");
    assert.equal(meters[3].location, "Keller · Technik");
    assert.deepEqual(meters.slice(0, 3).map((m) => m.name), ["Whg 1 Wärme", "Whg 1 Kaltwasser", "Whg 1 Warmwasser"]);
    await page.click("#quick-toggle");
    assert.equal((await rowTexts(page, "#meters-table tbody tr")).length, 4);
  },
};
