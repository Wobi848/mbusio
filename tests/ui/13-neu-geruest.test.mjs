import { assert, simLine } from "./helpers.mjs";

// Neue Oberfläche (web/neu), Schritt 0: Gerüst. Prüft die Vorgaben aus
// docs/konzept-oberflaeche-neu.md §2: kein Neuladen, kein Fokusverlust, keine Browser-Dialoge,
// Escape, Tasten, Rückfrage am Ort, Liste mit Schlüssel.

export default {
  name: "Neue Oberfläche: Gerüst (Kopfzeile, Linie wechseln, Tastatur, Dauerbetrieb)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    await api("gateways/1/lines", "POST", { name: "Haus B", connection: "SIMULATOR", baud: 2400 });
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("lines/1/plan", "PUT", { plan: "daily 00:05" });
  },
  async run({ page, base }) {
    let dialogs = 0;
    page.on("dialog", () => dialogs++);
    await page.goto(base + "/neu/#/zaehler");
    await page.waitForFunction(() => document.querySelector("#line b")?.textContent === "Haus A");
    assert.match(await page.textContent("#conn-state"), /Verbunden/);
    assert.equal(await page.getAttribute(".tabs [data-area=zaehler]", "aria-current"), "page");

    // Linie wechseln ohne Neuladen: Merker im Fenster bleibt, Bereich bleibt
    await page.evaluate(() => (window.__kept = true));
    await page.click("#line");
    await page.click("#line-menu [role=menuitem]:has-text('Haus B')");
    await page.waitForFunction(() => document.querySelector("#line b")?.textContent === "Haus B");
    assert.ok(await page.evaluate(() => window.__kept), "Seite wurde neu geladen");
    assert.ok(page.url().endsWith("#/zaehler"));

    // Escape schliesst das Menü, Fokus zurück am Knopf
    await page.click("#line");
    assert.ok(await page.isVisible("#line-menu"));
    await page.keyboard.press("Escape");
    assert.ok(await page.isHidden("#line-menu"));
    assert.equal(await page.evaluate(() => document.activeElement.id), "line");

    // Statusabfrage (alle 5 s) nimmt den Fokus nicht weg
    await page.waitForTimeout(6000);
    assert.equal(await page.evaluate(() => document.activeElement.id), "line", "Fokus verloren");

    // Tasten 1–6 wechseln den Bereich
    await page.keyboard.press("3");
    await page.waitForFunction(() => location.hash === "#/weitergabe");
    assert.equal(await page.getAttribute(".tabs [data-area=weitergabe]", "aria-current"), "page");

    // Dauerbetrieb: ohne Plan gesperrt mit Grund; mit Plan ein, aus nur nach Rückfrage am Ort
    assert.ok(await page.isDisabled("#op-switch"));
    assert.match(await page.textContent("#op-text"), /kein Ausleseplan/);
    await page.click("#line");
    await page.click("#line-menu [role=menuitem]:has-text('Haus A')");
    await page.waitForFunction(() => !document.querySelector("#op-switch").disabled);
    await page.click("#op-switch");
    await page.waitForSelector("#op.on");
    await page.waitForSelector("#op-all:not([hidden])");
    await page.click("#op-switch");
    await page.waitForSelector(".ask");
    await page.keyboard.press("Escape");
    assert.ok(await page.isHidden(".ask").catch(() => true));
    assert.ok(await page.isChecked("#op-switch"), "Escape darf nicht ausschalten");
    await page.click("#op-switch");
    await page.click(".ask button.danger");
    await page.waitForSelector("#op:not(.on)");
    assert.equal(dialogs, 0, "Browser-Dialog benutzt");

    // Liste mit Schlüssel: unveränderte Zeile bleibt dasselbe Element, geänderte wird ersetzt
    const same = await page.evaluate(async () => {
      const { list } = await import("./neu/view.js");
      const box = document.createElement("div");
      const row = (x) => `<div>${x.text}</div>`;
      list(box, [{ id: 1, text: "a" }, { id: 2, text: "b" }, { id: 3, text: "c" }], (x) => x.id, row);
      const [one, two] = box.children;
      list(box, [{ id: 2, text: "b" }, { id: 1, text: "A" }, { id: 4, text: "d" }], (x) => x.id, row);
      return {
        kept: box.children[0] === two,
        replaced: box.children[1] !== one && box.children[1].textContent === "A",
        order: [...box.children].map((c) => c.dataset.key).join(","),
      };
    });
    assert.deepEqual(same, { kept: true, replaced: true, order: "2,1,4" });
  },
};
