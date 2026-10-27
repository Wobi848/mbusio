import { assert, simLine, smtp } from "./helpers.mjs";

// Meldungen und Warnmail (Konzept E-Mail 5): erkannter Zählertausch erscheint in der
// Statusleiste, „Zeigen“ springt zum Zähler, „gesehen“ beruhigt den Zähler; mit Empfänger geht
// eine gebündelte Warnmail raus. Einstellungen unter Weitergabe → E-Mail → Warnungen.

const wg = "section[data-area=weitergabe]";

export default {
  name: "Meldungen und Warnmail (Statusleiste, Zeigen, gesehen, Einstellungen)",
  async setup(api) {
    const { key } = await api("sim", "POST", { name: "Warnung", from: "ok10" });
    await simLine(api, `SIMULATOR:${key}`);
    await api("meters", "POST", { address: "2", name: "Whg 2" });
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
    const sim = (await api("sim")).scenarios.find((s) => s.key === key);
    sim.meters[0].id = "87654321";
    await api(`sim/${key}`, "PUT", { name: sim.name, meters: sim.meters });
    await api(`meters/${m.id}/read`, "POST");
  },
  async run({ page, base, api }) {
    const { port, mails, server } = await smtp();
    try {
      // Mailserver und Warn-Empfänger über die Oberfläche
      await page.goto(base + "/#/weitergabe/email");
      const f = (n) => `${wg} [data-out-form=email] [name=${n}]`;
      await page.waitForSelector(f("host"));
      await page.fill(f("host"), "127.0.0.1");
      await page.selectOption(f("security"), "none");
      await page.fill(f("port"), String(port));
      await page.fill(f("username"), "anna");
      await page.fill(f("password"), "geheim");
      await page.fill(f("from"), "mbusio@example.org");
      await page.fill(f("device"), "Keller-Pi");
      await page.click(`${wg} [data-out-form=email] button[type=submit]`);
      // gespeichert ist erst, wenn das Formular zu ist (sonst schliesst das Speichern das neu geöffnete)
      await page.waitForSelector(`${wg} [data-out-form=email]`, { state: "detached" });
      await page.goto(base + "/#/weitergabe/email");
      await page.click(`${wg} [data-warn-section] summary`);
      const w = (n) => `${wg} [data-warn-form] [name=${n}]`;
      assert.equal(await page.inputValue(w("line_minutes")), "30");
      assert.equal(await page.inputValue(w("interval_minutes")), "60");
      assert.ok(await page.isChecked(w("kind_swap")));
      await page.fill(w("to"), "hv@example.org");
      await page.uncheck(w("kind_battery"));
      await page.selectOption(w("interval_minutes"), "0");
      await page.click(`${wg} [data-warn-form] button[type=submit]`);
      await page.waitForFunction(() => /Warnungen gespeichert/.test(document.querySelector("#toast")?.textContent));
      const c = (await api("mail")).config.warn;
      assert.deepEqual([c.to, c.off, c.interval_minutes, c.resolved], ["hv@example.org", ["battery"], 0, true]);
      assert.equal((await api("mail")).config.host, "127.0.0.1", "Server bleibt beim Speichern der Warnungen");

      // Statusleiste: Meldung erscheint (Zeitplan prüft alle 10 s)
      const chip = page.locator("#warnings [data-warn-open]");
      await chip.waitFor({ timeout: 30000 });
      assert.match(await chip.textContent(), /Meldungen 1/);
      assert.match(await chip.getAttribute("class"), /warn/);
      await chip.click();
      const row = page.locator("#warn-list .wrow").first();
      assert.match(await row.textContent(), /Whg 1: Zählertausch erkannt.*87654321/);

      // Warnmail gebündelt mit Betreff nach Vorgabe
      for (let i = 0; !mails.length; i++) {
        assert.ok(i < 40, "keine Warnmail");
        await page.waitForTimeout(500);
      }
      assert.match(mails[0], /^Subject: mbusio Keller-Pi: 1 neue Probleme$/m);
      assert.match(mails[0], /^To: hv@example.org$/m);

      // gesehen: Zähler wird ruhig, Meldung bleibt in der Liste
      await row.locator("[data-warn-seen]").click();
      await page.waitForFunction(() => document.querySelector("#warnings [data-warn-open]")?.classList.contains("quiet"));
      assert.equal((await api("status")).warnings.unseen, 0);

      // Zeigen: zum Zähler, Tausch bestätigen → Meldung verschwindet von selbst
      await row.locator("[data-warn-go]").click();
      await page.waitForFunction(() => /Whg 1/.test(document.querySelector("section[data-area=zaehler] [data-list] .row.active")?.textContent));
      await page.click("section[data-area=zaehler] [data-swap-confirm]");
      await page.locator("#warnings [data-warn-open]").waitFor({ state: "detached", timeout: 30000 });
    } finally {
      server.close();
    }
  },
};
