// Bereich Zähler: Liste links, Details rechts (Konzept neue Oberfläche §3, Schritt 1).
// Zustand als Wort am Element, Bearbeiten direkt in den Feldern, Mehrfachauswahl, Tastatur.

import { state, watch, activeLine } from "./store.js";
import { html, text, list, ask, toast, styles, texts, menu, closeMenu, previewApply } from "./view.js";
import { bind, onEscape } from "./keys.js";
import { loadMeters, loadTree, report } from "./data.js";

texts(
  {
    zl_more: "Liste ▾",
    zl_csv: "Als CSV exportieren",
    zl_history: "Verlauf",
    zl_exchange: "Zähler getauscht …",
    zl_exchange_addr: "Adresse des neuen Zählers",
    zl_exchange_go: "Tauschen",
    zl_paste: "Aus Excel kopierte Zellen hier einfügen, oder:",
    zl_file: "Datei wählen (CSV)",
    zl_names_scope: (n) => (n ? `für die ${n} ausgewählten Zähler` : "für alle Zähler der Linie"),
    zl_close: "Schliessen",
    zl_key_history: "Verlauf zeigen",
  },
  {
    zl_more: "List ▾",
    zl_csv: "Export as CSV",
    zl_history: "History",
    zl_exchange: "Meter replaced …",
    zl_exchange_addr: "Address of the new meter",
    zl_exchange_go: "Replace",
    zl_paste: "Paste cells copied from Excel here, or:",
    zl_file: "Choose file (CSV)",
    zl_names_scope: (n) => (n ? `for the ${n} selected meters` : "for all meters of the line"),
    zl_close: "Close",
    zl_key_history: "Show history",
  }
);

/// Einstellungen je Linie: Suche, Filter, gewählter Zähler, Auswahl (bleiben beim Wechsel)
const perLine = new Map();
let root = null;
let view = null; // Einstellungen der aktiven Linie
let lineId = null;
let values = null; // Werte der letzten Auslesung des gewählten Zählers
const busy = new Set(); // Zähler, die gerade gelesen werden
let releaseEscape = null;

const FILTERS = {
  all: () => true,
  err: (m) => !!(m.last_error || m.plan_info?.disturbed),
  noname: (m) => !m.name,
};
const PLANS = ["off", "daily 00:05", "hourly", "15min", "monthly 00:05"];
const open = () => state.area === "zaehler";

