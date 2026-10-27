import { assert, smtp } from "./helpers.mjs";

// Bericht per E-Mail (Konzept E-Mail 6): Einstellungen unter Weitergabe → E-Mail → Bericht mit
// Vorgaben, speichern ändert nur den Bericht, Vorschau des Betreffs, „Bericht jetzt senden“.
// Wann er von selbst rausgeht, prüft der Rust-Test im Zeitplan.

const wg = "section[data-area=weitergabe]";

export default {
  name: "Bericht (Einstellungen mit Vorgaben, Betreff, jetzt senden)",
  async run({ page, base, api }) {
    const { port, mails, server } = await smtp();
    try {
      const mail = (await api("mail")).config;
      await api("mail", "PUT", {
        config: { ...mail, host: "127.0.0.1", port, security: "none", username: "anna", from: "mbusio@example.org", device: "Keller-Pi", warn: { to: "warn@example.org" } },
        password: "geheim",
      });
      await page.goto(base + "/#/weitergabe/email");
      // Testmail geht vorbelegt an den, der die Warnungen bekommt
      assert.equal(await page.inputValue(`${wg} [data-mail-test-to]`), "warn@example.org");
      await page.click(`${wg} [data-report-section] summary`);
      const r = (n) => `${wg} [data-report-form] [name=${n}]`;
      assert.equal(await page.inputValue(r("frequency")), "daily");
      assert.equal(await page.inputValue(r("time")), "07:00");
      assert.equal(await page.inputValue(r("weekday")), "0");
      for (const k of ["reading", "outputs", "problems", "billing", "disk"]) assert.ok(await page.isChecked(r(`part_${k}`)), k);
      // bleibt offen, auch wenn die Statusleiste neu abfragt
      await page.waitForTimeout(5500);
      assert.ok(await page.isVisible(r("to")), "Abschnitt bleibt offen");

      // Vorschau des Betreffs: Vorgabe, beim Tippen neu, unbekannte Platzhalter bleiben
      const preview = `${wg} [data-report-form] [data-subject-preview]`;
      assert.match(await page.textContent(preview), /^→ mbusio Keller-Pi: Bericht \d\d\.\d\d\.\d{4}$/);
      await page.fill(r("subject"), "Pi {geraet} {anzahl}");
      assert.equal(await page.textContent(preview), "→ Pi Keller-Pi {anzahl}");
      await page.fill(r("subject"), "");

      // ohne Empfänger: jetzt senden erklärt, warum nicht
      await page.click(`${wg} [data-report-now]`);
      await page.waitForFunction(() => /Bericht an/.test(document.querySelector("#toast")?.textContent));

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
      assert.equal(c.host, "127.0.0.1", "Server bleibt");
      assert.equal(c.warn.to, "warn@example.org", "Warnungen bleiben");

      // jetzt senden: geht an „Bericht an“, Abschnitt bleibt offen
      await page.click(`${wg} [data-report-now]`);
      await page.waitForFunction(() => /Bericht gesendet an hv@example.org/.test(document.querySelector("#toast")?.textContent));
      assert.equal(mails.length, 1);
      assert.match(mails[0], /^Subject: mbusio Keller-Pi: Bericht \d\d\.\d\d\.\d{4}$/m);
      assert.match(mails[0], /^To: hv@example.org$/m);
      assert.ok(await page.isVisible(r("to")), "Abschnitt bleibt offen");

      // falsche Uhrzeit wird abgelehnt
      await page.fill(r("time"), "");
      await page.click(`${wg} [data-report-form] button[type=submit]`);
      await page.waitForFunction(() => /Uhrzeit/.test(document.querySelector("#toast")?.textContent));
      assert.equal((await api("mail")).config.report.time, "18:30");
    } finally {
      server.close();
    }
  },
};
