import { assert, simLine } from "./helpers.mjs";

// Was man ändert, zeigt sich sofort: der Fokus bleibt auf dem benutzten Knopf oder Auswahlfeld, das
// darf die Anzeige nicht aufhalten. Beim Tippen in ein Feld bleibt der Text dagegen stehen.

const zl = "section[data-area=zaehler]";
const an = "section[data-area=anlage]";

export default {
  name: "Neu zeichnen (Auswahlfeld und Knopf mit Fokus, Tippen bleibt)",
  async setup(api) {
    await simLine(api);
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
    await api(`meters/${m.id}/read`, "POST");
    await api("gateways/1/lines", "POST", { name: "Zweite" });
  },
  async run({ page, base }) {
    // Verlauf: andere Reihe wählen, Fokus bleibt im Auswahlfeld, die Tabelle wechselt trotzdem
    await page.goto(`${base}/#/meters`);
    await page.locator(`${zl} [data-list] [role=option]`).first().click();
    await page.click(`${zl} [data-history]`);
    await page.waitForSelector(`${zl} [data-hist-key]`);
    await page.click(`${zl} [data-hist-view=table]`);
    const body = () => page.textContent(`${zl} .htable tbody`);
    const before = await body();
    const keys = await page.$$eval(`${zl} [data-hist-key] option`, (o) => o.map((x) => x.value));
    const other = keys.find((k) => k !== keys[0]);
    await page.focus(`${zl} [data-hist-key]`);
    await page.selectOption(`${zl} [data-hist-key]`, other);
    await page.waitForFunction(([z, b]) => document.querySelector(`${z} .htable tbody`)?.textContent !== b, [zl, before], { timeout: 3000 });
    assert.equal(await page.inputValue(`${zl} [data-hist-key]`), other, "Auswahl bleibt");
    assert.equal(await page.evaluate(() => document.activeElement.matches("[data-hist-key]")), true, "Fokus bleibt im Auswahlfeld");

    // Anlage: Linie aktiv machen, der Knopf „Wählen“ verschwindet sofort, Maus bleibt, wo sie ist
    await page.goto(`${base}/#/site`);
    const row = (name) => page.locator(`${an} [data-list] [role=option]`, { hasText: name });
    await row("Zweite").locator("[data-use]").click();
    await page.waitForFunction((a) => [...document.querySelectorAll(`${a} [data-list] [role=option]`)].some((r) => r.textContent.includes("Zweite") && !r.querySelector("[data-use]")), an, { timeout: 3000 });
    assert.equal(await row("Haus A").locator("[data-use]").count(), 1, "die frühere Linie wieder wählbar");

    await row("Haus A").locator("[data-use]").click();
    await page.waitForFunction((a) => [...document.querySelectorAll(`${a} [data-list] [role=option]`)].some((r) => r.textContent.includes("Haus A") && !r.querySelector("[data-use]")), an, { timeout: 3000 });

    // Tippen: eine Neuzeichnung darf nichts überschreiben, solange der Cursor im Feld steht
    await page.goto(`${base}/#/meters`);
    await page.locator(`${zl} [data-list] [role=option]`).first().click();
    await page.click(`${zl} [data-field=note]`);
    await page.keyboard.type("halb getippt");
    await page.waitForTimeout(6000); // Status kommt alle paar Sekunden
    assert.equal(await page.inputValue(`${zl} [data-field=note]`), "halb getippt");
    assert.equal(await page.evaluate(() => document.activeElement.matches("[data-field=note]")), true);
  },
};
