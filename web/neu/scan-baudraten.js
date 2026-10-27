// Bereich Scan: Menü der Baudraten mit „Prüfen, welche antworten“. `menu` ist das Menü-Element,
// `view` die Einstellungen der Linie (bauds = gewählte, baudCheck = letztes Ergebnis).

import { html, text, explain } from "./view.js";
import { report } from "./data.js";

let checking = null; // Beginn der Prüfung (ms), solange sie läuft

export function fillBaudMenu(menu, view, busBaud) {
  const on = new Set(view.bauds || [busBaud]);
  html(
    menu,
    `<div class="bauds">${BAUDS.map((b) => `<label class="baud"><input type="checkbox" data-baud value="${b}"${on.has(b) ? " checked" : ""}>${b}</label>`).join("")}</div>
    <div class="row end"><span class="sub" data-baud-result></span><button type="button" class="tonal" data-baud-check>${esc(t("sc_baud_check"))}</button></div>
    ${explain(t("sc_explain_bauds"))}`
  );
  showCheck(menu, view);
}

/// Zustand der Prüfung: läuft (Punkt, Sekunden seit Beginn), sonst das Ergebnis. Keine Schätzung:
/// die Dauer hängt vom Wandler und vom Betriebssystem ab (gemessen 8 s bis 2 min)
function showCheck(menu, view) {
  const out = menu.querySelector("[data-baud-result]");
  const go = menu.querySelector("[data-baud-check]");
  if (!out) return;
  go.disabled = !!checking;
  if (!checking) return text(out, view.baudCheck ? t("sc_baud_found", view.baudCheck) : "");
  const s = Math.round((Date.now() - checking) / 1000);
  html(out, `<span class="dot busy"></span> ${esc(t("sc_baud_checking", s))}`);
}

/// Welche Baudraten antworten? Ergebnis gleich als Auswahl übernehmen
export async function checkBauds(menu, view, busBaud) {
  checking = Date.now();
  showCheck(menu, view);
  const tick = setInterval(() => showCheck(menu, view), 1000);
  try {
    const r = await request("bus-bauds", { method: "POST", body: { bauds: [] } });
    view.baudCheck = r.bauds;
    if (r.bauds.length) view.bauds = r.bauds;
  } catch (e) {
    report(e);
  } finally {
    clearInterval(tick);
    checking = null;
  }
  fillBaudMenu(menu, view, busBaud);
}
