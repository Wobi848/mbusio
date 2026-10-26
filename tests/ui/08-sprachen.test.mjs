import { assert, simLine } from "./helpers.mjs";

// jeder Bereich in beiden Sprachen ohne fehlende Texte (Schlüssel statt Text),
// auf Englisch ohne deutsche Wörter – auch in Tooltips, Platzhaltern und Details

const AREAS = ["scan", "zaehler", "weitergabe", "anlage", "werkzeuge", "simulator"];
const KEY = /^[a-z]+(_[a-z0-9]+)+$/;
const GERMAN = /[äöüÄÖÜß]|\b(und|nicht|oder|mit|der|die|das|ein|eine|keine?|noch|auf|für|wählen|speichern|Linie|Zähler)\b/;
// Daten aus der Datenbank (Projekt- und Linienname) sind keine Oberflächentexte
const DATA = /Schnellscan|Haus A|Gateway 1|Whg \d/g;

/// sichtbare Texte, Tooltips, Platzhalter und Beschriftungen im Arbeitsbereich, Kopf- und Statusleiste
const visibleTexts = (page) =>
  page.evaluate(() => {
    const out = [];
    for (const el of document.querySelectorAll("body *:not(script):not(style)")) {
      if (!el.getClientRects().length) continue;
      if (![...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) {
        if (!el.title && !el.placeholder && !el.getAttribute("aria-label")) continue;
      } else out.push([...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join(" ").trim());
      for (const a of ["title", "placeholder", "aria-label"]) if (el.getAttribute(a)) out.push(el.getAttribute(a));
    }
    return out.filter(Boolean);
  });

async function check(page, lang, where) {
  const texts = await visibleTexts(page);
  assert.deepEqual(texts.filter((s) => KEY.test(s)), [], `fehlende Texte (${lang}, ${where})`);
  if (lang === "en") {
    const german = texts.filter((s) => GERMAN.test(s.replace(DATA, "")));
    assert.deepEqual(german, [], `deutscher Text auf Englisch (${where})`);
  }
}

/// weitere Ansichten je Bereich: Klicks nacheinander, nach jedem wird geprüft
const MORE = {
  scan: async (page, step) => {
    await page.fill("[data-to]", "3");
    await page.press("[data-to]", "Tab");
    await page.keyboard.press("s");
    await page.waitForFunction(() => document.querySelectorAll("section[data-area=scan] [role=option]").length && !document.querySelector("[data-start]").classList.contains("quiet"), null, { timeout: 60000 });
    await step("section[data-area=scan] [role=option]");
  },
  zaehler: async (page, step) => {
    for (const panel of ["import", "names"]) {
      await step("section[data-area=zaehler] [data-more]");
      await step(`section[data-area=zaehler] [data-panel=${panel}]`);
    }
    await step("section[data-area=zaehler] [role=option]");
    await step("section[data-area=zaehler] [data-exchange]");
  },
  weitergabe: async (page, step) => {
    await step("[data-out-settings=mqtt]");
    await step("[data-out-settings=modbus]");
  },
  anlage: async (page, step) => {
    for (const i of [1, 2]) await step(`section[data-area=anlage] [role=option] >> nth=${i}`);
  },
};

export default {
  name: "Sprachen (alle Bereiche ohne fehlende Texte, Englisch ohne Deutsch)",
  async setup(api) {
    await simLine(api, "SIMULATOR:ok10");
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
  },
  async run({ page, base }) {
    for (const lang of ["en", "de"]) {
      // das Testgerüst setzt bei jedem Laden Deutsch; ein späteres Init-Skript gewinnt
      await page.addInitScript((l) => localStorage.setItem("lang", l), lang);
      for (const area of AREAS) {
        await page.goto(`${base}/#/${area}`);
        await page.waitForSelector(`.tabs [data-area=${area}][aria-current=page]`);
        await page.waitForTimeout(600);
        await check(page, lang, area);
        // erste Zeile: Details rechts
        const row = page.locator(`section[data-area=${area}] [data-list] [role=option]`).first();
        if (await row.count()) {
          await row.click();
          await page.waitForTimeout(300);
          await check(page, lang, area + " Details");
        }
        await MORE[area]?.(page, async (sel) => {
          await page.click(sel);
          await page.waitForTimeout(300);
          await check(page, lang, `${area} ${sel}`);
        });
      }
      // Einstellungen und Tastenübersicht
      await page.click("#settings");
      await check(page, lang, "Einstellungen");
      await page.keyboard.press("Escape");
      await page.keyboard.press("?");
      await check(page, lang, "Tasten");
      await page.keyboard.press("Escape");
    }
  },
};
