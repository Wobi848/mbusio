import net from "node:net";
import { assert, simLine } from "./helpers.mjs";

// Modbus-Server einrichten und einschalten, Datenpunkt weitergeben, Wert wie ein
// Leitsystem lesen

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

const detail = "section[data-area=weitergabe] [data-detail]";

export default {
  name: "Modbus (einrichten, Datenpunkt weitergeben, Register lesen)",
  async setup(api) {
    await simLine(api);
    await api("meters", "POST", { address: "1", name: "Whg 1" });
    await api("meters/1/read", "POST");
    await api("meters/1/datapoints", "PUT", { keys: ["instantaneous|0|0|0|energy|kWh"] });
    // Werte nur aus Linien im Dauerbetrieb
    await api("projects/1/plan", "PUT", { plan: "daily 00:05" });
    await api("lines/1/operation", "PUT", { on: true });
  },
  async run({ page, base, api }) {
    const port = 16000 + (process.pid % 1000);
    await page.goto(base + "/#/weitergabe");
    await page.click("[data-out-settings=modbus]");
    await page.fill(`${detail} [name=bind]`, "127.0.0.1");
    await page.fill(`${detail} [name=port]`, String(port));
    await page.click(`${detail} [type=submit]`);
    await page.waitForSelector(`${detail} [data-out-form]`, { state: "detached" });
    await page.click("[data-out-toggle=modbus]");
    await page.waitForFunction(() => document.querySelector("[data-outputs]")?.textContent.includes("127.0.0.1"), null, { timeout: 15000 });

    // Datenpunkt an Modbus: Register aus der Nummer des Datenpunkts (1 → 0)
    await page.focus("section[data-area=weitergabe] [data-list]");
    await page.keyboard.press("Control+a");
    await page.click("[data-bulk='out:modbus:on']");
    let list = null;
    for (let i = 0; i < 30 && !list?.datapoints.length; i++) {
      list = await api("projects/1/outputs/modbus/list?format=json");
      await page.waitForTimeout(100);
    }
    assert.equal(list.datapoints[0].address, "0");
    assert.equal(list.modbus.word_order, "ABCD");

    // wie ein Leitsystem lesen: Wert als float64, Qualität gut; Zustand ohne Störungen
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
    assert.equal((await readRegisters(port, 60000, 3))[1], 0);
  },
};
