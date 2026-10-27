// Gemeinsame Schritte der Browser-Tests
import net from "node:net";
import assert from "node:assert/strict";
export { assert };

/// Linie 1 mit einer Simulation verbinden und aktiv machen. `timeout_ms`: kurze Antwortzeit für
/// lange Scans (der Simulator antwortet sofort; 0 = automatisch wie im Feld)
export async function simLine(api, connection = "SIMULATOR", baud = 2400, timeout_ms = 0) {
  await api("lines/1", "PUT", { name: "Haus A", connection, baud, timeout_ms });
  await api("lines/1/activate", "POST");
}

/// Scan im Bereich Scan starten (Taste S) und warten, bis er fertig ist: erst läuft er (Knopf
/// „Abbrechen“), dann nicht mehr
export async function scan(page, timeout = 120000) {
  await page.keyboard.press("s");
  await scanDone(page, timeout);
}

export async function scanDone(page, timeout = 120000) {
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent === "Abbrechen", null, { timeout: 10000 }).catch(() => {});
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent !== "Abbrechen", null, { timeout });
}

/// SMTP-Server ohne TLS: nimmt jede Anmeldung, sammelt die Nachrichten
export function smtp() {
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