export function mount(pane) {
  styles("zaehler");
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <input type="search" data-search placeholder="${esc(t("n_search"))}" aria-label="${esc(t("n_search"))}">
      <div class="seg" role="group" data-filters></div>
      <span class="grow"></span>
      <span class="sub" data-sum></span>
      <div class="menu-host"><button type="button" class="quiet" data-more>${esc(t("zl_more"))}</button><div class="menu end" role="menu" data-more-menu hidden>
        <button type="button" role="menuitem" data-panel="import">${esc(t("list_import"))} …</button>
        <button type="button" role="menuitem" data-panel="names">${esc(t("quick_names"))}</button>
        <a role="menuitem" class="quiet" data-csv>${esc(t("zl_csv"))}</a>
      </div></div>
      <button type="button" class="tonal" data-add>${esc(t("meter_add"))}</button>
      <button type="button" data-read-all></button>
    </div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" aria-hidden="true"><span></span><span>${esc(t("n_col_meter"))}</span><span>${esc(t("n_col_ident"))}</span><span>${esc(
          t("meter_location")
        )}</span><span>${esc(t("n_col_value"))}</span><span>${esc(t("n_col_state"))}</span></div>
        <div class="rows" role="listbox" tabindex="0" aria-multiselectable="true" aria-label="${esc(t("tab_meters"))}" data-list></div>
        <div class="empty" data-empty hidden></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>
    <div class="selbar" data-selbar hidden></div>`;
  wire();
  watch(["status"], onLine);
  watch(["meters", "status", "tree"], render);
}

const q = (sel) => root.querySelector(sel);

/// Linie gewechselt: Einstellungen dieser Linie holen, Zählerliste laden
function onLine() {
  const id = state.status?.active?.line?.id;
  if (!id || id === lineId) return;
  lineId = id;
  if (!perLine.has(id)) perLine.set(id, { search: "", filter: "all", active: null, selected: new Set(), adding: false, panel: null, history: false, exchange: false });
  view = perLine.get(id);
  q("[data-search]").value = view.search;
  values = null;
  loadMeters().catch(report);
}

function shown() {
  const s = view.search.trim().toLowerCase();
  return (state.meters || []).filter(
    (m) => FILTERS[view.filter](m) && (!s || [m.name, m.location, m.note, m.meter_id, m.address, m.manufacturer].some((x) => String(x ?? "").toLowerCase().includes(s)))
  );
}

const byId = (id) => (state.meters || []).find((m) => m.id === id);

// ------------------------------------------------------------ Zeichnen

function render() {
  if (!view || !state.meters) return;
  const all = state.meters;
  html(
    q("[data-filters]"),
    Object.keys(FILTERS)
      .map((k) => {
        const n = all.filter(FILTERS[k]).length;
        return `<button type="button" data-filter="${k}" aria-pressed="${view.filter === k}">${esc(t("mf_" + k))} <span class="n${k === "err" && n ? " err" : ""}">${n}</span></button>`;
      })
      .join("")
  );
  const job = state.job?.running && state.job.kind === "read_all";
  const readAll = q("[data-read-all]");
  text(readAll, job ? t("cancel") : t("meters_read_all_n", all.length));
  readAll.className = job ? "quiet" : "";
  readAll.hidden = !all.length;
  text(q("[data-sum]"), job ? `${state.job.done}/${state.job.total}` : "");

  const rows = shown();
  if (view.active != null && !byId(view.active)) view.active = null;
  for (const id of [...view.selected]) if (!byId(id)) view.selected.delete(id);
  list(q("[data-list]"), rows, (m) => m.id, row);
  const empty = q("[data-empty]");
  empty.hidden = rows.length > 0;
  html(
    empty,
    all.length
      ? esc(t("meters_none_match"))
      : `${esc(t("n_no_meters"))} <a href="#/scan">${esc(t("n_go_scan"))} →</a> <button type="button" class="quiet" data-add>${esc(t("meter_add"))}</button>`
  );
  renderDetail();
  renderSelbar();
}

function row(m) {
  const sel = view.selected.has(m.id);
  const b = meterBlock(m);
  const ident = [b.ident, b.pa != null && b.pa !== "" ? `PA ${b.pa}` : ""].filter(Boolean).join(" · ");
  const state_ = busy.has(m.id)
    ? `<span class="chip">${esc(t("reading_busy"))}</span>`
    : m.last_error
      ? `<span class="chip err" title="${esc(m.last_error)}">${esc(t("n_no_answer"))}</span>`
      : m.plan_info?.disturbed
        ? `<span class="chip warn">${esc(t("meter_disturbed"))}</span>`
        : m.clock_off != null
          ? `<span class="chip warn" title="${esc(t("meter_clock_title", m.clock_off))}">${esc(t("meter_clock", m.clock_off))}</span>`
          : m.plan_info?.effective && m.plan_info.effective !== "off"
            ? `<span class="sub">${esc(planText(m.plan_info.effective))}</span>`
            : m.plan === "off"
              ? `<span class="sub" title="${esc(t("plan"))}">${esc(planText("off"))}</span>`
              : "";
  const value = m.last_value
    ? `<b class="mono">${esc(m.last_value)}</b> <span class="sub">${esc(slotText(m.last_read))}</span>`
    : `<span class="sub">${esc(t("n_never_read"))}</span>`;
  return `<div class="row${m.id === view.active ? " active" : ""}" role="option" id="meter-${m.id}" aria-selected="${sel}">
    <span class="box" data-box aria-hidden="true">${sel ? "✓" : ""}</span>
    <span class="name">${mediumIcon(m.medium, true)}${m.name ? esc(m.name) : `<span class="sub">${esc(t("meter_unnamed"))}</span>`}</span>
    <span class="mono sub">${esc(ident)}</span>
    <span class="cut">${esc(m.location)}</span>
    <span class="cut">${value}</span>
    <span>${state_}</span>
  </div>`;
}

function renderDetail() {
  const box = q("[data-detail]");
  // wer gerade in ein Feld tippt, verliert nichts: neu gezeichnet wird erst danach
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input:not([type=checkbox]), select");
  if (typing && box.dataset.meter === String(view.adding ? "new" : view.active)) return;
  box.dataset.meter = view.panel || (view.adding ? "new" : String(view.active));
  if (view.panel) return; // Liste importieren / Namen: eigener Inhalt, siehe openPanel
  if (view.adding) return html(box, addForm());
  const m = byId(view.active);
  box.classList.toggle("open", !!m);
  if (!m) {
    html(box, `<p class="sub">${esc(t(state.meters.length ? "n_pick_meter" : "n_no_meters"))}</p>`);
    return;
  }
  const b = meterBlock(m);
  const lp = activeLine()?.plan_info?.effective || "off";
  const plans = [["", t("plan_inherit_line", planText(lp))], ...PLANS.map((p) => [p, planText(p)])];
  if (m.plan && !PLANS.includes(m.plan)) plans.push([m.plan, planText(m.plan)]);
  const info = [b.ident ? `ID ${b.ident}` : "", b.pa != null && b.pa !== "" ? `PA ${b.pa}` : "", b.man, m.medium ? label(m.medium) : "", b.info].filter(Boolean).join(" · ");
  const vals = values?.id === m.id ? values.list : null;
  html(
    box,
    `<button type="button" class="quiet back" data-back>← ${esc(t("tab_meters"))}</button>
    <header class="dhead">${mediumIcon(m.medium)}<div><b>${m.name ? esc(m.name) : esc(t("meter_unnamed"))}</b><div class="sub mono">${esc(info)}</div></div></header>
    ${m.last_error ? `<p class="chip err wide">${esc(m.last_error)}</p>` : ""}
    ${m.clock_off != null ? `<p class="chip warn wide" title="${esc(t("meter_clock_title", m.clock_off))}">${esc(t("meter_clock", m.clock_off))}</p>` : ""}
    <div class="fields">
      ${field("name", t("meter_name"), m.name, t("meter_name_ph"))}
      ${field("location", t("meter_location"), m.location, t("meter_location_ph"))}
      ${field("note", t("meter_note"), m.note, "")}
      <label><span>${esc(t("plan"))}</span><select data-plan>${plans
        .map(([v, txt]) => `<option value="${esc(v)}"${(m.plan || "") === v ? " selected" : ""}>${esc(txt)}</option>`)
        .join("")}</select></label>
    </div>
    <div class="actions">
      <button type="button" data-read title="${esc(t("read"))} (R)">${esc(busy.has(m.id) ? t("reading_busy") : t("read"))}</button>
      <button type="button" class="${view.history ? "tonal" : "quiet"}" data-history aria-pressed="${view.history}" title="${esc(t("zl_history"))} (V)">${esc(t("zl_history"))}</button>
      <button type="button" class="quiet" data-exchange aria-pressed="${view.exchange}">${esc(t("zl_exchange"))}</button>
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-del>${esc(t("delete"))}</button>
    </div>
    ${
      view.exchange
        ? `<div class="fields inset"><p class="sub">${esc(t("exchange_title"))}</p><label><span>${esc(t("address"))}</span><input data-exchange-addr class="mono" placeholder="${esc(
            t("address_ph")
          )}" aria-label="${esc(t("zl_exchange_addr"))}"></label><div class="actions"><button type="button" data-exchange-go>${esc(t("zl_exchange_go"))}</button></div></div>`
        : ""
    }
    ${view.history ? historyHtml(m) : ""}
    <h3>${esc(t("n_last_reading"))} <span class="sub">${esc(m.last_read ? slotText(m.last_read) : t("n_never_read"))}</span></h3>
    ${
      vals
        ? vals.length
          ? `<div class="vals"><div class="vrow head"><span>${esc(t("history_series"))}</span><span></span><span>${esc(t("n_as_dp"))}</span></div>${vals
              .map(
                (v) =>
                  `<label class="vrow${v.main ? " main" : ""}"><span>${esc(v.label)}</span><span class="mono">${esc(v.value ?? "–")} ${esc(v.unit)}</span><input type="checkbox" data-dp="${esc(
                    v.key
                  )}"${v.selected ? " checked" : ""}></label>`
              )
              .join("")}</div>`
          : `<p class="sub">${esc(t("history_none"))}</p>`
        : `<p class="sub">…</p>`
    }`
  );
  if (values?.id !== m.id) loadValues(m.id);
  if (view.history) drawHistory(box, m);
}

