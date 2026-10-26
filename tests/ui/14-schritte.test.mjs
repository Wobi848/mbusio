import { assert, scanDone, simLine } from "./helpers.mjs";

// „Nächster Schritt“ in der Statusleiste führt von der leeren Linie bis zum Ausgang:
// Anschluss, suchen, übernehmen, auslesen, Gestörte, Bericht, dann Dauerbetrieb, Werte, Ausgang

const next = (page) => page.textContent("#next");
const waitNext = (page, text, timeout = 30000) =>
  page.waitForFunction((t) => document.querySelector("#next:not([hidden])")?.textContent.includes(t), text, { timeout });

export default {
  name: "Nächster Schritt (Service bis Bericht, Dauerbetrieb bis Ausgang)",
  async run({ page, base, api, ctx }) {
    // neue Datenbank: Linie ohne Anschluss
    await page.goto(base + "/");
    await waitNext(page, "Anschluss festlegen");
    await simLine(api, "SIMULATOR:ok10");
    await page.reload();
    await waitNext(page, "Zähler suchen");

    // suchen: Schritt öffnet den Scan und startet ihn (Bereich vorher eingrenzen)
    await page.goto(base + "/#/scan");
    await page.fill("[data-to]", "12");
    await page.press("[data-to]", "Tab");
    await page.click("#next");
    await scanDone(page);
    await page.click("[data-merge]");
    await waitNext(page, "Zähler auslesen");
    await page.click("#next");
    await page.waitForSelector(".tabs [data-area=zaehler][aria-current=page]");
    await waitNext(page, "Bericht", 60000);

    // ein Zähler antwortet nicht: Schritt zeigt die Gestörten
    const dead = await api("meters", "POST", { address: "200" });
    await api(`meters/${dead.id}/read`, "POST").catch(() => {});
    await page.reload();
    await waitNext(page, "Gestörte zeigen");
    await page.click("#next");
    await page.waitForSelector("[data-filter=err][aria-pressed=true]");
    assert.equal(await page.locator("section[data-area=zaehler] [data-list] [role=option]").count(), 1);
    await api(`meters/${dead.id}`, "DELETE");
    await page.reload();

    // Service fertig: Bericht in neuem Fenster, mit allen Zählern
    await waitNext(page, "Bericht");
    const [report] = await Promise.all([ctx.waitForEvent("page"), page.click("#next")]);
    await report.waitForSelector("#table tbody tr");
    assert.equal(await report.locator("#table tbody tr").count(), 10);
    await report.close();

    // Dauerbetrieb: Plan und Schalter in der Kopfzeile, dann Werte und Ausgang
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await page.reload();
    await page.waitForFunction(() => !document.querySelector("#op-switch").disabled);
    await page.click("#op-switch");
    await waitNext(page, "Werte zum Weitergeben wählen");
    const m = (await api("meters"))[0];
    await api(`meters/${m.id}/datapoints`, "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
    await page.reload();
    await waitNext(page, "Ausgang einrichten");
    await page.click("#next");
    await page.waitForSelector(".tabs [data-area=weitergabe][aria-current=page]");
    assert.match(await next(page), /Ausgang einrichten/);
  },
};
