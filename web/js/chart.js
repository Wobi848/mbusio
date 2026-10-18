"use strict";

// Verlauf eines Zählers: Diagramm und Tabelle

// ------------------------------------------------------------ Verlauf eines Zählers

/// { meter, points, keys (Reihen nach Messgrösse/Einheit/Tarif), key (gewählte Reihe), view }
let hist = null;

const parseTime = (s) => new Date(String(s).replace(" ", "T"));

function formatNum(x, maxDigits = 3) {
  return new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { maximumFractionDigits: maxDigits }).format(x);
}

function seriesName(v) {
  return `${label(v.quantity)}${v.tariff ? ` · ${t("tariff")} ${v.tariff}` : ""}${v.unit ? ` (${v.unit})` : ""}`;
}

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

function historySeries() {
  return hist.points
    .map((p) => {
      const v = p.values.find((x) => x.key === hist.key);
      return v
        ? { time: parseTime(p.timestamp), ts: p.timestamp, unsure: p.time_unsure, value: v.value, text: v.text, unit: v.unit }
        : null;
    })
    .filter(Boolean);
}

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
function niceTicks(lo, hi, n) {
  const raw = (hi - lo) / n;
  const p = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((k) => k * p).find((s) => s >= raw);
  const out = [];
  for (let v = Math.floor(lo / step) * step; v <= hi + step / 2; v += step) out.push(Number(v.toPrecision(12)));
  if (out[out.length - 1] < hi) out.push(Number((out[out.length - 1] + step).toPrecision(12)));
  return out;
}

/// Achsenbeschriftung mit so vielen Stellen, wie der Abstand der Linien braucht (sonst stehen bei
/// kleinen Änderungen grosser Zählerstände zweimal dieselben Zahlen da)
function axisNum(v, step) {
  const digits = Math.min(6, Math.max(0, -Math.floor(Math.log10(step || 1) + 1e-9)));
  return Math.abs(v) >= 10000 && step >= 1000
    ? new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { notation: "compact", maximumFractionDigits: 1 }).format(v)
    : new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(v);
}