// ------------------------------------------------------------ Verlauf

let hist = null; // { id, points, keys (Schlüssel → Reihe), key }

function historyHtml(m) {
  if (hist?.id !== m.id) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">…</p>`;
  if (hist.points.length < 2) return `<h3>${esc(t("zl_history"))}</h3><p class="sub">${esc(t(hist.points.length ? "history_few" : "history_none"))}</p>`;
  return `<h3>${esc(t("zl_history"))} <span class="sub">${esc(t("history_first"))} ${esc(slotText(hist.points[0].timestamp))}</span></h3>
    <div class="actions"><select data-hist-key aria-label="${esc(t("history_series"))}">${[...hist.keys]
      .map(([k, v]) => `<option value="${esc(k)}"${k === hist.key ? " selected" : ""}>${esc(seriesName(v))}</option>`)
      .join("")}</select><button type="button" class="quiet" data-hist-csv>${esc(t("history_csv"))}</button></div>
    <div class="chart" data-chart></div>`;
}

async function loadHistory(m) {
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
  render();
}

/// Diagramm zeichnen, wenn die Fläche neu ist (html() schreibt nur bei Änderung)
function drawHistory(box, m) {
  if (hist?.id !== m.id) return loadHistory(m);
  const el = box.querySelector("[data-chart]");
  if (!el || el.childElementCount || !hist.key) return;
  drawChart(el, plotSeries(hist.points, hist.key), seriesName(hist.keys.get(hist.key)));
}

