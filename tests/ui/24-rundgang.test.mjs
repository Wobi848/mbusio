import { assert, simLine } from "./helpers.mjs";

// Rundgang 27.10.2026: Funktionen der bisherigen Oberfläche, die gefehlt haben, und Fehler in
// Bedienung und Aussehen, die beim Durchklicken aufgefallen sind

const zl = "section[data-area=zaehler]";
let sim = ""; // eigene Simulation (die eingebauten lassen sich nicht ändern)
const box = (page, sel) => page.locator(sel).first().boundingBox();

export default {
  name: "Rundgang (Benennen in der Liste, Messwerte, Wiederholungen, Timeout, Abrechnung, Aussehen)",
  async setup(api) {
    sim = `SIMULATOR:${(await api("sim", "POST", { name: "Rundgang", from: "ok10" })).key}`;
    await simLine(api, sim);
    for (const a of ["1", "2", "3"]) {
      const m = await api("meters", "POST", { address: a, name: a === "1" ? "Whg 1" : "" });
      await api(`meters/${m.id}/read`, "POST");
    }
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
  },
  async run({ page, base, api }) {
    // Benennen in der Liste: Enter springt zum nächsten Zähler, „wie oben“ übernimmt den Ort
    await page.goto(base + "/#/meters");
    await page.waitForSelector(`${zl} [data-list] [role=option]`);
    await page.click(`${zl} [data-more]`);
    await page.click(`${zl} [data-naming]`);
    const name = (i) => page.locator(`${zl} [data-list] input[data-q=name]`).nth(i);
    const loc = (i) => page.locator(`${zl} [data-list] input[data-q=location]`).nth(i);
    assert.equal(await page.getAttribute(`${zl} [data-read-all]`, "class"), "tonal", "beim Benennen ist „Fertig“ der Hauptknopf");
    const same = (i) => page.locator(`${zl} [data-list] [data-q-same]`).nth(i);
    assert.equal(await same(0).isVisible(), false, "über der ersten Zeile steht nichts");
    assert.ok(await same(1).isVisible());
    // mittlere Breite: die Spalte Ort fehlt sonst, beim Benennen muss ihr Feld bleiben
    const viewport = page.viewportSize();
    await page.setViewportSize({ width: 1100, height: viewport.height });
    assert.ok(await loc(0).isVisible(), "Ort beim Benennen auch bei 1100 px");
    await page.setViewportSize({ width: 390, height: viewport.height });
    assert.ok(await loc(0).isVisible(), "Ort beim Benennen auch auf dem Handy");
    await page.setViewportSize(viewport);
    await name(1).fill("Whg 2");
    await page.keyboard.press("Enter");
    assert.equal(await page.evaluate(() => document.activeElement.dataset.id), "3", "Enter: nächster Zähler");
    await loc(0).fill("Keller");
    await loc(0).press("Tab");
    await page.waitForFunction(async () => (await (await fetch("/api/meters")).json())[0].location === "Keller");
    await page.locator(`${zl} [data-list] [data-q-same]`).nth(1).click();
    await page.waitForFunction(async () => (await (await fetch("/api/meters")).json())[1].location === "Keller");
    await page.click(`${zl} [data-naming-done]`);
    await page.waitForFunction(() => !document.querySelector("section[data-area=zaehler] [data-list] input"));
    assert.match(await page.textContent(`${zl} [data-list]`), /Whg 2/);

    // Messwerte exportieren und alle Auslesungen löschen (Menü „Liste“)
    await page.click(`${zl} [data-more]`);
    assert.match(await page.getAttribute(`${zl} [data-readings=csv]`, "href"), /export\?format=csv/);
    assert.match(await page.getAttribute(`${zl} [data-readings=json]`, "href"), /export\?format=json/);
    await page.click(`${zl} [data-readings-clear]`);
    await page.click(".ask button.danger");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("Auslesungen gelöscht"));
    assert.equal((await api("readings")).length, 0);

    // Aktionen des Zählers in einer Zeile, Häkchen der Vorlage normal gross
    await page.locator(`${zl} [data-list] [role=option]`).first().click();
    await page.waitForSelector(`${zl} [data-detail] [data-del]`);
    // beide in einem Zug messen (die Details können zwischen zwei Abfragen neu gebaut werden)
    const [read, del] = await page.evaluate((d) => ["[data-read]", "[data-del]"].map((s) => document.querySelector(`${d} ${s}`).getBoundingClientRect().toJSON()), `${zl} [data-detail]`);
    assert.ok(Math.abs(read.y - del.y) < 4, `Löschen in derselben Zeile wie Auslesen: ${JSON.stringify([read, del])}`);

    // Scan: Wiederholungen einstellbar, die Dauer folgt
    await page.goto(base + "/#/scan");
    await page.waitForSelector("[data-retries]");
    const before = await page.textContent("[data-estimate]");
    await page.selectOption("[data-retries]", "3");
    await page.waitForFunction((b) => document.querySelector("[data-estimate]").textContent !== b, before);

    // Anlage: Timeout der Linie, bleibt beim Verbinden über die Kopfzeile; Plan so breit wie die Felder
    await page.goto(base + "/#/site");
    await page.locator("section[data-area=anlage] [data-list] [role=option]").nth(2).click();
    await page.fill("[data-field=timeout_ms]", "800");
    await page.press("[data-field=timeout_ms]", "Tab");
    await page.waitForFunction(async () => (await (await fetch("/api/status")).json()).active.line.timeout_ms === 800);
    await api("connect", "POST", { port: sim, baud: 2400 });
    assert.equal((await api("status")).active.line.timeout_ms, 800, "Verbinden ohne Timeout behält ihn");
    const plan = await box(page, "section[data-area=anlage] .detail [data-plan=line]");
    const name_ = await box(page, "section[data-area=anlage] .detail [data-field=name]");
    assert.ok(plan.x + plan.width > name_.x + name_.width - 90, "Plan reicht fast bis zum Rand der Felder");

    // Weitergabe: E-Mail ohne Server ist aus; in der Abrechnung ist keine Karte gewählt
    await page.goto(base + "/#/transfer/email");
    await page.waitForSelector("[data-out-form=email]");
    assert.equal(await page.isChecked("[data-out-toggle=email]"), false);
    await page.click("[data-open-billing]");
    await page.waitForSelector(".btable");
    assert.equal(await page.locator(".out.active").count(), 0);
    // Vorschau: Zähler ohne Namen mit ID
    assert.match(await page.textContent(".btable tbody"), /ID \d{8}/);

    // Datenpunkt: Häkchen so gross wie sonst
    await page.goto(base + "/#/transfer");
    await page.reload();
    await page.locator("section[data-area=weitergabe] [data-list] [role=option]").first().click();
    const check = await box(page, "[data-dp-field=active]");
    assert.ok(check.width <= 18 && check.height <= 18, `Häkchen ${check.width}×${check.height}`);

    // Werkzeuge: neue Baudrate steht auf der des Busses
    await page.goto(base + "/#/tools");
    await page.waitForSelector("[data-new-baud]");
    assert.equal(await page.inputValue("[data-new-baud]"), "2400");

    // Simulator: „+ mehrere“ steht vor Anzahl und Medium
    await page.goto(base + "/#/simulator");
    await page.waitForSelector("[data-add-many]");
    assert.ok((await box(page, "[data-add-many]")).x < (await box(page, "[data-n]")).x);

    // Handy: Reiter in einer Zeile, Seite nicht breiter als der Bildschirm
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(base + "/#/tools");
    await page.waitForSelector(".tabs [data-area=werkzeuge][aria-current=page]");
    const tabs = await box(page, ".tab-scroll");
    assert.ok(tabs.height < 45, `Reiter ${tabs.height}px hoch`);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth), 0);
  },
};
