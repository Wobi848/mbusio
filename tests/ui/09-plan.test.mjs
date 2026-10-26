import { assert, simLine } from "./helpers.mjs";

// Ausleseplan in der Anlage (Projekt, Linie, Uhrzeit 24 h mit Kurzformen), im
// Zähler geerbt oder aus; ohne Zeitquelle die Uhrzeit des Browsers übernehmen

const tree = "section[data-area=anlage] [data-list] [role=option]";
const meterRows = "section[data-area=zaehler] [data-list] [role=option]";

export default {
  name: "Ausleseplan (Projekt, Linie, Uhrzeit, Zähler aus, Zeit übernehmen)",
  args: ["--untrusted-clock"],
  async setup(api) {
    await simLine(api);
    for (const a of ["1", "2", "3"]) await api("meters", "POST", { address: a, name: "Z" + a });
  },
  async run({ page, base, api }) {
    const plan = async () => (await api("projects")).projects.flatMap((p) => p.gateways.flatMap((g) => g.lines))[0].plan_info;
    await page.goto(base + "/#/anlage");
    await page.locator(tree).first().click();

    // Projekt täglich; Ungültiges abgelehnt und markiert, Kurzformen ergänzt
    await page.selectOption("[data-plan=project]", "daily");
    const time = "[data-plan-time=project]";
    await page.fill(time, "25:00");
    await page.press(time, "Tab");
    await page.waitForFunction(() => document.querySelector("#toast")?.textContent.includes("HH:MM"));
    assert.ok(await page.$eval(time, (el) => el.classList.contains("invalid")));
    await page.fill(time, "1730");
    await page.press(time, "Tab");
    await page.waitForFunction((sel) => document.querySelector(sel).value === "17:30", time);
    await page.fill(time, "5");
    await page.press(time, "Tab");
    await page.waitForFunction((sel) => document.querySelector(sel).value === "05:00", time);
    assert.equal((await plan()).effective, "daily 05:00");

    // Linie: eigener Plan
    await page.locator(tree).nth(2).click();
    await page.selectOption("[data-plan=line]", "15min");
    await page.waitForFunction(() => document.querySelector("[data-plan=line]")?.value === "15min");
    for (let i = 0; i < 20 && (await plan()).effective !== "15min"; i++) await page.waitForTimeout(100);
    assert.equal((await plan()).effective, "15min");

    // Zähler: geerbter Plan in der Liste, einen aus dem Plan nehmen
    await page.goto(base + "/#/zaehler");
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 3, meterRows);
    await page.waitForFunction((sel) => document.querySelector(sel)?.textContent.includes("alle 15 min"), meterRows);
    await page.locator(meterRows).nth(1).click();
    await page.selectOption("section[data-area=zaehler] [data-detail] [data-plan]", "off");
    await page.waitForFunction((sel) => /\baus\b/.test(document.querySelectorAll(sel)[1]?.textContent), meterRows);
    assert.equal((await api("meters")).find((m) => m.address === "2").plan, "off");

    // ohne Zeitquelle: Anlage zeigt „Uhrzeit unsicher“ mit Knopf zum Übernehmen
    await api("lines/1/operation", "PUT", { on: true });
    await page.goto(base + "/#/anlage");
    await page.waitForSelector("[data-take-time]", { timeout: 15000 });
    await page.click("[data-take-time]");
    await page.waitForSelector("[data-take-time]", { state: "detached", timeout: 10000 });
  },
};
