// Gemeinsame Schritte der Browser-Tests
import assert from "node:assert/strict";
export { assert };

/// Linie 1 mit einer Simulation verbinden und aktiv machen. `timeout_ms`: kurze Antwortzeit für
/// lange Scans (der Simulator antwortet sofort; 0 = automatisch wie im Feld)
export async function simLine(api, connection = "SIMULATOR", baud = 2400, timeout_ms = 0) {
  await api("lines/1", "PUT", { name: "Haus A", connection, baud, timeout_ms });
  await api("lines/1/activate", "POST");
}

/// Scan im Bereich Scan starten (Taste S) und warten, bis er fertig ist: erst läuft er (Knopf
/// „Abbrechen“), dann nicht mehr
export async function scan(page, timeout = 120000) {
  await page.keyboard.press("s");
  await scanDone(page, timeout);
}

export async function scanDone(page, timeout = 120000) {
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent === "Abbrechen", null, { timeout: 10000 }).catch(() => {});
  await page.waitForFunction(() => document.querySelector("[data-start]")?.textContent !== "Abbrechen", null, { timeout });
}
