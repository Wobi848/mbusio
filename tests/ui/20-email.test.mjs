import net from "node:net";
import { assert, simLine } from "./helpers.mjs";

// Abrechnung per E-Mail (Konzept Export, Teil C): Empfänger, Mailserver mit Passwort (bleibt auf
// dem Gerät), Testmail, abgelegte Datei senden; kleiner SMTP-Server im Test

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
  name: "Abrechnung per E-Mail (Empfänger, Server, Testmail, Datei senden)",
  async setup(api) {
    await simLine(api, "SIMULATOR:praxis");
    const m = await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api(`meters/${m.id}/read`, "POST");
  },
  async run({ page, base, api }) {
    const { port, mails, server } = await smtp();
    try {
      await page.goto(base + "/#/anlage");
      await page.locator(`${sec} [data-list] [role=option]`).first().click();
      await page.click("[data-billing]");
      await page.check("[data-b-auto]");

      // Empfänger: falsche Adresse wird abgelehnt, richtige gespeichert
      await page.fill("[data-b-mail-to]", "a@example.org, kaputt");
      await page.press("[data-b-mail-to]", "Tab");
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("keine E-Mail-Adresse"));
      await page.fill("[data-b-mail-to]", "a@example.org");
      await page.press("[data-b-mail-to]", "Tab");
      // Mailserver: offen, weil noch keiner eingetragen ist
      await page.waitForSelector("details[data-mail][open]");
      await page.fill("[data-mail-form] [name=host]", "127.0.0.1");
      await page.selectOption("[data-mail-form] [name=security]", "tls");
      assert.equal(await page.inputValue("[data-mail-form] [name=port]"), "465", "üblicher Port zur Verschlüsselung");
      await page.selectOption("[data-mail-form] [name=security]", "none");
      await page.fill("[data-mail-form] [name=port]", String(port));
      await page.fill("[data-mail-form] [name=username]", "anna");
      await page.fill("[data-mail-form] [name=password]", "geheim");
      await page.fill("[data-mail-form] [name=from]", "mbusio@example.org");
      await page.click("[data-mail-form] [type=submit]");
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("gespeichert"));
      const m = await api("mail");
      assert.equal(m.has_password, true);
      assert.equal(m.config.password, "", "Passwort nie an den Browser");

      await page.click("[data-mail-test]");
      await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("Testmail gesendet"));
      assert.match(mails[0], /To: a@example.org/);

      // abgelegte Datei (Vormonat; der Zeitplan schickt sie selbst oder man drückt „Senden“)
      for (let i = 0; !(await api("projects/1/billing")).files.length; i++) {
        assert.ok(i < 30, "keine Datei nach 30 s");
        await page.waitForTimeout(1000);
      }
      await page.click("[data-billing-close]");
      await page.click("[data-billing]");
      await page.waitForSelector("[data-b-send]");
      if (!(await api("projects/1/billing")).files[0].mail?.sent) await page.click("[data-b-send]");
      await page.waitForFunction(() => document.querySelector(".bfile .chip.ok")?.textContent === "gesendet");
      const sent = mails.find((x) => x.includes("Content-Disposition: attachment"));
      assert.ok(sent, "Anhang fehlt");
      assert.match(sent, /abrechnung-\d{4}-\d\d-01-/);
    } finally {
      server.close();
    }
  },
};
