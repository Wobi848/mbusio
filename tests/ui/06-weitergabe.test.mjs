import { assert, simLine } from "./helpers.mjs";

// Weitergabe (Ausgänge mit Schalter und Zustand, Datenpunkte als
// Liste mit Details, Mehrfachauswahl, Adressen nach Muster)

const ENERGY = "instantaneous|0|0|0|energy|kWh";
const rows = (page) => page.locator("section[data-area=weitergabe] [data-list] [role=option]");
const detail = "section[data-area=weitergabe] [data-detail]";

export default {
  name: "Weitergabe (Ausgänge, Datenpunkte, Auswahl, Muster)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    for (const a of ["1", "2", "3"]) {
      await api("meters", "POST", { address: a, name: "Whg " + a });
      await api(`meters/${a}/read`, "POST");
      await api(`meters/${a}/datapoints`, "PUT", { keys: [ENERGY] });
    }
  },
  async run({ page, base, api }) {
    await page.goto(base + "/#/weitergabe");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=weitergabe] [data-list] [role=option]").length === 3);
    assert.match(await rows(page).nth(0).textContent(), /gut/);
    assert.match(await page.textContent("section[data-area=weitergabe] [data-outputs]"), /MQTT\s*aus/);

    // MQTT einschalten ohne Broker: Einstellungen gehen auf; Adresse eintragen und speichern
    await page.click("[data-out-toggle=mqtt]");
    await page.waitForSelector(`${detail} [data-out-form=mqtt]`);
    await page.fill(`${detail} [name=host]`, "127.0.0.1");
    await page.fill(`${detail} [name=port]`, "1");
    await page.click(`${detail} [type=submit]`);
    await page.waitForFunction(() => document.querySelector("[data-out-toggle=mqtt]")?.checked);
    const mq = (await api("mqtt")).config;
    assert.deepEqual([mq.host, mq.enabled], ["127.0.0.1", true], "nach „ein“ und Speichern ist MQTT ein");

    // Datenpunkt: Details rechts, Name im Feld ändern (Verlassen speichert)
    await rows(page).nth(0).click();
    await page.fill(`${detail} [data-dp-field=name]`, "Heizung Whg 1");
    await page.press(`${detail} [data-dp-field=name]`, "Enter");
    await page.waitForFunction(() => document.querySelector("section[data-area=weitergabe] [role=option]")?.textContent.includes("Heizung Whg 1"));

    // Mehrfachauswahl: an MQTT geben (Topics nach Muster), ausschalten
    await page.focus("section[data-area=weitergabe] [data-list]");
    await page.keyboard.press("Control+a");
    await page.waitForSelector("section[data-area=weitergabe] [data-selbar]:not([hidden])");
    await page.click("[data-bulk='out:mqtt:on']");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=weitergabe] [data-list] .chip.ok, section[data-area=weitergabe] [data-list] [title*='/']").length >= 3);
    const dps = (await api("datapoints?scope=line")).meters.flatMap((m) => m.datapoints);
    assert.ok(dps.every((d) => d.outputs.mqtt?.enabled && d.outputs.mqtt.address), "Topics fehlen");
    await page.click("[data-bulk=off]");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=weitergabe] [data-list] .row.off").length === 3);
    await page.keyboard.press("Escape");

    // Adressen nach Muster: Vorschau im Detailbereich, verwerfen
    await page.click("[data-readdress=mqtt]");
    await page.waitForSelector(`${detail} [data-cs-close]`);
    await page.click(`${detail} [data-cs-close]`);

    // MQTT ausschalten nur nach Rückfrage am Ort
    await page.click("[data-out-toggle=mqtt]");
    await page.click(".ask button.danger");
    await page.waitForFunction(async () => !document.querySelector("[data-out-toggle=mqtt]").checked);
    assert.equal((await api("mqtt")).config.enabled, false);
  },
};
