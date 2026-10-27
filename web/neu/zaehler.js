// Bereich Zähler: Liste links, Details rechts (Konzept neue Oberfläche §3, Schritt 1).
// Zustand als Wort am Element, Bearbeiten direkt in den Feldern, Mehrfachauswahl, Tastatur.

import { state, watch, activeLine, faulty } from "./store.js";
import { html, text, list, ask, toast, styles, menu, closeMenu, explain } from "./view.js";
import { bind, onEscape } from "./keys.js";
import { loadMeters, loadTree, loadStatus, report } from "./data.js";
import "./zaehler-texte.js";
import { drawHistory, setHistoryKey, setHistoryView, historyCsv, forgetHistory } from "./zaehler-verlauf.js";
import { PLANS, rowHtml, detailHtml, addFormHtml } from "./zaehler-ansicht.js";
import { panelHtml, listLoad, listChange, listCheck, namesPreview } from "./zaehler-listen.js";
import { locationsHtml, saveQuick, sameAsAbove, quickEnter } from "./zaehler-benennen.js";


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
  err: faulty,
  noname: (m) => !m.name,
};
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
        <button type="button" role="menuitem" data-naming>${esc(t("zl_naming"))}</button>
        <button type="button" role="menuitem" data-panel="names">${esc(t("quick_names"))}</button>
        <a role="menuitem" class="quiet" data-csv>${esc(t("zl_csv"))}</a>
        <a role="menuitem" class="quiet" data-readings="csv">${esc(t("zl_readings_csv"))}</a>
        <a role="menuitem" class="quiet" data-readings="json">${esc(t("zl_readings_json"))}</a>
        <button type="button" role="menuitem" class="danger-text" data-readings-clear>${esc(t("zl_readings_clear"))} …</button>
      </div></div>
      <button type="button" data-naming-done hidden>${esc(t("zl_naming_done"))}</button>
      <datalist id="zl-locations" data-locations></datalist>
      <button type="button" class="tonal" data-add>${esc(t("meter_add"))}</button>
      <button type="button" data-read-all></button>
      ${explain(t("zl_explain"))}
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
  list(q("[data-list]"), rows, (m) => m.id, (m) => rowHtml(m, { active: view.active, selected: view.selected, busy, naming: view.naming }));
  q("[data-naming-done]").hidden = !view.naming;
  if (view.naming) html(q("[data-locations]"), locationsHtml(all));
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

function renderDetail() {
  const box = q("[data-detail]");
  // wer gerade in ein Feld tippt, verliert nichts: neu gezeichnet wird erst danach
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input:not([type=checkbox]), select");
  if (typing && box.dataset.meter === String(view.adding ? "new" : view.active)) return;
  box.dataset.meter = view.panel || (view.adding ? "new" : String(view.active));
  if (view.panel) return; // Liste importieren / Namen: eigener Inhalt, siehe openPanel
  if (view.adding) return html(box, addFormHtml());
  const m = byId(view.active);
  box.classList.toggle("open", !!m);
  if (!m) {
    html(box, `<p class="sub">${esc(t(state.meters.length ? "n_pick_meter" : "n_no_meters"))}</p>`);
    return;
  }
  html(box, detailHtml(m, { vals: values?.id === m.id ? values.list : null, template: values?.id === m.id ? values.template : null, busy: busy.has(m.id), history: view.history, exchange: view.exchange, linePlan: activeLine()?.plan_info?.effective || "off" }));
  if (values?.id !== m.id) loadValues(m.id);
  if (view.history) drawHistory(box, m, render);
}

// ------------------------------------------------------------ Felder im Detailbereich

/// Liste importieren oder Namen nach Muster (zaehler-listen.js) statt der Details
function openPanel(kind) {
  closeMenu();
  view.panel = kind;
  view.adding = false;
  const box = q("[data-detail]");
  box.classList.add("open");
  box._html = null;
  box.innerHTML = panelHtml(kind, view.selected.size);
  box.querySelector("textarea, input")?.focus();
}

function closePanel() {
  view.panel = null;
  q("[data-detail]")._html = null;
  render();
}

/// nach Übernehmen aus einem Feld: neu laden, Feld zu
async function panelDone() {
  await loadMeters();
  loadTree().catch(() => {});
  closePanel();
}

