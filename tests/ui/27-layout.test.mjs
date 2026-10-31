import { assert, simLine } from "./helpers.mjs";

// In jeder Sprache und Breite: nichts läuft aus dem Fenster, Kopfzeile und Laschen überlappen nicht,
// auch mit langen Namen und auf einer Linie ohne Anschluss

const AREAS = ["anlage", "scan", "zaehler", "weitergabe", "werkzeuge", "simulator"];
const WIDTHS = [390, 1000, 1250];

const problems = (page) =>
  page.evaluate(() => {
    const box = (e) => e.getBoundingClientRect();
    const shown = (e) => box(e).width > 0 && box(e).height > 0 && getComputedStyle(e).visibility !== "hidden";
    const overlap = (a, b) => a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1;
    const out = [];
    for (const e of document.querySelectorAll("button, a.quiet, a.button, select, input:not([type=file])")) {
      if (!shown(e) || e.closest(".tab-scroll, .seg, .menu, .detail:not(.open), .rows")) continue;
      if (box(e).right > innerWidth + 1 || box(e).left < -1) out.push(`ausserhalb: ${(e.textContent || e.id).trim().slice(0, 30)}`);
    }
    const parts = ["#project", "#line", "#conn-state", "#conn-action", "#op-switch", "#op > b", "#op-text", "#op-all", "#settings", ".brand"]
      .map((s) => [s, document.querySelector(s)])
      .filter(([, e]) => e && !e.hidden && shown(e));
    parts.forEach(([a, x], i) => parts.slice(i + 1).forEach(([b, y]) => overlap(box(x), box(y)) && out.push(`überlappt: ${a} / ${b}`)));
    const next = document.querySelector("#next");
    if (!next.hidden && overlap(box(next), box(document.querySelector(".tab-scroll")))) out.push("überlappt: #next / Laschen");
    for (const e of document.querySelectorAll(".top button, .toolbar button, #next, .selbar button")) {
      if (shown(e) && e.scrollWidth > e.clientWidth + 1) out.push(`abgeschnitten: ${e.textContent.trim().slice(0, 30)}`);
    }
    if (document.documentElement.scrollWidth > innerWidth) out.push("Seite zu breit");
    return out;
  });

export default {
  name: "Layout (alle Sprachen, Breiten und Bereiche, lange Namen, ohne Anschluss)",
  async setup(api) {
    await simLine(api);
    await api("projects/1", "PUT", { name: "Wohnanlage Seeweg Nord, Häuser A bis D (Sanierung 2026)" });
    await api("lines/1", "PUT", { name: "Strang Nord Wärme und Kaltwasser Obergeschosse", connection: "SIMULATOR", baud: 2400 });
    await api("meters", "POST", { address: "1", name: "Wohnung 1 Warmwasser Küche und Bad" });
    await api("lines/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, base, api }) {
    // mit Bereich in der Adresse: ohne springt die App beim Start selbst auf einen Bereich
    await page.goto(base + "/#/zaehler");
    const languages = await page.evaluate(() => Object.keys({ ...LANGUAGES, ...LANG_PREVIEW }));
    const empty = await api("gateways/1/lines", "POST", { name: "Ohne Anschluss" });
    const found = [];
    for (const line of [1, empty.id]) {
      await api(`lines/${line}/activate`, "POST");
      for (const lang of languages) {
        await page.addInitScript((l) => localStorage.setItem("lang", l), lang);
        for (const width of WIDTHS) {
          await page.setViewportSize({ width, height: 800 });
          await page.reload();
          for (const area of AREAS) {
            await page.goto(`${base}/#/${area}`);
            await page.waitForSelector(`section[data-area=${area}]`);
            await page.waitForTimeout(150);
            for (const p of await problems(page)) found.push(`Linie ${line}, ${lang}, ${width} px, ${area}: ${p}`);
          }
        }
      }
    }
    assert.deepEqual([...new Set(found)], []);
  },
};