// ------------------------------------------------------------ Liste importieren, Namen nach Muster

let listData = null;

function openPanel(kind) {
  closeMenu();
  view.panel = kind;
  view.adding = false;
  const box = q("[data-detail]");
  box.classList.add("open");
  box._html = null;
  if (kind === "import") {
    listData = null;
    box.innerHTML = `<button type="button" class="quiet back" data-close-panel>← ${esc(t("tab_meters"))}</button>
      <header class="dhead"><b>${esc(t("list_title"))}</b></header><p class="sub">${esc(t("list_hint"))}</p>
      <textarea data-paste rows="5" class="mono" placeholder="${esc(t("list_paste_ph"))}" aria-label="${esc(t("zl_paste"))}"></textarea>
      <label class="quiet filebtn">${esc(t("zl_file"))}<input type="file" accept=".csv,.txt,text/csv" data-file hidden></label>
      <div data-map></div><div data-result></div>
      <div class="actions"><span class="grow"></span><button type="button" class="quiet" data-close-panel>${esc(t("zl_close"))}</button></div>`;
  } else {
    const n = view.selected.size;
    box.innerHTML = `<button type="button" class="quiet back" data-close-panel>← ${esc(t("tab_meters"))}</button>
      <header class="dhead"><b>${esc(t("names_title"))}</b></header><p class="sub">${esc(t("names_hint"))}</p>
      <div class="fields"><label><span>${esc(t("names_pattern"))}</span><input data-pattern class="mono" value="{ort} {medium}"></label>
      <label class="check"><span>${esc(t("names_only_empty"))}</span><input type="checkbox" data-only-empty></label></div>
      <p class="sub">${esc(t("zl_names_scope", n))}</p>
      <div class="actions"><button type="button" data-names-preview>${esc(t("quick_names"))}</button><button type="button" class="quiet" data-close-panel>${esc(t("zl_close"))}</button></div>
      <div data-result></div>`;
  }
  box.querySelector("textarea, input")?.focus();
}