/// Erkannten Zählertausch bestätigen (Konzept Export 3.6)
async function confirmSwap(id) {
  try {
    await request(`exchanges/${id}/confirm`, { method: "POST" });
    toast(t("zl_swap_confirmed"));
    await loadMeters();
  } catch (e) {
    report(e);
  }
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
    forgetHistory();
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  } catch (e) {
    report(e);
  }
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
    if (values.id === id) values = { id, list: r.values, template: r.template };
  } catch (e) {
    values = { id, list: [], template: null };
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
    loadStatus().catch(() => {}); // Statusleiste gleich auf „Bus arbeitet“
    while (job.running) {
      state.job = job;
      await loadMeters();
      await new Promise((r) => setTimeout(r, 700));
      job = await request("job");
    }
    state.job = null;
    await loadMeters();
    loadTree().catch(() => {});
    loadStatus().catch(() => {});
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

function startNaming() {
  view.naming = true;
  render();
  q("[data-list] input[data-q=name]")?.focus();
}

/// Benennen beenden: Liste neu laden, damit Filter und Zähler stimmen
async function stopNaming() {
  view.naming = false;
  await loadMeters().catch(report);
  loadTree().catch(() => {});
  render();
}

/// Alle Auslesungen der Linie löschen (Zählerliste und Datenpunkte bleiben)
async function clearReadings() {
  if (!(await ask(q("[data-more]"), t("zl_readings_clear_q"), t("delete")))) return;
  try {
    await request("readings", { method: "DELETE" });
    values = null;
    await loadMeters();
    loadTree().catch(() => {});
    toast(t("zl_readings_cleared"));
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
    if (el.closest("[data-naming]")) return startNaming();
    if (el.closest("[data-naming-done]")) return stopNaming();
    if (el.closest("[data-q-same]")) return sameAsAbove(el.closest("[data-q-same]"), shown(), state.meters);
    if (el.matches("input[data-q]")) return;
    if (el.closest("[data-readings-clear]")) return clearReadings();
    if (el.closest("[data-close-panel]")) return closePanel();
    if (el.closest("[data-list-check]")) return listCheck(q("[data-detail]"), panelDone);
    if (el.closest("[data-names-preview]")) return namesPreview(q("[data-detail]"), [...view.selected], panelDone);
    if (el.closest("[data-history]")) {
      view.history = !view.history;
      return render();
    }
    if (el.closest("[data-hist-csv]")) return historyCsv(byId(view.active));
    if (el.closest("[data-hist-view]")) {
      setHistoryView(el.closest("[data-hist-view]").dataset.histView);
      return render();
    }
    if (el.closest("[data-exchange]")) {
      view.exchange = !view.exchange;
      render();
      return q("[data-exchange-addr]")?.focus();
    }
    if (el.closest("[data-exchange-go]")) return exchange(el.closest("[data-exchange-go]"));
    if (el.closest("[data-swap-confirm]")) return confirmSwap(el.closest("[data-swap-confirm]").dataset.swapConfirm);
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
    if (el.matches("input[data-q]")) return saveQuick(el, state.meters);
    if (el.matches("[data-plan]")) return setPlan([view.active], el.value || null);
    if (el.matches("[data-sel-plan]") && el.value) return setPlan([...view.selected], el.value === "inherit" ? null : el.value);
    if (el.matches("[data-dp], [data-tpl], [data-tpl-auto]")) return setDatapoints();
    if (el.matches("[data-hist-key]")) {
      setHistoryKey(el.value);
      q("[data-detail]")._html = null;
      return render();
    }
    listChange(el, q("[data-detail]"));
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
    if (el.matches("input[data-q]") && ev.key === "Enter") return quickEnter(ev, q("[data-list]"), state.meters);
    if (el.matches("input[data-q]")) ev.stopPropagation();
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
    if (ev.target.matches("[data-paste]")) listLoad(ev.target.value, q("[data-detail]"));
  });
  menu(q("[data-more]"), q("[data-more-menu]"), {
    onOpen: () => {
      q("[data-csv]").href = apiHref("meters/csv");
      for (const a of root.querySelectorAll("[data-readings]")) a.href = apiHref(`export?format=${a.dataset.readings}`);
    },
  });
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

/// Auswahl speichern; mit „Gleiche Auswahl“ als Vorlage für alle Zähler des Mediums auf der Linie.
/// Vorlage abgewählt: sie wird entfernt, die Datenpunkte der anderen Zähler bleiben.
async function setDatapoints() {
  const m = byId(view.active);
  const box = q("[data-detail]");
  const keys = [...box.querySelectorAll("[data-dp]:checked")].map((c) => c.dataset.dp);
  const same = !!box.querySelector("[data-tpl]")?.checked;
  const old = values?.template;
  try {
    const r = await request(`meters/${m.id}/datapoints`, {
      method: "PUT",
      body: { keys, template: same ? { auto: !!box.querySelector("[data-tpl-auto]")?.checked } : null },
    });
    if (!same && old) await request(`templates/${old.id}`, { method: "DELETE" });
    if (r.others) toast(t("dp_saved", r.added, r.removed, r.others));
    values = null;
    loadTree().catch(() => {});
    render();
  } catch (e) {
    report(e);
  }
}
