// Protokoll des Busses (Telegramme TX/RX, Meldungen) an einer Stelle: einmal abgefragt, gezeigt in
// Werkzeuge und in der Bus-Konsole der Statusleiste. Zeilen stehen in state.log.

import { state, set } from "./store.js";

const EVERY = 1000;
const KEEP = 1000;
let seq = 0;

/// Abfragen, solange `needed()` gilt (Werkzeuge offen oder Konsole offen) und das Fenster sichtbar ist
export function follow(needed) {
  setInterval(() => needed() && !document.hidden && poll().catch(() => {}), EVERY);
}

export async function poll() {
  const entries = await request("log?since=" + seq);
  if (!entries.length) return;
  seq = entries[entries.length - 1].seq;
  set({ log: [...(state.log || []), ...entries].slice(-KEEP) });
}

export async function clear() {
  await request("log", { method: "DELETE" });
  set({ log: [] });
}

/// Eine Zeile; Telegramme tragen ihre Bytes (Klick → analysieren)
export function logRow(e) {
  const frame = e.dir === "TX" || e.dir === "RX";
  return `<div class="l d-${esc(e.dir)}"${frame ? ` data-hex="${esc(e.text)}" title="${esc(t("log_click"))}"` : ""}><span class="sub">${esc(e.time)}</span><b>${esc(e.dir)}</b><span>${esc(e.text)}</span></div>`;
}