function closePanel() {
  view.panel = null;
  q("[data-detail]")._html = null;
  render();
}

function listLoad(text_) {
  listData = listLogic.guess(parseTable(text_));
  renderListMap();
}

function renderListMap() {
  const box = q("[data-detail] [data-map]");
  if (!box) return;
  if (!listData) return (box.innerHTML = "");
  const d = listData;
  const names = Array.from({ length: d.cols }, (_, i) => (d.header && d.rows[0][i]) || t("list_col", i + 1));
  box.innerHTML = `<h3>${esc(t("list_map"))} <span class="sub">${esc(t("list_rows", d.rows.length - (d.header ? 1 : 0)))}</span></h3>
    <div class="fields"><label class="check"><span>${esc(t("list_header"))}</span><input type="checkbox" data-header${d.header ? " checked" : ""}></label>
    ${listLogic.FIELDS.map(
      (f) => `<label><span>${esc(t("list_f_" + f))}</span><select data-map-f="${f}"><option value="">${esc(t("list_unused"))}</option>${names
        .map((n_, i) => `<option value="${i}"${d.map[f] === i ? " selected" : ""}>${esc(n_)}</option>`)
        .join("")}</select></label>`
    ).join("")}</div>
    <div class="actions"><button type="button" data-list-check>${esc(t("list_check"))}</button></div>`;
}

async function listCheck() {
  if (listData?.map.id === undefined) return toast(t("list_need_id"), { error: true });
  const rows = listLogic.rows(listData);
  await previewApply(q("[data-detail] [data-result]"), {
    title: t("list_check"),
    run: (apply) => request("meters/import-list", { method: "POST", body: { rows, apply } }),
    done: async (cs) => {
      if (!cs) return;
      toast(t("list_done", cs.added.length, cs.changed.length));
      await loadMeters();
      loadTree().catch(() => {});
      closePanel();
    },
  }).catch(report);
}

async function namesPreview() {
  const box = q("[data-detail]");
  const body = { pattern: box.querySelector("[data-pattern]").value, only_empty: box.querySelector("[data-only-empty]").checked };
  if (view.selected.size) body.ids = [...view.selected];
  await previewApply(box.querySelector("[data-result]"), {
    title: t("names_title"),
    run: (apply) => request("meters/names", { method: "POST", body: { ...body, apply } }),
    done: async (cs) => {
      if (!cs) return;
      toast(t("names_done", cs.changed.length));
      await loadMeters();
      closePanel();
    },
  }).catch(report);
}

async function exchange(button) {
  const m = byId(view.active);
  const addr = q("[data-detail] [data-exchange-addr]").value.trim();
  if (!addr) return;
  if (!(await ask(button, t("exchange_confirm", m.meter_id || m.address, addr), t("zl_exchange_go")))) return;
  try {
    const r = await request(`meters/${m.id}/exchange`, { method: "POST", body: { address: addr } });
    view.exchange = false;
    values = null;
    hist = null;
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  } catch (e) {
    report(e);
  }
}

const field = (name, label_, value, ph) =>
  `<label><span>${esc(label_)}</span><input data-field="${name}" value="${esc(value)}" placeholder="${esc(ph)}" data-saved="${esc(value)}"></label>`;

