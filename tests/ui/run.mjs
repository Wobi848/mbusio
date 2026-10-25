// Browser-Tests der Oberfläche: je Test ein frischer mbusio (Simulator, eigener Datenordner).
//
//   cargo build && node tests/ui/run.mjs            alle Tests
//   node tests/ui/run.mjs liste                     nur Tests, deren Name „liste“ enthält
//
// Braucht Playwright (`npm i -D playwright && npx playwright install chromium`) oder den Pfad zu
// einem vorhandenen Paket in PLAYWRIGHT. MQTT-Test nur mit MBUSIO_TEST_BROKER=host:port.

import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const bin = path.join(root, "target/debug/mbusio" + (process.platform === "win32" ? ".exe" : ""));
const require = createRequire(import.meta.url);
let chromium;
try {
  ({ chromium } = require(process.env.PLAYWRIGHT || "playwright"));
} catch {
  console.error("Playwright fehlt: npm i -D playwright, oder PLAYWRIGHT=<pfad/zu/node_modules/playwright>");
  process.exit(2);
}
if (!fs.existsSync(bin)) {
  console.error(`${bin} fehlt: zuerst cargo build`);
  process.exit(2);
}

const filter = process.argv[2] || "";
const files = fs.readdirSync(here).filter((f) => f.endsWith(".test.mjs")).sort();
const tests = [];
for (const f of files) {
  const mod = await import(path.join(here, f));
  tests.push({ file: f, ...mod.default });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/// mbusio starten, warten bis die API antwortet
async function startServer(port, args = []) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mbusio-ui-"));
  const proc = spawn(bin, ["--http-port", String(port), "--no-browser", "--data-dir", dir, ...args], { stdio: "ignore" });
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(base + "/api/status")).ok) break;
    } catch {
      /* noch nicht da */
    }
    await sleep(100);
  }
  const api = async (p, method = "GET", body) => {
    const r = await fetch(base + "/api/" + p, {
      method,
      headers: { "content-type": "application/json", "x-lang": "de" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const data = await r.json().catch(() => null);
    if (!r.ok) throw new Error(`${method} ${p}: ${data?.error || r.status}`);
    return data;
  };
  return {
    base,
    api,
    stop() {
      proc.kill();
      fs.rmSync(dir, { recursive: true, force: true });
    },
  };
}

const browser = await chromium.launch();
let failed = 0;
// eigener Bereich mit MBUSIO_TEST_PORT, wenn mehrere Testläufe gleichzeitig laufen (z.B. in Arbeitsbäumen)
let port = Number(process.env.MBUSIO_TEST_PORT) || 18300;
for (const t of tests.filter((t) => t.name.includes(filter) || t.file.includes(filter))) {
  if (t.skip?.()) {
    console.log(`– ${t.name} (übersprungen)`);
    continue;
  }
  const server = await startServer(port++, t.args || []);
  const ctx = await browser.newContext({ viewport: { width: 1250, height: 1000 }, acceptDownloads: true });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  // Rückfragen (confirm/prompt) beantwortet der Test über page.answers, sonst OK
  page.answers = [];
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? page.answers.shift() : undefined));
  const t0 = Date.now();
  try {
    await page.addInitScript(() => localStorage.setItem("lang", "de"));
    if (t.setup) await t.setup(server.api);
    // bisherige Oberfläche (alt.html); die Tests der neuen öffnen „/“ selbst
    await page.goto(server.base + "/alt.html");
    await page.waitForFunction(() => document.querySelector("#h-project")?.textContent);
    await t.run({ page, api: server.api, base: server.base, ctx });
    if (errors.length) throw new Error("JavaScript-Fehler: " + errors.join(" | "));
    console.log(`✓ ${t.name} (${((Date.now() - t0) / 1000).toFixed(1)} s)`);
  } catch (e) {
    failed++;
    console.log(`✗ ${t.name}\n    ${String(e.stack || e).split("\n").slice(0, 4).join("\n    ")}`);
    await page.screenshot({ path: path.join(os.tmpdir(), `mbusio-ui-${t.file}.png`) }).catch(() => {});
  } finally {
    await ctx.close();
    server.stop();
  }
}
await browser.close();
console.log(failed ? `${failed} fehlgeschlagen` : "alle Tests bestanden");
process.exit(failed ? 1 : 0);
