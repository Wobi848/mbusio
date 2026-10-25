import net from "node:net";
import { assert, simLine, tab } from "./helpers.mjs";

/// Register lesen wie ein Leitsystem (Funktion 04)
function readRegisters(port, start, count) {
  return new Promise((resolve, reject) => {
    const s = net.connect(port, "127.0.0.1", () => {
      const req = Buffer.alloc(12);
      req.writeUInt16BE(1, 0); // Transaktion
      req.writeUInt16BE(6, 4); // Länge
      req[6] = 1; // Unit-ID
      req[7] = 0x04;
      req.writeUInt16BE(start, 8);
      req.writeUInt16BE(count, 10);
      s.write(req);
    });
    s.on("data", (b) => {
      s.end();
      const regs = [];
      for (let i = 0; i < b[8] / 2; i++) regs.push(b.readUInt16BE(9 + 2 * i));
      resolve(regs);
    });
    s.on("error", reject);
  });
}

export default {
  name: "Modbus: Server einrichten, Register vergeben, Wert über Modbus lesen",
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
    // Werte nur aus Linien im Dauerbetrieb
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, api }) {
    const port = 16000 + (process.pid % 1000);
    await tab(page, "transfer");
    await page.click("[data-out-settings=modbus]");
    const f = "[data-out-form=modbus]";
    await page.check(`${f} [name=enabled]`);
    await page.fill(`${f} [name=bind]`, "127.0.0.1");
    await page.fill(`${f} [name=port]`, String(port));
    await page.click(`${f} [type=submit]`);
    await page.waitForFunction(() => document.querySelector("#tr-outputs")?.textContent.includes("Hört auf"), null, { timeout: 15000 });
    // Häkchen: Register aus der Nummer des Datenpunkts (1 → 0), Block im Tooltip
    await page.locator("[data-dp-out=modbus]").first().check();
    await page.waitForFunction(() => [...document.querySelectorAll("#dp-table .out-addr")].some((x) => x.title === "Register 0–9"));
    // wie ein Leitsystem lesen: Wert als float64, Qualität gut
    await api("meters/1/read", "POST");
    let regs = [];
    for (let i = 0; i < 30; i++) {
      regs = await readRegisters(port, 0, 10);
      if (regs[0] || regs[1]) break;
      await page.waitForTimeout(200);
    }
    const b = Buffer.alloc(8);
    regs.slice(0, 4).forEach((r, i) => b.writeUInt16BE(r, 2 * i));
    assert.ok(b.readDoubleBE(0) > 123456, `Wert ${b.readDoubleBE(0)}`);
    assert.equal(regs[8], 0, "Qualität gut");
    // Zustand: Lebenszeichen, keine Störungen
    const status = await readRegisters(port, 60000, 3);
    assert.equal(status[1], 0);
    // Liste für den Integrator mit Register
    const list = await api("projects/1/outputs/modbus/list?format=json");
    assert.equal(list.datapoints[0].address, "0");
    assert.equal(list.modbus.word_order, "ABCD");
  },
};