function addForm() {
  return `<header class="dhead"><b>${esc(t("meter_add"))}</b></header>
    <div class="fields">
      <label><span>${esc(t("address"))}</span><input data-new="address" class="mono" placeholder="${esc(t("address_ph"))}" autofocus></label>
      <label><span>${esc(t("meter_name"))}</span><input data-new="name" placeholder="${esc(t("meter_name_ph"))}"></label>
      <label><span>${esc(t("meter_location"))}</span><input data-new="location" placeholder="${esc(t("meter_location_ph"))}"></label>
    </div>
    <div class="actions"><button type="button" data-save-new>${esc(t("remember"))}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button></div>`;
}

function renderSelbar() {
  const bar = q("[data-selbar]");
  const n = view.selected.size;
  bar.hidden = !n;
  if (!n) {
    releaseEscape?.();
    releaseEscape = null;
    return;
  }
  releaseEscape ??= onEscape(clearSelection);
  html(
    bar,
    `<b>${esc(t("n_selected", n))}</b>
    <button type="button" data-sel-read>${esc(t("read"))}</button>
    <select data-sel-plan aria-label="${esc(t("plan"))}"><option value="">${esc(t("n_set_plan"))}</option><option value="inherit">${esc(
      t("plan_inherit_line", planText(activeLine()?.plan_info?.effective || "off"))
    )}</option>${PLANS.map((p) => `<option value="${p}">${esc(planText(p))}</option>`).join("")}</select>
    <button type="button" class="quiet danger-text" data-sel-del>${esc(t("n_delete_n", n))}</button>
    <span class="grow"></span><span class="sub">${esc(t("n_esc_clear"))}</span>`
  );
}

// ------------------------------------------------------------ Aktionen

async function loadValues(id) {
  values = { id, list: null };
  try {
    const r = await request(`meters/${id}/values`);
    if (values.id === id) values = { id, list: r.values };
  } catch (e) {
    values = { id, list: [] };
    report(e);
  }
  render();
}

function select(id, { focusRow = false } = {}) {
  view.active = id;
  view.adding = false;
  view.panel = null;
  view.exchange = false;
  render();
  const el = root.querySelector(`#meter-${id}`);
  if (el) {
    el.scrollIntoView({ block: "nearest" });
    q("[data-list]").setAttribute("aria-activedescendant", el.id);
  }
  if (focusRow) q("[data-list]").focus();
}

function toggle(id) {
  if (view.selected.has(id)) view.selected.delete(id);
  else view.selected.add(id);
  render();
}

function clearSelection() {
  view.selected.clear();
  render();
}

async function readMeters(ids) {
  for (const id of ids) busy.add(id);
  render();
  let failed = 0;
  for (const id of ids) {
    try {
      await request(`meters/${id}/read`, { method: "POST" });
    } catch {
      failed++;
    }
    busy.delete(id);
    if (values?.id === id) values = null;
    await loadMeters().catch(() => {});
  }
  render();
  loadTree().catch(() => {});
  if (ids.length > 1) toast(failed ? t("read_all_sum_bad", ids.length - failed, ids.length, failed) : t("read_all_sum_ok", ids.length));
  else if (failed) toast(byId(ids[0])?.last_error || t("n_no_answer"), { error: true });
}

async function readAll() {
  if (state.job?.running) return request("job/cancel", { method: "POST" }).catch(report);
  try {
    let job = await request("meters/read-all", { method: "POST" });
    while (job.running) {
      state.job = job;
      await loadMeters();
      await new Promise((r) => setTimeout(r, 700));
      job = await request("job");
    }
    state.job = null;
    await loadMeters();
    loadTree().catch(() => {});
    const bad = state.meters.filter((m) => m.last_error).length;
    toast(bad ? t("read_all_sum_bad", state.meters.length - bad, state.meters.length, bad) : t("read_all_sum_ok", state.meters.length), { error: !!bad });
  } catch (e) {
    state.job = null;
    report(e);
  }
  render();
}

