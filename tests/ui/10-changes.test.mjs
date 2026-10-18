import { assert, primaryScan } from "./helpers.mjs";

/// Eigene Simulation aus der Vorlage „alles i.O.“, auf Linie 1
async function ownSim(api) {
  const r = await api("sim", "POST", { name: "Vergleich", from: "ok10" });
  await api(`sim/${r.key}/use`, "POST");
  return r.key;
}

export default {
  name: "Vergleich mit dem letzten Scan: neu, fehlt, geändert",
  async run({ page, api }) {
    const key = await ownSim(api);
    await page.reload();
    const changes = () => page.textContent("#hits-changes");
    // erster Scan: nichts zu vergleichen
    assert.equal(await primaryScan(page, 0, 12), 10);
    assert.ok(await page.isHidden("#hits-changes"));
    // gleicher Bus, gleicher Bereich: keine Änderung
    await primaryScan(page, 0, 12);
    assert.match(await changes(), /Keine Änderung seit dem letzten Scan/);
    // am Bus: ein Zähler weg, einer hat eine andere PA
    const sim = (await api("sim")).scenarios.find((s) => s.key === key);
    const meters = sim.meters.slice(1).map((m, i) => (i === 0 ? { ...m, primary: 12 } : m));
    await api(`sim/${key}`, "PUT", { name: sim.name, meters });
    await primaryScan(page, 0, 12);
    const text = await changes();
    assert.match(text, /1 fehlt/, text);
    assert.match(text, /1 geändert/, text);
    assert.equal(await page.locator("#hits tr.missing").count(), 1);
    assert.match(await page.textContent("#hits tr.chg"), /PA \d+ → 12/);
    // anderer Bereich: kein Vergleich
    await primaryScan(page, 0, 20);
    assert.ok(await page.isHidden("#hits-changes"));
  },
};
