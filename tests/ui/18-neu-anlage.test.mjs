import { assert, simLine } from "./helpers.mjs";

// Neue Oberfläche, Schritt 3: Bereich Anlage (Projektbaum mit festen Spalten, Details rechts,
// Anlegen ohne Browser-Dialog, Linie wechseln ohne Neuladen, Löschen mit Rückfrage am Ort)

const row = (page, text) => page.locator("[data-area=anlage] [data-list] [role=option]", { hasText: text });

export default {
  name: "Neue Oberfläche: Anlage (Baum, Zustand, Anlegen, Bearbeiten, Linie wechseln, Löschen)",
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    // Dauerbetrieb (braucht einen Zähler mit Plan), damit der Zustand der Anlage erscheint
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, base, api }) {
    let dialogs = 0;
    page.on("dialog", (d) => {
      dialogs++;
      d.dismiss();
    });
    await page.goto(base + "/#/anlage");
    await page.waitForSelector("[data-area=anlage] [role=option]");
    // ohne Neuladen: diese Markierung bleibt bis zum Ende
    await page.evaluate(() => (window.__nochDa = true));

    // Baum: Projekt, Gateway, Linie mit Anschluss und Dauerbetrieb; Zustand der Anlage oben
    assert.match(await row(page, "Haus A").textContent(), /SIMULATOR/);
    assert.ok(await page.isChecked("[data-area=anlage] [data-list] [data-op='1']"));
    assert.ok(await page.isVisible("[data-area=anlage] [data-plant]"));

    // Gateway anlegen: Feld an Ort und Stelle, Enter
    await page.click("[data-area=anlage] [data-add-gateway]");
    await page.fill("[data-area=anlage] [data-new=gateway]", "Gateway B");
    await page.press("[data-area=anlage] [data-new=gateway]", "Enter");
    await row(page, "Gateway B").waitFor();
    const gw = (await api("projects")).projects[0].gateways.find((g) => g.name === "Gateway B");
    assert.ok(gw, "Gateway angelegt");

    // Linie im neuen Gateway anlegen
    await page.click(`[data-area=anlage] [data-add-line='${gw.id}']`);
    await page.fill("[data-area=anlage] [data-new]", "Linie B1");
    await page.press("[data-area=anlage] [data-new]", "Enter");
    await row(page, "Linie B1").waitFor();

    // Details: Escape setzt zurück, Enter speichert
    await row(page, "Linie B1").locator(".name").click();
    const name = "[data-area=anlage] [data-detail] [data-kind=line][data-field=name]";
    await page.waitForFunction((sel) => document.querySelector(sel)?.value === "Linie B1", name);
    await page.fill(name, "falsch");
    await page.press(name, "Escape");
    assert.equal(await page.inputValue(name), "Linie B1");
    await page.fill(name, "Linie B1 neu");
    await page.press(name, "Enter");
    await row(page, "Linie B1 neu").waitFor();
    // ein neues Gateway hat schon seine erste Linie; die eben angelegte ist die zweite
    const line = (await api("projects")).projects[0].gateways.find((g) => g.id === gw.id).lines.find((l) => l.name === "Linie B1 neu");
    assert.ok(line, "Name gespeichert");

    // Linie wechseln ohne Neuladen: Kopfzeile zeigt die neue Linie
    await row(page, "Linie B1 neu").locator("[data-use]").click();
    await page.waitForFunction(async (id) => (await (await fetch("api/status")).json()).active.line.id === id, line.id);
    await page.waitForFunction(() => document.querySelector("header")?.textContent.includes("Linie B1 neu"));

    // Löschen mit Rückfrage am Ort (keine Browser-Dialoge); erst Haus A wieder aktiv machen
    await row(page, "Haus A").locator("[data-use]").click();
    await page.waitForFunction(async () => (await (await fetch("api/status")).json()).active.line.name === "Haus A");
    await row(page, "Linie B1 neu").locator(".name").click();
    await page.click("[data-area=anlage] [data-detail] [data-delete=line]");
    await page.click(".ask button.danger");
    await row(page, "Linie B1 neu").waitFor({ state: "detached" });
    const left = (await api("projects")).projects[0].gateways.find((g) => g.id === gw.id).lines.map((l) => l.name);
    assert.ok(!left.includes("Linie B1 neu"), `gelöscht: ${left}`);

    assert.equal(dialogs, 0, "keine Browser-Dialoge");
    assert.ok(await page.evaluate(() => window.__nochDa), "Seite nie neu geladen");
  },
};
