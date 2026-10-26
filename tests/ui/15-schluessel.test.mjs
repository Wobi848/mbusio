import { assert } from "./helpers.mjs";

// mit Zugriffsschlüssel (--token): Feld über der Seite statt Browser-Fenster,
// falscher Schlüssel wird gesagt, der richtige bleibt gespeichert

export default {
  name: "Zugriffsschlüssel (Feld, falsch, gespeichert)",
  args: ["--token", "geheim"],
  async run({ page, base }) {
    await page.goto(base + "/");
    await page.waitForSelector(".token-gate input");
    // viele Anfragen gleichzeitig abgewiesen: trotzdem nur ein Feld
    assert.equal(await page.locator(".token-gate").count(), 1);
    await page.fill(".token-gate input", "falsch");
    await page.press(".token-gate input", "Enter");
    await page.waitForFunction(() => document.querySelector(".token-gate")?.textContent.includes("falsch"));
    await page.fill(".token-gate input", "geheim");
    await page.click(".token-gate button");
    await page.waitForFunction(() => !document.querySelector(".token-gate") && document.querySelector("#project")?.textContent.trim());
    // beim nächsten Öffnen nicht mehr gefragt
    await page.reload();
    await page.waitForFunction(() => document.querySelector("#project")?.textContent.trim());
    assert.equal(await page.locator(".token-gate").count(), 0);
  },
};