function timeLabel(d, span) {
  const loc = lang === "de" ? "de-CH" : "en-GB";
  if (span < 2 * 36e5) return d.toLocaleTimeString(loc, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  if (span < 2 * 864e5) return d.toLocaleTimeString(loc, { hour: "2-digit", minute: "2-digit" });
  return d.toLocaleDateString(loc, { day: "2-digit", month: "2-digit" });
}

/// Eine Reihe als Linie: 2px, dünnes Raster, Endwert beschriftet, Fadenkreuz mit Tooltip
function drawChart(box, pts, name) {
  const W = Math.max(box.clientWidth || 600, 280);
  const H = 240;
  const t0 = +pts[0].time;
  const t1 = +pts[pts.length - 1].time;
  const vals = pts.map((p) => p.value);
  let lo = Math.min(...vals);
  let hi = Math.max(...vals);
  if (lo === hi) {
    lo -= Math.abs(lo) * 0.01 || 1;
    hi += Math.abs(hi) * 0.01 || 1;
  }
  const yt = niceTicks(lo, hi, 4);
  const ystep = yt.length > 1 ? yt[1] - yt[0] : 1;
  lo = yt[0];
  hi = yt[yt.length - 1];
  const last = pts[pts.length - 1];
  const endText = `${last.text} ${last.unit}`;
  // Ränder aus der Länge der Beschriftungen, damit nichts abgeschnitten wird (Schätzung je Zeichen)
  const mg = {
    l: Math.max(...yt.map((v) => axisNum(v, ystep).length)) * 6.6 + 14,
    r: endText.length * 8.2 + 18,
    t: 14,
    b: 28,
  };
  const iw = W - mg.l - mg.r;
  const ih = H - mg.t - mg.b;
  const x = (tm) => mg.l + (t1 === t0 ? iw / 2 : ((tm - t0) / (t1 - t0)) * iw);
  const y = (v) => mg.t + ih - ((v - lo) / (hi - lo)) * ih;
  const path = pts.map((p, i) => `${i ? "L" : "M"}${x(+p.time).toFixed(1)},${y(p.value).toFixed(1)}`).join("");
  const span = t1 - t0;
  const xt = [0, 1, 2, 3, 4].map((k) => new Date(t0 + (span * k) / 4));
  const showDots = pts.length <= 40;
  const grid = yt
    .map((v) => `<line class="grid" x1="${mg.l}" x2="${mg.l + iw}" y1="${y(v)}" y2="${y(v)}"/><text x="${mg.l - 8}" y="${y(v) + 4}" text-anchor="end">${esc(axisNum(v, ystep))}</text>`)
    .join("");
  const xlab = (t1 === t0 ? [new Date(t0)] : xt)
    .map((d, k, a) => {
      const text = timeLabel(d, span);
      // gleiche Beschriftung wie die vorige weglassen
      if (k && text === timeLabel(a[k - 1], span)) return "";
      const anchor = a.length === 1 ? "middle" : k === 0 ? "start" : k === a.length - 1 ? "end" : "middle";
      return `<text x="${x(+d)}" y="${H - 8}" text-anchor="${anchor}">${esc(text)}</text>`;
    })
    .join("");
  const dots = showDots ? pts.map((p) => `<circle class="dot" cx="${x(+p.time)}" cy="${y(p.value)}" r="4"/>`).join("") : "";
  box.innerHTML = `<div class="chart">
    <svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(t("chart_label", name, endText))}">
      <g class="axis">${grid}${xlab}</g>
      <path class="series" d="${path}"/>
      ${dots}
      <circle class="dot" cx="${x(+last.time)}" cy="${y(last.value)}" r="4"/>
      <text class="end-label" x="${x(+last.time) + 9}" y="${y(last.value) + 4}">${esc(endText)}</text>
      <line class="cross" y1="${mg.t}" y2="${mg.t + ih}" visibility="hidden"/>
      <circle class="dot hover" r="5" visibility="hidden"/>
      <rect class="hit" x="${mg.l}" y="${mg.t}" width="${iw}" height="${ih}" tabindex="0"/>
    </svg>
    <div class="chart-tip" hidden></div>
  </div>`;
  const svgEl = box.querySelector("svg");
  const cross = svgEl.querySelector(".cross");
  const hov = svgEl.querySelector(".dot.hover");
  const tip = box.querySelector(".chart-tip");
  const hit = svgEl.querySelector(".hit");
  let idx = pts.length - 1;
  const show = (i) => {
    idx = Math.max(0, Math.min(pts.length - 1, i));
    const p = pts[idx];
    const px = x(+p.time);
    cross.setAttribute("x1", px);
    cross.setAttribute("x2", px);
    cross.setAttribute("visibility", "visible");
    hov.setAttribute("cx", px);
    hov.setAttribute("cy", y(p.value));
    hov.setAttribute("visibility", "visible");
    const scale = svgEl.clientWidth / W;
    tip.hidden = false;
    tip.style.left = `${px * scale}px`;
    tip.style.top = `${y(p.value) * scale}px`;
    // Wert zuerst, Zeitpunkt danach; Texte nur als textContent
    tip.replaceChildren();
    const b = document.createElement("b");
    const key = document.createElement("span");
    key.className = "key";
    b.append(key, `${p.text} ${p.unit}`);
    const ts = document.createElement("span");
    ts.textContent = p.ts;
    tip.append(b, ts);
  };
  const hide = () => {
    cross.setAttribute("visibility", "hidden");
    hov.setAttribute("visibility", "hidden");
    tip.hidden = true;
  };
  hit.addEventListener("pointermove", (ev) => {
    const r = svgEl.getBoundingClientRect();
    const mx = ((ev.clientX - r.left) / r.width) * W;
    let best = 0;
    for (let i = 1; i < pts.length; i++) if (Math.abs(x(+pts[i].time) - mx) < Math.abs(x(+pts[best].time) - mx)) best = i;
    show(best);
  });
  hit.addEventListener("pointerleave", hide);
  hit.addEventListener("focus", () => show(idx));
  hit.addEventListener("blur", hide);
  hit.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
      ev.preventDefault();
      show(idx + (ev.key === "ArrowRight" ? 1 : -1));
    }
  });
}

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
    // Semikolon und Dezimalkomma wie der übrige CSV-Export (Excel)
    const series = historySeries();
    const v = hist.keys.get(hist.key);
    const lines = [`${t("th_time")};${seriesName(v)}`, ...series.map((p) => `${p.ts};${String(p.text).replace(".", ",")}`)];
    const blob = new Blob(["\ufeff" + lines.join("\r\n")], { type: "text/csv;charset=utf-8" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    const m = hist.meter;
    a.download = `mbusio-${(m.meter_id || m.address).replace(/[^\w-]/g, "_")}-${v.quantity}.csv`;
    a.click();
    URL.revokeObjectURL(a.href);
  }
});

let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => hist?.view === "chart" && renderHistory(), 150);
});
