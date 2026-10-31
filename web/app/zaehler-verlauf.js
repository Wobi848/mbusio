// Bereich Zähler: Verlauf eines Zählers im Detailbereich (Reihe wählen, Diagramm oder Tabelle,
// Zusammenfassung erste/letzte Auslesung mit Differenz und Verbrauch pro Tag, CSV)

import { report } from "./data.js";

let hist = null; // { id, points, keys (Schlüssel → Reihe), key, view: "chart" | "table" }

const num = (x) => new Intl.NumberFormat(langFormat().locale, { maximumFractionDigits: 3 }).format(x);
const series = () => plotSeries(hist.points, hist.key);

export function historyHtml(m) {
  if (hist?.id !== m.id) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">…</p>`;
  if (!hist.points.length) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">${esc(t("history_none"))}</p>`;
  const s = series();
  const table = hist.view === "table" || s.length < 2;
  return `<h3>${esc(t("zl_history"))}</h3>
    <div class="actions hbar"><select data-hist-key aria-label="${esc(t("history_series"))}">${[...hist.keys]
      .map(([k, v]) => `<option value="${esc(k)}"${k === hist.key ? " selected" : ""}>${esc(seriesName(v))}</option>`)
      .join("")}</select>
      <span class="seg" role="group">${["chart", "table"]
        .map((v) => `<button type="button" data-hist-view="${v}" aria-pressed="${hist.view === v}">${esc(t(`history_${v}`))}</button>`)
        .join("")}</span>
      <button type="button" class="quiet" data-hist-csv>${esc(t("history_csv"))}</button></div>
    ${summaryHtml(s)}${s.length < 2 ? `<p class="sub">${esc(t("history_few"))}</p>` : ""}
    ${table ? tableHtml(s) : `<div class="chart" data-chart></div>`}`;
}

/// Erste und letzte Auslesung, Differenz, bei mindestens einem halben Tag auch pro Tag
function summaryHtml(s) {
  if (s.length < 2) return "";
  const a = s[0];
  const b = s[s.length - 1];
  const diff = b.value - a.value;
  const days = (b.time - a.time) / 864e5;
  const u = esc(b.unit);
  return `<dl class="hsum">
    <div><dt>${esc(t("history_first"))}</dt><dd><b>${esc(a.text)} ${u}</b><span class="sub">${esc(slotText(a.ts))}</span></dd></div>
    <div><dt>${esc(t("history_last"))}</dt><dd><b>${esc(b.text)} ${u}</b><span class="sub">${esc(slotText(b.ts))}</span></dd></div>
    <div><dt>${esc(t("history_diff"))}</dt><dd><b>${num(diff)} ${u}</b>${days >= 0.5 ? `<span class="sub">${num(diff / days)} ${u} ${esc(t("history_per_day"))}</span>` : ""}</dd></div>
  </dl>`;
}

/// Neueste zuerst, mit Differenz zur vorherigen Auslesung; unsichere Uhrzeit markiert
function tableHtml(s) {
  const rows = s
    .map((p, i) => {
      const d = i ? p.value - s[i - 1].value : null;
      const unsure = p.unsure ? ` <span class="chip warn" title="${esc(t("reading_time_unsure_title"))}">${esc(t("reading_time_unsure"))}</span>` : "";
      return `<tr><td>${esc(slotText(p.ts))}${unsure}</td><td class="num">${esc(p.text)} ${esc(p.unit)}</td><td class="num">${d === null ? "" : (d > 0 ? "+" : "") + num(d)}</td></tr>`;
    })
    .reverse()
    .join("");
  return `<div class="htable"><table><thead><tr><th>${esc(t("th_time"))}</th><th class="num">${esc(t("history_series"))}</th><th class="num">${esc(
    t("th_delta")
  )}</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

async function loadHistory(m, rerender) {
  const view = hist?.view || "chart";
  hist = { id: m.id, points: [], keys: new Map(), key: null, view };
  try {
    const r = await request(`meters/${m.id}/history`);
    const keys = new Map();
    for (const p of r.points) for (const v of p.values) if (!keys.has(v.key)) keys.set(v.key, v);
    const main = [...keys.keys()].find((k) => keys.get(k).quantity === m.last_quantity) ?? [...keys.keys()][0] ?? null;
    hist = { id: m.id, points: r.points, keys, key: main, view };
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
  drawChart(el, series(), seriesName(hist.keys.get(hist.key)));
}

export function setHistoryKey(key) {
  hist.key = key;
}

export function setHistoryView(view) {
  hist.view = view;
}

export function historyCsv(m) {
  downloadSeries(series(), hist.keys.get(hist.key), m);
}

/// nach einem Zählertausch: Verlauf neu laden
export function forgetHistory() {
  hist = null;
}
