// Bereich Zähler: Verlauf eines Zählers als Diagramm im Detailbereich (Reihe wählen, CSV)

import { report } from "./data.js";

let hist = null; // { id, points, keys (Schlüssel → Reihe), key }

export function historyHtml(m) {
  if (hist?.id !== m.id) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">…</p>`;
  if (hist.points.length < 2) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">${esc(t(hist.points.length ? "history_few" : "history_none"))}</p>`;
  return `<h3>${esc(t("zl_history"))} <span class="sub">${esc(t("history_first"))} ${esc(slotText(hist.points[0].timestamp))}</span></h3>
    <div class="actions"><select data-hist-key aria-label="${esc(t("history_series"))}">${[...hist.keys]
      .map(([k, v]) => `<option value="${esc(k)}"${k === hist.key ? " selected" : ""}>${esc(seriesName(v))}</option>`)
      .join("")}</select><button type="button" class="quiet" data-hist-csv>${esc(t("history_csv"))}</button></div>
    <div class="chart" data-chart></div>`;
}

async function loadHistory(m, rerender) {
  hist = { id: m.id, points: [], keys: new Map(), key: null };
  try {
    const r = await request(`meters/${m.id}/history`);
    const keys = new Map();
    for (const p of r.points) for (const v of p.values) if (!keys.has(v.key)) keys.set(v.key, v);
    const main = [...keys.keys()].find((k) => keys.get(k).quantity === m.last_quantity) ?? [...keys.keys()][0] ?? null;
    hist = { id: m.id, points: r.points, keys, key: main };
  } catch (e) {
    report(e);
  }
  rerender();
}

/// Diagramm zeichnen, wenn die Fläche neu ist (html() schreibt nur bei Änderung); `rerender` nach
/// dem Laden
export function drawHistory(box, m, rerender) {
  if (hist?.id !== m.id) return loadHistory(m, rerender);
  const el = box.querySelector("[data-chart]");
  if (!el || el.childElementCount || !hist.key) return;
  drawChart(el, plotSeries(hist.points, hist.key), seriesName(hist.keys.get(hist.key)));
}

export function setHistoryKey(key) {
  hist.key = key;
}

export function historyCsv(m) {
  downloadSeries(plotSeries(hist.points, hist.key), hist.keys.get(hist.key), m);
}

/// nach einem Zählertausch: Verlauf neu laden
export function forgetHistory() {
  hist = null;
}
