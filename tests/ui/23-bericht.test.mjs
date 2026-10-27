import { assert } from "./helpers.mjs";

// Bericht per E-Mail (Konzept E-Mail 6): Einstellungen unter Weitergabe → E-Mail → Bericht mit
// Vorgaben, speichern ändert nur den Bericht. Wann er rausgeht, prüft der Rust-Test im Zeitplan.

const wg = "section[data-area=weitergabe]";

export default {
  name: "Bericht (Einstellungen mit Vorgaben, speichern)",
  async run({ page, base, api }) {
    await api("mail", "PUT", { config: { ...(await api("mail")).config, host: "smtp.example.org", warn: { to: "warn@example.org" } } });
    await page.goto(base + "/#/weitergabe/email");
    await page.click(`${wg} [data-report-section] summary`);
    const r = (n) => `${wg} [data-report-form] [name=${n}]`;
    assert.equal(await page.inputValue(r("frequency")), "daily");
    assert.equal(await page.inputValue(r("time")), "07:00");
    assert.equal(await page.inputValue(r("weekday")), "0");
    assert.match(await page.getAttribute(r("subject"), "placeholder"), /Bericht \{datum\}/);
    for (const k of ["reading", "outputs", "problems", "billing", "disk"]) assert.ok(await page.isChecked(r(`part_${k}`)), k);

    await page.fill(r("to"), "hv@example.org");
    await page.selectOption(r("frequency"), "weekly");
    await page.selectOption(r("weekday"), "2");
    await page.fill(r("time"), "18:30");
    await page.uncheck(r("part_disk"));
    await page.click(`${wg} [data-report-form] button[type=submit]`);
    await page.waitForFunction(() => /Bericht gespeichert/.test(document.querySelector("#toast")?.textContent));
    const c = (await api("mail")).config;
    assert.deepEqual(
      [c.report.to, c.report.frequency, c.report.weekday, c.report.time, c.report.off],
      ["hv@example.org", "weekly", 2, "18:30", ["disk"]],
    );
    assert.equal(c.host, "smtp.example.org", "Server bleibt");
    assert.equal(c.warn.to, "warn@example.org", "Warnungen bleiben");

    // falsche Uhrzeit wird abgelehnt
    await page.fill(r("time"), "");
    await page.click(`${wg} [data-report-form] button[type=submit]`);
    await page.waitForFunction(() => /Uhrzeit/.test(document.querySelector("#toast")?.textContent));
    assert.equal((await api("mail")).config.report.time, "18:30");
  },
};
