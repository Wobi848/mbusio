import { assert, simLine } from "./helpers.mjs";

// Speicherschutz im Menü Einstellungen: Vorgaben, ändern speichert gleich, falsche Stufen werden
// abgelehnt und zurückgesetzt

export default {
  name: "Speicherschutz (Einstellungen, Vorgaben, Stufen prüfen)",
  async setup(api) {
    await simLine(api, "SIMULATOR");
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/anlage");
    await page.click("#settings");
    const f = (n) => `#storage [name=${n}]`;
    await page.waitForFunction(() => document.querySelector("#storage [name=first_days]")?.value === "365");
    assert.ok(await page.isChecked(f("protect")));
    assert.equal(await page.inputValue(f("second_days")), "90");

    await page.fill(f("first_days"), "400");
    await page.press(f("first_days"), "Tab");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("gespeichert"));
    assert.equal((await api("storage")).first_days, 400);

    // zweite Stufe unter 30 Tagen: abgelehnt, Feld wieder wie gespeichert
    await page.fill(f("second_days"), "10");
    await page.press(f("second_days"), "Tab");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("30 Tage"));
    await page.waitForFunction(() => document.querySelector("#storage [name=second_days]").value === "90");
    assert.equal((await api("storage")).second_days, 90);
  },
};