/// Feld beim Verlassen speichern; Escape setzt es auf den gespeicherten Wert zurück
async function saveField(input) {
  const m = byId(view.active);
  if (!m || input.value === input.dataset.saved) return;
  try {
    await request(`meters/${m.id}`, { method: "PUT", body: { ...m, [input.dataset.field]: input.value.trim() } });
    input.dataset.saved = input.value.trim();
    input.classList.add("saved");
    setTimeout(() => input.classList.remove("saved"), 900);
    await loadMeters();
  } catch (e) {
    report(e);
  }
}

async function setPlan(ids, plan) {
  try {
    for (const id of ids) await request(`meters/${id}/plan`, { method: "PUT", body: { plan } });
    await loadMeters();
    loadTree().catch(() => {});
  } catch (e) {
    report(e);
  }
}

async function remove(ids, anchor) {
  const one = ids.length === 1 ? byId(ids[0]) : null;
  const question = one ? t("n_delete_q_one", one.name || meterBlock(one).ident || one.address) : t("n_delete_q", ids.length);
  if (!(await ask(anchor, question, t("delete")))) return;
  try {
    for (const id of ids) await request(`meters/${id}`, { method: "DELETE" });
    view.selected.clear();
    await loadMeters();
    loadTree().catch(() => {});
    toast(t("n_deleted", ids.length));
  } catch (e) {
    report(e);
  }
}

async function saveNew() {
  const v = (k) => q(`[data-new=${k}]`).value.trim();
  try {
    const m = await request("meters", { method: "POST", body: { address: v("address"), name: v("name"), location: v("location") } });
    view.adding = false;
    await loadMeters();
    loadTree().catch(() => {});
    select(m.id);
  } catch (e) {
    report(e);
  }
}

/// Mobil: Details als eigene Ansicht; auf dem Desktop stehen sie immer rechts
function back() {
  view.active = null;
  render();
  q("[data-list]").focus();
}

// ------------------------------------------------------------ Bedienung

