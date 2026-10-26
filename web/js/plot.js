"use strict";

// Verlauf als Diagramm (SVG).
// Stile: css/plot.css. Eingabe: Punkte aus /api/meters/{id}/history.

/// Name einer Reihe: Grösse, Tarif, Untereinheit (z.B. Impulseingang), Einheit
function seriesName(v) {
  return `${label(v.quantity)}${v.tariff ? ` · ${t("tariff")} ${v.tariff}` : ""}${v.subunit ? ` · ${t("subunit_n", v.subunit)}` : ""}${v.unit ? ` (${v.unit})` : ""}`;
}

/// Eine Reihe aus den Punkten des Verlaufs: [{ time, ts, unsure, value, text, unit }]
function plotSeries(points, key) {
  return points
    .map((p) => {
      const v = p.values.find((x) => x.key === key);
      return v ? { time: parseTime(p.timestamp), ts: p.timestamp, unsure: p.time_unsure, value: v.value, text: v.text, unit: v.unit } : null;
    })
    .filter(Boolean);
}

/// Reihe als CSV herunterladen; Semikolon und Dezimalkomma wie der übrige CSV-Export (Excel)
function downloadSeries(series, v, meter) {
  const lines = [`${t("th_time")};${seriesName(v)}`, ...series.map((p) => `${p.ts};${String(p.text).replace(".", ",")}`)];
  const blob = new Blob(["\ufeff" + lines.join("\r\n")], { type: "text/csv;charset=utf-8" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `mbusio-${(meter.meter_id || meter.address).replace(/[^\w-]/g, "_")}-${v.quantity}.csv`;
  a.click();
  URL.revokeObjectURL(a.href);
}

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
