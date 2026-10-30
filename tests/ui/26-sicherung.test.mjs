import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assert, simLine } from "./helpers.mjs";

// Sicherung im Einstellungsmenü (Konzept Sicherung): herunterladen, einspielen fragt mit dem
// Inhalt nach, fremde Dateien werden abgelehnt. Einspielen mit Neustart prüft der Rust-Test
// (der Neustart würde den Testserver ersetzen).

export default {
  name: "Sicherung (herunterladen, Rückfrage mit Inhalt, fremde Datei)",
  async setup(api) {
    await simLine(api);
    for (const a of ["1", "2"]) await api("meters", "POST", { address: a, name: "Whg " + a });
    await api("meters/1/read", "POST");
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/zaehler");
    await page.click("#settings");
    await page.waitForSelector("#settings-menu:not([hidden]) #backup-get");

    // herunterladen: eine Datenbank-Datei mit Gerät und Datum im Namen
    const [dl] = await Promise.all([page.waitForEvent("download"), page.click("#backup-get")]);
    assert.match(dl.suggestedFilename(), /^mbusio-sicherung-.+-\d{4}-\d{2}-\d{2}\.db$/);
    const file = await dl.path();
    assert.equal(fs.readFileSync(file).subarray(0, 15).toString(), "SQLite format 3");

    // einspielen: Rückfrage zeigt, was drin ist; Abbrechen ändert nichts
    await page.setInputFiles("#backup-file", file);
    await page.waitForSelector(".ask");
    assert.match(await page.textContent(".ask"), /2 Zähler · 1 Messwerte/);
    await page.click(".ask button.quiet");
    await page.waitForSelector(".ask", { state: "detached" });

    // fremde Datei: klare Meldung statt Rückfrage
    const other = path.join(os.tmpdir(), "mbusio-keine-sicherung.db");
    fs.writeFileSync(other, "irgendwas");
    await page.click("#settings").catch(() => {});
    await page.setInputFiles("#backup-file", other);
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("keine Sicherung von mbusio"));
    assert.equal(await page.locator(".ask").count(), 0);

    // Sicherungs-Passwort eingestellt (automatische Sicherung): Herunterladen verschlüsselt
    // (.mbusio, Kennung MBUSIOV2); Einspielen fragt erst nach dem Passwort
    await api("backup/auto", "PUT", {
      config: { enabled: false, target: "folder", folder: "/tmp", host: "", port: 21, user: "", dir: "", tls: true, insecure: false, time: "02:30", keep: 7 },
      secret: "geheim",
    });
    const [dl2] = await Promise.all([page.waitForEvent("download"), page.click("#backup-get")]);
    assert.match(dl2.suggestedFilename(), /\.mbusio$/);
    const file2 = await dl2.path();
    assert.equal(fs.readFileSync(file2).subarray(0, 8).toString(), "MBUSIOV2");

    await page.setInputFiles("#backup-file", file2);
    await page.waitForSelector(".ask input");
    await page.fill(".ask input", "geheim");
    await page.click(".ask button.danger");
    await page.waitForFunction(() => document.querySelector(".ask")?.textContent.includes("Zähler"));
    assert.match(await page.textContent(".ask"), /2 Zähler/);
    await page.click(".ask button.quiet");
    await page.waitForSelector(".ask", { state: "detached" });
  },
};
