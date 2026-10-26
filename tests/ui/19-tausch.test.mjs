import { assert, simLine } from "./helpers.mjs";

// Zählertausch erkannt (Konzept Export 3.6): unter derselben Adresse antwortet ein Zähler mit
// anderer ID → Hinweis in der Liste und in den Details, bestätigen; die Abrechnung zählt den Hinweis

const sec = "section[data-area=zaehler]";

export default {
  name: "Zählertausch erkannt (Hinweis, bestätigen, Abrechnung)",
  async setup(api) {
    const { key } = await api("sim", "POST", { name: "Tausch", from: "ok10" });
    await simLine(api, `SIMULATOR:${key}`);
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
    // derselbe Platz, neuer Zähler
    const sim = (await api("sim")).scenarios.find((s) => s.key === key);
    sim.meters[0].id = "87654321";
    await api(`sim/${key}`, "PUT", { name: sim.name, meters: sim.meters });
    await api(`meters/${m.id}/read`, "POST");
  },
  async run({ page, base, api }) {
    // Abrechnung bis heute: Hinweis gezählt
    const today = new Date().toISOString().slice(0, 10);
    await api("projects/1/billing", "PUT", { period: "custom", from: today.slice(0, 8) + "01", to: today });
    assert.equal((await api("projects/1/billing")).preview.summary.swap_open, 1);

    await page.goto(base + "/#/zaehler");
    const row = page.locator(`${sec} [data-list] [role=option]`).first();
    await row.waitFor();
    assert.match(await row.textContent(), /Tausch erkannt/);
    await row.click();
    const banner = page.locator(`${sec} .chip.wide.swap`);
    assert.match(await banner.textContent(), /87654321/);
    await page.click(`${sec} [data-swap-confirm]`);
    await banner.waitFor({ state: "detached" });
    assert.doesNotMatch(await row.textContent(), /Tausch erkannt/);

    // bestätigt: Abrechnung nennt den Tausch ohne „nicht bestätigt“
    const s = (await api("projects/1/billing")).preview.summary;
    assert.deepEqual([s.swap, s.swap_open], [1, undefined]);
    await page.goto(base + "/#/anlage");
    await page.locator("section[data-area=anlage] [data-list] [role=option]").first().click();
    await page.click("[data-billing]");
    await page.waitForFunction(() => /1 × Zählertausch/.test(document.querySelector("section[data-area=anlage] .chip.warn.wide")?.textContent));
  },
};
