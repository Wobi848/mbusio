import { assert, simLine } from "./helpers.mjs";

// Weitergabe → Protokoll: was wartet (Broker nicht erreichbar), Filter nach Ergebnis und
// Datenpunkt, vom Datenpunkt aus vorgefiltert, CSV, zurück zu den Datenpunkten

const ENERGY = "instantaneous|0|0|0|energy|kWh";
const sec = "section[data-area=weitergabe]";
const rows = `${sec} [data-log-rows] .row`;

export default {
  name: "Weitergabe-Protokoll (wartet, Filter, je Datenpunkt, CSV, zurück)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    for (const a of ["1", "2"]) {
      await api("meters", "POST", { address: a, name: "Whg " + a });
      await api(`meters/${a}/read`, "POST");
      await api(`meters/${a}/datapoints`, "PUT", { keys: [ENERGY] });
    }
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
    // Broker nicht erreichbar: alles bleibt in der Warteschlange
    const c = (await api("mqtt")).config;
    await api("mqtt", "PUT", { config: { ...c, host: "127.0.0.1", port: 1, enabled: true }, password: null });
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/weitergabe");
    await page.waitForSelector(`${sec} [data-list] [role=option]`);
    await page.focus(`${sec} [data-list]`);
    await page.keyboard.press("Control+a");
    await page.click("[data-bulk='out:mqtt:on']");
    await page.keyboard.press("Escape");
    for (const a of ["1", "2"]) await api(`meters/${a}/read`, "POST");

    // Protokoll statt der Datenpunkte: wartende Werte mit Datenpunkt und Wert
    await page.click("[data-open-log]");
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 4, rows);
    assert.ok(await page.isHidden(`${sec} [data-list]`), "Datenpunkte bleiben sichtbar");
    assert.match(await page.textContent(rows), /Whg \d · Energie.*kWh.*wartet/s);
    const waiting = Number((await page.textContent("[data-log-result=waiting] .n")).trim());
    assert.ok(waiting >= 4, `wartet ${waiting}`);

    // Filter: nur Verworfenes → leer, mit Hinweis
    await page.click("[data-log-result=dropped]");
    await page.waitForFunction((sel) => !document.querySelectorAll(sel).length, rows);
    assert.match(await page.textContent(`${sec} [data-log-empty]`), /Keine Einträge/);
    await page.click(`[data-log-result=""]`);
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 4, rows);

    // CSV mit denselben Filtern
    assert.match(await page.getAttribute("[data-log-csv]", "href"), /out-log\?.*format=csv/);
    // Excel: UTF-8 mit BOM (fetch().text() entfernt es beim Lesen, darum die Bytes)
    const [bom, csv] = await page.evaluate(async () => {
      const b = new Uint8Array(await (await fetch(document.querySelector("[data-log-csv]").href)).arrayBuffer());
      return [[...b.slice(0, 3)], new TextDecoder().decode(b)];
    });
    assert.deepEqual(bom, [0xef, 0xbb, 0xbf]);
    assert.match(csv, /^Zeit;Ausgang;Datenpunkt;Adresse;Wert;Qualität;Ergebnis;Grund\r\n/);
    assert.match(csv, /MQTT;Whg \d · Energie;mbusio\/.*;\d+ kWh;gut;wartet;/);

    // zurück; vom Datenpunkt aus vorgefiltert, Filter lässt sich aufheben
    await page.click("[data-log-back]");
    await page.waitForSelector(`${sec} [data-list] [role=option]`);
    await page.locator(`${sec} [data-list] [role=option]`).first().click();
    await page.click("[data-dp-log]");
    await page.waitForSelector("[data-log-dp-clear]");
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 2, rows);
    const names = await page.$$eval(rows, (l) => l.map((r) => r.children[2].textContent));
    assert.equal(new Set(names).size, 1, names.join());
    await page.click("[data-log-dp-clear]");
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 4, rows);

    // Escape schliesst das Protokoll
    await page.keyboard.press("Escape");
    await page.waitForSelector(`${sec} [data-list] [role=option]`);
  },
};
