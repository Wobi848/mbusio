"use strict";

// Verlauf eines Zählers: Diagramm und Tabelle

// ------------------------------------------------------------ Verlauf eines Zählers

/// { meter, points, keys (Reihen nach Messgrösse/Einheit/Tarif), key (gewählte Reihe), view }
let hist = null;

async function openHistory(m) {
  const r = await api(`meters/${m.id}/history`);
  const keys = new Map();
  for (const p of r.points) for (const v of p.values) if (!keys.has(v.key)) keys.set(v.key, v);
  // gleicher Zähler nochmal: gewählte Reihe und Ansicht behalten; sonst Hauptwert der Liste zuerst
  const same = hist?.meter.id === m.id ? hist : null;
  const main = [...keys.keys()].find((k) => keys.get(k).quantity === m.last_quantity) ?? [...keys.keys()][0] ?? null;
  hist = {
    meter: m,
    points: r.points,
    keys,
    key: same?.key && keys.has(same.key) ? same.key : main,
    view: same?.view || "chart",
  };
  renderHistory();
  $("history").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

const historySeries = () => plotSeries(hist.points, hist.key);

function renderHistory() {
  const el = $("history");
  if (!hist) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  const m = hist.meter;
  const name = m.name || m.meter_id || m.address;
  const meterTitle = ui.meter(meterBlock(m));
  const series = hist.key ? historySeries() : [];
  const opts = [...hist.keys]
    .map(([k, v]) => `<option value="${esc(k)}" ${k === hist.key ? "selected" : ""}>${esc(seriesName(v))}</option>`)
    .join("");
  let sum = "";
  if (series.length >= 2) {
    const a = series[0];
    const b = series[series.length - 1];
    const diff = b.value - a.value;
    const days = (b.time - a.time) / 864e5;
    const u = esc(b.unit);
    sum = `<div class="history-sum">
      <span>${esc(t("history_first"))}: <b>${esc(a.text)} ${u}</b> · ${esc(a.ts)}</span>
      <span>${esc(t("history_last"))}: <b>${esc(b.text)} ${u}</b> · ${esc(b.ts)}</span>
      <span>${esc(t("history_diff"))}: <b>${formatNum(diff)} ${u}</b>${
        days >= 0.5 ? ` · <b>${formatNum(diff / days)} ${u}</b> ${esc(t("history_per_day"))}` : ""
      }</span></div>`;
  }
  el.innerHTML = `<div class="history-head">
      <h3>${esc(t("history"))}</h3>${meterTitle}
      ${hist.keys.size ? `<label class="inline"><span>${esc(t("history_series"))}</span> <select id="hist-key">${opts}</select></label>` : ""}
      <span class="spacer"></span>
      <div class="seg" role="group"><button data-hview="chart" class="${hist.view === "chart" ? "active" : ""}">${esc(t("history_chart"))}</button><button data-hview="table" class="${hist.view === "table" ? "active" : ""}">${esc(t("history_table"))}</button></div>
      <button class="small quiet" data-hcsv ${series.length ? "" : "disabled"}>${esc(t("history_csv"))}</button>
      <button class="small quiet" data-hclose>${esc(t("history_close"))}</button>
    </div>${sum}<div id="hist-body"></div>`;
  const body = $("hist-body");
  if (!series.length) body.innerHTML = `<p class="hint">${esc(t("history_none"))}</p>`;
  else if (hist.view === "table") body.innerHTML = historyTable(series);
  else if (series.length < 2) body.innerHTML = `<p class="hint">${esc(t("history_few"))}</p>` + historyTable(series);
  else drawChart(body, series, name);
}

/// Tabelle, neueste zuerst, mit Differenz zur vorherigen Auslesung
function historyTable(series) {
  const rows = series
    .map((p, i) => {
      const d = i ? p.value - series[i - 1].value : null;
      const unsure = p.unsure
        ? " " + ui.chip("warn", t("reading_time_unsure"), t("reading_time_unsure_title"))
        : "";
      return `<tr><td class="mono">${esc(p.ts)}${unsure}</td><td class="num">${esc(p.text)} ${esc(p.unit)}</td><td class="num">${
        d === null ? "" : (d > 0 ? "+" : "") + formatNum(d)
      }</td></tr>`;
    })
    .reverse()
    .join("");
  return `<div class="table-wrap"><table><thead><tr><th>${esc(t("th_time"))}</th><th class="num">${esc(
    t("history_series")
  )}</th><th class="num">${esc(t("th_delta"))}</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

/// Runde Achsenschritte: 1, 2 oder 5 mal Zehnerpotenz
$("history").addEventListener("change", (ev) => {
  if (ev.target.id !== "hist-key") return;
  hist.key = ev.target.value;
  renderHistory();
});

$("history").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.hview) {
    hist.view = b.dataset.hview;
    renderHistory();
  } else if (b.dataset.hclose !== undefined) {
    hist = null;
    renderHistory();
  } else if (b.dataset.hcsv !== undefined) {
    downloadSeries(historySeries(), hist.keys.get(hist.key), hist.meter);
  }
});

let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => hist?.view === "chart" && renderHistory(), 150);
});
