import { assert, scanDone, simLine, tab } from "./helpers.mjs";

const hitIds = (page) =>
  page.$$eval("#hits tbody tr:not(.issue-strip):not(.missing) .mtr-info", (l) => l.map((e) => (e.textContent.match(/ID (\d{8})/) || [])[1]).filter(Boolean));

export default {
  name: "Sekundärscan: ID-Anfang eingrenzen, nach Abbruch weitersuchen",
  setup: (api) => simLine(api, "SIMULATOR:mfh30"),
  async run({ page, api }) {
    const ids = (await api("sim")).scenarios.find((s) => s.key === "mfh30").meters.map((m) => m.id);
    await tab(page, "scan");
    await page.click("[data-kind=secondary]");
    // nur IDs mit diesem Anfang
    const prefix = ids[0].slice(0, 3);
    await page.fill("#scan-prefix", prefix);
    await page.click("#scan-secondary");
    await scanDone(page);
    const some = await hitIds(page);
    assert.deepEqual(some.sort(), ids.filter((id) => id.startsWith(prefix)).sort());
    // ganzer Bus, mittendrin abbrechen, dann weitersuchen: am Ende alle, keiner doppelt
    await page.fill("#scan-prefix", "");
    await page.click("#scan-secondary");
    await page.waitForFunction(() => document.querySelectorAll("#hits tbody tr .mtr-info").length >= 5, null, { timeout: 60000 });
    await page.click("#scan-cancel");
    await scanDone(page);
    assert.ok(await page.isVisible("#scan-resume"), "Weitersuchen nach Abbruch");
    const before = (await hitIds(page)).length;
    assert.ok(before < ids.length, `abgebrochen nach ${before}`);
    await page.click("#scan-resume");
    await scanDone(page);
    const all = await hitIds(page);
    assert.deepEqual(all.sort(), [...ids].sort());
    assert.ok(await page.isHidden("#scan-resume"));
  },
};
