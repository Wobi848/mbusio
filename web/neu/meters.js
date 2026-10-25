// Bereich Zähler: Liste links, Details rechts (Konzept neue Oberfläche §3, Schritt 1).
// Zustand als Wort am Element, Bearbeiten direkt in den Feldern, Mehrfachauswahl, Tastatur.

import { state, watch, activeLine } from "./store.js";
import { html, text, list, ask, toast } from "./view.js";
import { bind, onEscape } from "./keys.js";
import { loadMeters, loadTree, report } from "./data.js";

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
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <input type="search" data-search placeholder="${esc(t("n_search"))}" aria-label="${esc(t("n_search"))}">
      <div class="seg" role="group" data-filters></div>
      <span class="grow"></span>
      <span class="sub" data-sum></span>
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
  if (!perLine.has(id)) perLine.set(id, { search: "", filter: "all", active: null, selected: new Set(), adding: false });
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
  box.dataset.meter = view.adding ? "new" : String(view.active);
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
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-del>${esc(t("delete"))}</button>
    </div>
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
  listEl.addEventListener("focus", () => {
    if (view.active == null && shown()[0]) select(shown()[0].id);
  });
  bind("r", () => (view.selected.size ? readMeters([...view.selected]) : view.active != null && readMeters([view.active])), t("n_key_read"), open);
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
