import { assert, simLine } from "./helpers.mjs";

// Neue Oberfläche, Schritt 5: Bereich Werkzeuge (ein Ziel für alle Werkzeuge, Auslesen,
// Primäradresse mit Rückfrage am Ort, Telegramm analysieren)

const area = "[data-area=werkzeuge]";

export default {
  name: "Neue Oberfläche: Werkzeuge (Ziel wählen, Auslesen, Adresse ändern, Analyse)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    await api("meters", "POST", { address: "1", name: "Whg 1" });
  },
  async run({ page, base, api }) {
    let dialogs = 0;
    page.on("dialog", (d) => {
      dialogs++;
      d.dismiss();
    });
    await page.goto(base + "/neu/#/werkzeuge");
    await page.waitForSelector(`${area} [data-read]`);

    // ohne Ziel gesperrt, mit Grund
    assert.ok(await page.isDisabled(`${area} [data-read]`));
    // Ziel aus der Zählerliste über die Suche: tippen, Enter
    await page.fill(`${area} [data-target-input]`, "whg");
    await page.waitForSelector(`${area} [data-picks]:not([hidden]) [data-pick]`);
    await page.press(`${area} [data-target-input]`, "Enter");
    await page.waitForFunction((a) => document.querySelector(`${a} [data-tstate]`)?.textContent.includes("Whg 1"), area);
    assert.ok(!(await page.isDisabled(`${area} [data-read]`)));

    // Auslesen: Werte im Ergebnis, Telegramme im Log
    await page.click(`${area} [data-read]`);
    await page.waitForFunction((a) => /kWh/.test(document.querySelector(`${a} [data-result]`)?.textContent || ""), area, { timeout: 20000 });
    await page.waitForFunction((a) => /RX/.test(document.querySelector(`${a} [data-log]`)?.textContent || ""), area);

    // Primäradresse ändern: Rückfrage am Ort, danach folgt die Zählerliste (Server)
    await page.fill(`${area} [data-new-pa]`, "77");
    await page.click(`${area} [data-set-pa]`);
    await page.click(".ask button.danger");
    await page.waitForFunction(async () => (await (await fetch("api/meters")).json()).some((m) => m.name === "Whg 1" && m.address === "77"), null, {
      timeout: 20000,
    });

    // Telegramm analysieren
    await page.fill(`${area} input[data-hex]`, "10 40 01 41 16");
    await page.press(`${area} input[data-hex]`, "Enter");
    await page.waitForSelector(`${area} [data-analysis]:not([hidden])`);
    assert.ok((await page.textContent(`${area} [data-analysis]`)).length > 10);

    assert.equal(dialogs, 0, "keine Browser-Dialoge");
  },
};
