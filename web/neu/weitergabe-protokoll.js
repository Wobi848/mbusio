// Bereich Weitergabe: Protokoll, was an die Ausgänge ging, was wartet und was verworfen wurde
// (Konzept Export, Teil A). Ersetzt beim Öffnen Werkzeugleiste und Liste der Datenpunkte.

import { html, text } from "./view.js";
import { onEscape } from "./keys.js";
import { report } from "./data.js";

const EVERY = 3000;
const DAYS = [1, 7, 30];
const RESULTS = ["", "sent", "waiting", "dropped"];
const RESULT_CHIP = { sent: "ok", waiting: "", dropped: "err" };

let box = null;
let filter = null; // { dp, dpName, days, result, q }
let timer = null;
let releaseEscape = null;
let onClose = null;

/// Protokoll in `container` öffnen; `dp` mit `dpName` filtert auf einen Datenpunkt
export function openLog(container, { dp = null, dpName = "", close }) {
  box = container;
  onClose = close;
  filter = { dp, dpName, days: 7, result: "", q: "" };
  box.hidden = false;
  box.innerHTML = `
    <div class="toolbar">
      <button type="button" class="quiet" data-log-back>← ${esc(t("wg_log_back"))}</button>
      <input type="search" data-search data-log-q placeholder="${esc(t("n_search"))}" aria-label="${esc(t("n_search"))}">
      <span data-log-dp></span>
      <div class="seg" role="group" data-log-days></div>
      <div class="seg" role="group" data-log-results></div>
      <span class="grow"></span>
      <a class="quiet" data-log-csv>CSV</a>
    </div>
    <div class="listwrap logwrap">
      <div class="row head" aria-hidden="true"><span>${esc(t("wg_log_time"))}</span><span>${esc(t("wg_log_output"))}</span><span>${esc(t("wg_col_name"))}</span><span>${esc(
        t("wg_log_address")
      )}</span><span>${esc(t("history_series"))}</span><span>${esc(t("wg_col_quality"))}</span><span>${esc(t("wg_log_result"))}</span></div>
      <div class="rows" data-log-rows></div>
      <div class="empty" data-log-empty hidden></div>
    </div>`;
  box.addEventListener("click", onClick);
  box.querySelector("[data-log-q]").addEventListener("input", (ev) => {
    filter.q = ev.target.value;
    load();
  });
  releaseEscape = onEscape(closeLog);
  load();
  timer = setInterval(() => !document.hidden && load(), EVERY);
}

export function closeLog() {
  if (!box) return;
  clearInterval(timer);
  releaseEscape?.();
  box.removeEventListener("click", onClick);
  box.hidden = true;
  box.innerHTML = "";
  box = null;
  onClose?.();
}

function query() {
  const p = new URLSearchParams({ days: filter.days, result: filter.result, q: filter.q.trim() });
  if (filter.dp != null) p.set("dp", filter.dp);
  return `out-log?${p}`;
}

async function load() {
  if (!box) return;
  try {
    render(await request(query()));
  } catch (e) {
    report(e);
  }
}

function render(page) {
  if (!box) return;
  const q = (sel) => box.querySelector(sel);
  html(
    q("[data-log-dp]"),
    filter.dp != null ? `<button type="button" class="chip" data-log-dp-clear title="${esc(t("wg_log_dp_clear"))}">${esc(t("wg_log_dp", filter.dpName))} ×</button>` : ""
  );
  html(
    q("[data-log-days]"),
    DAYS.map((d) => `<button type="button" data-log-day="${d}" aria-pressed="${filter.days === d}">${esc(d === 1 ? t("wg_log_today") : t("wg_log_days", d))}</button>`).join("")
  );
  const n = (k) => (k ? page.counts[k] || 0 : (page.counts.sent || 0) + (page.counts.waiting || 0) + (page.counts.dropped || 0));
  html(
    q("[data-log-results]"),
    RESULTS.map(
      (r) =>
        `<button type="button" data-log-result="${r}" aria-pressed="${filter.result === r}">${esc(t("wg_res_" + (r || "all")))} <span class="n${r === "dropped" && n(r) ? " err" : ""}">${n(r)}</span></button>`
    ).join("")
  );
  q("[data-log-csv]").href = apiHref(`${query()}&format=csv`);
  html(q("[data-log-rows]"), page.rows.map(row).join(""));
  const empty = q("[data-log-empty]");
  empty.hidden = page.rows.length > 0 && !page.more;
  text(empty, page.more ? t("wg_log_more") : page.rows.length ? "" : t("wg_log_empty"));
}

function row(r) {
  const result = esc(t("wg_res_" + r.result));
  const reason = r.reason ? t("wg_why_" + r.reason) : "";
  return `<div class="row">
    <span class="mono sub">${esc(r.time)}</span>
    <span>${esc(r.output.toUpperCase())}</span>
    <span class="cut">${r.dp != null ? esc(r.name) : `<span class="sub">${esc(t("wg_log_other"))}</span>`}</span>
    <span class="cut mono sub" title="${esc(r.address)}">${esc(r.address)}</span>
    <span class="mono">${esc(r.value)}</span>
    <span>${r.quality ? `<span class="sub">${esc(t("q_" + r.quality))}</span>` : ""}</span>
    <span><span class="chip ${RESULT_CHIP[r.result] ?? ""}"${reason ? ` title="${esc(reason)}"` : ""}>${result}</span>${reason ? ` <span class="sub cut">${esc(reason)}</span>` : ""}</span>
  </div>`;
}

function onClick(ev) {
  const b = (sel) => ev.target.closest(sel);
  if (b("[data-log-back]")) return closeLog();
  if (b("[data-log-dp-clear]")) {
    filter.dp = null;
    return load();
  }
  if (b("[data-log-day]")) {
    filter.days = Number(b("[data-log-day]").dataset.logDay);
    return load();
  }
  if (b("[data-log-result]")) {
    filter.result = b("[data-log-result]").dataset.logResult;
    return load();
  }
}