function wire() {
  const listEl = q("[data-list]");
  q("[data-search]").addEventListener("input", (ev) => {
    view.search = ev.target.value;
    render();
  });
  root.addEventListener("click", (ev) => {
    const el = ev.target;
    if (el.closest("[data-filter]")) {
      view.filter = el.closest("[data-filter]").dataset.filter;
      return render();
    }
    if (el.closest("[data-add]")) {
      view.adding = true;
      render();
      return q("[data-new=address]")?.focus();
    }
    if (el.closest("[data-cancel-new]")) {
      view.adding = false;
      return render();
    }
    if (el.closest("[data-save-new]")) return saveNew();
    if (el.closest("[data-read-all]")) return readAll();
    if (el.closest("[data-panel]")) return openPanel(el.closest("[data-panel]").dataset.panel);
    if (el.closest("[data-close-panel]")) return closePanel();
    if (el.closest("[data-list-check]")) return listCheck();
    if (el.closest("[data-names-preview]")) return namesPreview();
    if (el.closest("[data-history]")) {
      view.history = !view.history;
      return render();
    }
    if (el.closest("[data-hist-csv]")) return downloadSeries(plotSeries(hist.points, hist.key), hist.keys.get(hist.key), byId(view.active));
    if (el.closest("[data-exchange]")) {
      view.exchange = !view.exchange;
      render();
      return q("[data-exchange-addr]")?.focus();
    }
    if (el.closest("[data-exchange-go]")) return exchange(el.closest("[data-exchange-go]"));
    if (el.closest("[data-read]")) return readMeters([view.active]);
    if (el.closest("[data-del]")) return remove([view.active], el.closest("[data-del]"));
    if (el.closest("[data-sel-read]")) return readMeters([...view.selected]);
    if (el.closest("[data-sel-del]")) return remove([...view.selected], el.closest("[data-sel-del]"));
    if (el.closest("[data-back]")) return back();
    const r = el.closest("[role=option]");
    if (!r) return;
    const id = Number(r.dataset.key);
    if (el.closest("[data-box]") || ev.ctrlKey || ev.metaKey) return toggle(id);
    if (ev.shiftKey && view.active != null) return selectRange(view.active, id);
    select(id);
  });
  root.addEventListener("change", (ev) => {
    const el = ev.target;
    if (el.matches("[data-field]")) return saveField(el);
    if (el.matches("[data-plan]")) return setPlan([view.active], el.value || null);
    if (el.matches("[data-sel-plan]") && el.value) return setPlan([...view.selected], el.value === "inherit" ? null : el.value);
    if (el.matches("[data-dp]")) return setDatapoints();
    if (el.matches("[data-hist-key]")) {
      hist.key = el.value;
      q("[data-detail]")._html = null;
      return render();
    }
    if (el.matches("[data-file]") && el.files[0]) return el.files[0].text().then(listLoad);
    if (el.matches("[data-header]")) {
      listData.header = el.checked;
      return renderListMap();
    }
    if (el.matches("[data-map-f]")) {
      const f = el.dataset.mapF;
      const v = el.value === "" ? undefined : Number(el.value);
      for (const g of listLogic.FIELDS) if (listData.map[g] === v && v !== undefined) delete listData.map[g];
      listData.map[f] = v;
      return renderListMap();
    }
  });
  root.addEventListener("keydown", (ev) => {
    const el = ev.target;
    if (el.matches("[data-field]") && ev.key === "Escape") {
      el.value = el.dataset.saved;
      el.blur();
      ev.stopPropagation();
    }
    if (el.matches("[data-field]") && ev.key === "Enter") el.blur();
    if (el.matches("[data-new]") && ev.key === "Enter") saveNew();
  });
  listEl.addEventListener("keydown", (ev) => {
    const rows = shown();
    const i = rows.findIndex((m) => m.id === view.active);
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      const next = rows[Math.max(0, Math.min(rows.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))];
      if (!next) return;
      if (ev.shiftKey && view.active != null) {
        view.selected.add(view.active);
        view.selected.add(next.id);
      }
      select(next.id, { focusRow: true });
    } else if (ev.key === " " && view.active != null) {
      ev.preventDefault();
      toggle(view.active);
    } else if (ev.key === "Enter" && view.active != null) {
      ev.preventDefault();
      q("[data-field=name]")?.focus();
    } else if (ev.key.toLowerCase() === "a" && (ev.ctrlKey || ev.metaKey)) {
      ev.preventDefault();
      for (const m of rows) view.selected.add(m.id);
      render();
    }
  });
  root.addEventListener("input", (ev) => {
    if (ev.target.matches("[data-paste]")) listLoad(ev.target.value);
  });
  menu(q("[data-more]"), q("[data-more-menu]"), { onOpen: () => (q("[data-csv]").href = apiHref("meters/csv")) });
  listEl.addEventListener("focus", () => {
    if (view.active == null && shown()[0]) select(shown()[0].id);
  });
  bind("r", () => (view.selected.size ? readMeters([...view.selected]) : view.active != null && readMeters([view.active])), t("n_key_read"), open);
  bind(
    "v",
    () => {
      view.history = !view.history;
      render();
    },
    t("zl_key_history"),
    open
  );
}

function selectRange(from, to) {
  const rows = shown().map((m) => m.id);
  const [a, b] = [rows.indexOf(from), rows.indexOf(to)].sort((x, y) => x - y);
  for (const id of rows.slice(a, b + 1)) view.selected.add(id);
  select(to);
}

async function setDatapoints() {
  const m = byId(view.active);
  const keys = [...q("[data-detail]").querySelectorAll("[data-dp]:checked")].map((c) => c.dataset.dp);
  try {
    await request(`meters/${m.id}/datapoints`, { method: "PUT", body: { keys } });
    values = null;
    loadTree().catch(() => {});
    render();
  } catch (e) {
    report(e);
  }
}
