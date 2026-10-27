import net from "node:net";
import { assert, simLine } from "./helpers.mjs";

// E-Mail als Ausgang der Weitergabe (Konzept Weitergabe/E-Mail, Schritt 1): Hinweis in der
// Abrechnung führt zu den Einstellungen, Passwort bleibt auf dem Gerät, Port folgt der
// Verschlüsselung, Testmail, Zustand der Karte, Protokoll; dann abgelegte Datei senden.
// Kleiner SMTP-Server im Test.

const sec = "section[data-area=anlage]";

/// SMTP-Server ohne TLS: nimmt jede Anmeldung, sammelt die Nachrichten
function smtp() {
  const mails = [];
  const server = net.createServer((c) => {
    let buf = "";
    let data = null;
    const say = (s) => c.write(s + "\r\n");
    say("220 test ESMTP");
    c.on("data", (chunk) => {
      buf += chunk;
      let i;
      while ((i = buf.indexOf("\r\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 2);
        if (data !== null) {
          if (line === ".") {
            mails.push(data);
            data = null;
            say("250 queued");
          } else data += line + "\n";
          continue;
        }
        const u = line.toUpperCase();
        if (u.startsWith("EHLO")) say("250-test\r\n250 AUTH PLAIN");
        else if (u.startsWith("AUTH PLAIN")) say("235 ok");
        else if (u === "DATA") (data = ""), say("354 go");
        else if (u === "QUIT") say("221 bye"), c.end();
        else say("250 ok");
      }
    });
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok({ port: server.address().port, mails, server })));
}

export default {
  name: "E-Mail (Ausgang, Einstellungen, Testmail, Protokoll, Abrechnung senden)",
  async setup(api) {
    await simLine(api, "SIMULATOR:praxis");
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
  },
  async run({ page, base, api }) {
    const { port, mails, server } = await smtp();
    const wg = "section[data-area=weitergabe]";
    try {
      // Abrechnung: Empfänger; ohne Server ein Hinweis mit Sprung zur Weitergabe
      await page.goto(base + "/#/anlage");
      await page.locator(`${sec} [data-list] [role=option]`).first().click();
      await page.click("[data-billing]");
      await page.check("[data-b-auto]");
      await page.fill("[data-b-mail-to]", "a@example.org, kaputt");
      await page.press("[data-b-mail-to]", "Tab");
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("keine E-Mail-Adresse"));
      await page.fill("[data-b-mail-to]", "a@example.org");
      await page.press("[data-b-mail-to]", "Tab");
      await page.click("text=Weitergabe → E-Mail");

      // Weitergabe: Einstellungen der E-Mail offen, Karte „nicht eingerichtet“
      await page.waitForSelector(`${wg} [data-out-form=email]`);
      assert.match(await page.textContent(`${wg} [data-outputs]`), /E-Mail\s*nicht eingerichtet/);
      const f = (n) => `${wg} [data-out-form=email] [name=${n}]`;
      await page.fill(f("host"), "127.0.0.1");
      await page.selectOption(f("security"), "tls");
      assert.equal(await page.inputValue(f("port")), "465", "üblicher Port zur Verschlüsselung");
      await page.selectOption(f("security"), "none");
      await page.fill(f("port"), String(port));
      await page.fill(f("username"), "anna");
      await page.fill(f("password"), "geheim");
      await page.fill(f("from"), "mbusio@example.org");
      await page.fill(f("device"), "Keller-Pi");
      await page.fill(f("retry_days"), "3");
      await page.click(`${wg} [data-out-form=email] [type=submit]`);
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("E-Mail-Einstellungen gespeichert"));
      const m = await api("mail");
      assert.equal(m.has_password, true);
      assert.equal(m.config.password, "", "Passwort nie an den Browser");
      assert.equal(m.config.retry_days, 3);

      // Testmail aus den Einstellungen; Karte „bereit“, Eintrag im Protokoll
      await page.click(`${wg} [data-out-settings=email]`);
      await page.fill(`${wg} [data-mail-test-to]`, "a@example.org");
      await page.click(`${wg} [data-mail-test]`);
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("Testmail an a@example.org gesendet"));
      assert.match(mails[0], /Subject: mbusio Keller-Pi: Testmail/);
      await page.waitForFunction(() => /E-Mail\s*bereit/.test(document.querySelector("section[data-area=weitergabe] [data-outputs]").textContent));
      await page.click(`${wg} [data-open-log]`);
      await page.waitForFunction(() => /E-Mail[\s\S]*Testmail[\s\S]*a@example.org/.test(document.querySelector("[data-log-rows]")?.textContent));
      await page.click("[data-log-back]");

      // E-Mail aus: Karte sagt es, Abrechnung weist darauf hin
      await page.click(`${wg} [data-out-toggle=email]`);
      await page.click(".ask button.danger");
      await page.waitForFunction(async () => /E-Mail\s*aus/.test(document.querySelector("section[data-area=weitergabe] [data-outputs]").textContent));
      assert.equal((await api("mail")).config.enabled, false);
      await page.click(`${wg} [data-out-toggle=email]`);
      await page.waitForFunction(() => /E-Mail\s*bereit/.test(document.querySelector("section[data-area=weitergabe] [data-outputs]").textContent));

      // abgelegte Datei (Vormonat; der Zeitplan schickt sie selbst oder man drückt „Senden“)
      for (let i = 0; !(await api("projects/1/billing")).files.length; i++) {
        assert.ok(i < 30, "keine Datei nach 30 s");
        await page.waitForTimeout(1000);
      }
      await page.goto(base + "/#/anlage");
      await page.locator(`${sec} [data-list] [role=option]`).first().click();
      await page.click("[data-billing]");
      await page.waitForSelector("[data-b-send]");
      assert.equal(await page.locator(`section[data-area=weitergabe] .chip.warn.wide a[href="#/weitergabe/email"]`).count(), 0, "Server da: kein Hinweis");
      if (!(await api("projects/1/billing")).files[0].mail?.sent) await page.click("[data-b-send]");
      await page.waitForFunction(() => document.querySelector(".bfile .chip.ok")?.textContent === "gesendet");
      const sent = mails.find((x) => x.includes("Content-Disposition: attachment"));
      assert.ok(sent, "Anhang fehlt");
      assert.match(sent, /abrechnung-\d{4}-\d\d-01-/);

      // von Hand: „Per E-Mail senden“ mit eigenem Betreff
      await page.fill("[data-b-subject]", "{geraet} {zeitraum}");
      await page.press("[data-b-subject]", "Tab");
      await page.waitForFunction(() => /→ Keller-Pi \d\d\/\d{4}/.test(document.querySelector(".bset").textContent));
      await page.click("[data-b-mail-now]");
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("Abrechnung an a@example.org gesendet"));
      assert.match(mails.at(-1), /Subject: Keller-Pi \d\d\/\d{4}/);
      // Wartezeit nur bei „automatisch“, gespeichert
      await page.selectOption("[data-b-wait]", "5");
      for (let i = 0; (await api("projects/1/billing")).settings.wait_days !== 5; i++) {
        assert.ok(i < 10);
        await page.waitForTimeout(200);
      }
    } finally {
      server.close();
    }
  },
};
