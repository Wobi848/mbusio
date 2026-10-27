// Bereich Anlage: Projektbaum (Gateways › Linien) links mit festen Spalten, rechts Projekt,
// Gateway oder Linie bearbeiten (Konzept neue Oberfläche §3.2, Schritt 3). Zustand der Anlage
// als Raster oben, nur wenn eine Linie des Projekts im Dauerbetrieb ist.

import { state, set, watch } from "./store.js";
import { html, text, list, ask, toast, styles, texts } from "./view.js";
import "./anlage-texte.js";
import { allLines, sum, plantHtml } from "./anlage-zustand.js";
import { rowHtml } from "./anlage-baum.js";
import { detailHtml, TIMED } from "./anlage-details.js";
import { bind } from "./keys.js";
import { loadTree, loadMeters, loadStatus, reload, useLine, report, switchOperation } from "./data.js";


const open = () => state.area === "anlage";

/// Einstellungen je Projekt: gewähltes Element („p“, „g:3“, „l:7“), zugeklappte Gateways,
/// Feld für ein neues Gateway („gateway“) oder eine neue Linie (Gateway-ID)
const perProject = new Map();
let root = null;
let view = null;
let projectId = null;
let addingProject = false;

export function mount(pane) {
  styles("anlage");
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <span class="newproj" data-newproj></span>
      <button type="button" class="quiet" data-import>${esc(t("import"))}</button>
      <input type="file" accept=".json,application/json" data-import-file hidden>
      <span class="grow"></span>
      <a class="quiet" data-export download title="${esc(t("an_backup_title"))}">${esc(t("an_backup"))}</a>
      <button type="button" data-report title="${esc(t("report_title"))}">${esc(t("report"))}</button>
    </div>
    <div class="plant" data-plant hidden></div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" aria-hidden="true"><span>${esc(t("an_col_name"))}</span><span>${esc(t("an_col_conn"))}</span><span class="num">${esc(
          t("an_col_meters")
        )}</span><span class="num">${esc(t("an_col_dp"))}</span><span>${esc(t("an_col_op"))}</span><span>${esc(t("an_col_state"))}</span><span></span></div>
        <div class="rows" role="listbox" tabindex="0" aria-label="${esc(t("tab_project"))}" data-list></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>`;
  wire();
  watch(["status", "tree"], onProject);
  watch(["status", "tree"], render);
}

const q = (sel) => root.querySelector(sel);

/// Projekt des Kopfzeilen-Menüs; die Anlage zeigt immer das aktive Projekt
const project = () => state.tree?.projects.find((p) => p.id === state.status?.active?.project?.id) || null;
const activeLineId = () => state.status?.active?.line?.id;

function onProject() {
  const id = state.status?.active?.project?.id;
  if (!id || id === projectId) return;
  projectId = id;
  if (!perProject.has(id)) perProject.set(id, { sel: `l:${activeLineId()}`, closed: new Set(), adding: null, detailOpen: false });
  view = perProject.get(id);
}

/// Element zum Schlüssel der Auswahl
function picked() {
  const p = project();
  const [kind, id] = view.sel.split(":");
  if (kind === "g") {
    const g = p.gateways.find((x) => x.id === Number(id));
    if (g) return { kind: "gateway", g };
  }
  if (kind === "l") {
    for (const g of p.gateways) {
      const l = g.lines.find((x) => x.id === Number(id));
      if (l) return { kind: "line", g, l };
    }
  }
  return { kind: "project", p };
}

/// Zeilen der Liste: Projekt, je Gateway seine Linien (wenn aufgeklappt), Felder für Neues
function items(p) {
  const out = [{ key: "p", kind: "project", p }];
  p.gateways.forEach((g, i) => {
    out.push({ key: `g:${g.id}`, kind: "gateway", g, i, n: p.gateways.length });
    if (!view.closed.has(g.id)) g.lines.forEach((l, k) => out.push({ key: `l:${l.id}`, kind: "line", g, l, i: k, n: g.lines.length }));
    if (view.adding === g.id) out.push({ key: `new-l:${g.id}`, kind: "new", what: "line" });
  });
  out.push(view.adding === "gateway" ? { key: "new-g", kind: "new", what: "gateway" } : { key: "add-g", kind: "add", empty: !p.gateways.length });
  return out;
}

const selectable = () => items(project()).filter((x) => ["project", "gateway", "line"].includes(x.kind));

// ------------------------------------------------------------ Zeichnen

function render() {
  if (!view || !project()) return;
  const p = project();
  if (picked().kind === "project" && view.sel !== "p") view.sel = "p";
  renderToolbar(p);
  const plant = plantHtml(p, state.status);
  q("[data-plant]").hidden = !plant;
  html(q("[data-plant]"), plant);
  list(q("[data-list]"), items(p), (x) => x.key, (x) => rowHtml(x, { sel: view.sel, closed: view.closed, activeId: activeLineId() }));
  q("[data-list]").setAttribute("aria-activedescendant", `an-${view.sel.replace(":", "-")}`);
  renderDetail();
}

function renderToolbar(p) {
  html(
    q("[data-newproj]"),
    addingProject
      ? `<input data-new="project" placeholder="${esc(t("an_new_project_ph"))}" aria-label="${esc(t("an_new_project_ph"))}"><button type="button" data-create="project">${esc(
          t("an_create")
        )}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button>`
      : `<button type="button" class="tonal" data-add-project>${esc(t("an_new_project"))}</button>`
  );
  q("[data-export]").href = apiHref(`projects/${p.id}/export`);
  // der Bericht ist der nächste Schritt, sobald etwas ausgelesen ist (Konzept §3.4)
  q("[data-report]").className = allLines(p).some((l) => l.last_read) ? "" : "quiet";
}

// ------------------------------------------------------------ Details

function renderDetail() {
  const box = q("[data-detail]");
  box.classList.toggle("open", view.detailOpen);
  // wer gerade in ein Feld tippt, verliert nichts: neu gezeichnet wird erst danach
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input:not([type=checkbox]), select, textarea");
  if (typing && box.dataset.sel === view.sel) return;
  box.dataset.sel = view.sel;
  const back = `<button type="button" class="quiet back" data-back>← ${esc(t("tab_project"))}</button>`;
  html(box, back + detailHtml(picked(), { p: project(), activeId: activeLineId(), ports: state.ports }));
}

// ------------------------------------------------------------ Aktionen

function select(key, { focusRow = false, detail = false } = {}) {
  view.sel = key;
  if (detail) view.detailOpen = true;
  render();
  const el = root.querySelector(`#an-${key.replace(":", "-")}`);
  el?.scrollIntoView({ block: "nearest" });
  if (focusRow) q("[data-list]").focus();
}

/// Feld beim Verlassen speichern; Escape setzt es auf den gespeicherten Wert zurück
async function saveField(input) {
  const value = input.dataset.field === "baud" ? Number(input.value) : input.value.trim();
  if (String(value) === input.dataset.saved) return;
  const x = picked();
  try {
    if (input.dataset.kind === "project") {
      const p = x.p;
      const body = { name: p.name, customer: p.customer, site: p.site, technician: p.technician, order_no: p.order_no, remarks: p.remarks };
      await request(`projects/${p.id}`, { method: "PUT", body: { ...body, [input.dataset.field]: value } });
    } else if (input.dataset.kind === "gateway") {
      const g = x.g;
      await request(`gateways/${g.id}`, { method: "PUT", body: { name: g.name, address: g.address, note: g.note, [input.dataset.field]: value } });
    } else {
      const l = x.l;
      const body = { name: l.name, connection: l.connection, baud: l.baud, timeout_ms: l.timeout_ms, [input.dataset.field]: value };
      await request(`lines/${l.id}`, { method: "PUT", body });
      // aktive Linie mit neuem Anschluss: gleich damit verbinden
      if (l.id === activeLineId() && input.dataset.field !== "name") await useLine(l.id);
    }
    input.dataset.saved = String(value);
    input.classList.add("saved");
    setTimeout(() => input.classList.remove("saved"), 900);
    await reload();
  } catch (e) {
    report(e);
  }
}

async function savePlan(kind) {
  const sel = q(`[data-plan="${kind}"]`);
  const time = q(`[data-plan-time="${kind}"]`);
  time.hidden = !TIMED.includes(sel.value);
  const hhmm = time24(time.value);
  time.classList.toggle("invalid", !time.hidden && !hhmm);
  if (!time.hidden && !hhmm) return toast(t("an_time_bad"), { error: true });
  if (hhmm) time.value = hhmm;
  const plan = !sel.value ? null : time.hidden ? sel.value : `${sel.value} ${hhmm}`;
  const x = picked();
  try {
    const r = await request(kind === "project" ? `projects/${x.p.id}/plan` : `lines/${x.l.id}/plan`, { method: "PUT", body: { plan } });
    if (r.warnings?.length) toast(t("plan_saved_warn", r.warnings.join(" ")), { error: true });
    await loadTree();
    loadMeters().catch(() => {}); // geerbte Pläne der Zähler ändern sich mit
  } catch (e) {
    report(e);
  }
}

async function saveThin(select_) {
  const days = Number(select_.value) || null;
  try {
    await request(`projects/${project().id}/thin`, { method: "PUT", body: { days } });
    await loadTree();
  } catch (e) {
    report(e);
  }
}

async function use(id) {
  const l = allLines(project()).find((x) => x.id === id);
  try {
    await useLine(id);
    toast(t("activated", l?.name || ""));
  } catch (e) {
    report(e);
  }
}

async function move(button) {
  try {
    await request(`${button.dataset.move}/${button.dataset.id}/move`, { method: "POST", body: { dir: Number(button.dataset.dir) } });
    await loadTree();
  } catch (e) {
    report(e);
  }
}

async function create(what) {
  const input = root.querySelector(`[data-new="${what}"]`);
  const name = input.value.trim();
  if (!name) return input.focus();
  try {
    if (what === "project") {
      await request("projects", { method: "POST", body: { name } });
      addingProject = false;
      await reload(); // das neue Projekt ist aktiv; die Anlage folgt über den Zustand
    } else if (what === "gateway") {
      const g = await request(`projects/${project().id}/gateways`, { method: "POST", body: { name, address: "", note: "" } });
      view.adding = null;
      await loadTree();
      select(`g:${g.id}`);
    } else {
      const gw = view.adding;
      const l = await request(`gateways/${gw}/lines`, { method: "POST", body: { name, connection: "", baud: 2400 } });
      view.adding = null;
      view.closed.delete(gw);
      await loadTree();
      select(`l:${l.id}`, { detail: true });
      q("[data-field=connection]")?.focus(); // nächster Schritt einer neuen Linie
    }
    toast(t("an_created", name));
  } catch (e) {
    report(e);
  }
}

function startAdding(what) {
  if (what === "project") addingProject = true;
  else view.adding = what;
  render();
  root.querySelector(`[data-new="${what === "project" ? "project" : what === "gateway" ? "gateway" : "line"}"]`)?.focus();
}

function cancelAdding() {
  addingProject = false;
  view.adding = null;
  render();
}

async function remove(button) {
  const x = picked();
  const what = button.dataset.delete;
  const name = (x.l || x.g || x.p).name;
  const question = t({ project: "confirm_project_delete", gateway: "confirm_gw_delete", line: "confirm_ln_delete" }[what], name);
  if (!(await ask(button, question, t("delete")))) return;
  const path = { project: `projects/${x.p?.id}`, gateway: `gateways/${x.g?.id}`, line: `lines/${x.l?.id}` }[what];
  try {
    await request(path, { method: "DELETE" });
    view.sel = what === "line" ? `g:${x.g.id}` : "p";
    view.detailOpen = false;
    await reload();
    toast(t("an_deleted", name));
  } catch (e) {
    report(e);
  }
}

/// Projekt für einen neuen Einsatz leeren (Konzept Bedienung §12): Linien und Einstellungen bleiben
async function clearProject(button) {
  const p = project();
  if (!(await ask(button, t("confirm_project_clear", sum(allLines(p), (l) => l.meters), p.name), t("project_clear").replace(/\s*…$/, "")))) return;
  try {
    await request(`projects/${p.id}/clear`, { method: "POST" });
    await reload();
    loadMeters().catch(() => {});
    toast(t("project_cleared", p.name));
  } catch (e) {
    report(e);
  }
}

async function importProject(file) {
  try {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("project_import_bad"));
    }
    const r = await request("projects/import", { method: "POST", body: data });
    await loadTree();
    const p = state.tree.projects.find((x) => x.id === r.project);
    const first = p && allLines(p)[0];
    if (first) await useLine(first.id);
    toast(t("project_imported", p?.name || ""));
  } catch (e) {
    report(e);
  }
}

async function takeTime() {
  try {
    const r = await request("time", { method: "POST", body: { epoch_ms: Date.now() } });
    toast(t("time_taken", Math.round(r.offset_s)));
    await loadStatus();
  } catch (e) {
    report(e);
  }
}

function toggleGateway(id, show = view.closed.has(id)) {
  if (show) view.closed.delete(id);
  else view.closed.add(id);
  // die Auswahl verschwindet nicht mit einer zugeklappten Linie, sie geht an ihr Gateway
  if (!show && picked().g?.id === id) view.sel = `g:${id}`;
  render();
}

// ------------------------------------------------------------ Bedienung

function wire() {
  const listEl = q("[data-list]");
  root.addEventListener("click", (ev) => {
    const el = ev.target;
    const hit = (sel) => el.closest(sel);
    if (hit("[data-add-project]")) return startAdding("project");
    if (hit("[data-add-gateway]")) return startAdding("gateway");
    if (hit("[data-add-line]")) return startAdding(Number(hit("[data-add-line]").dataset.addLine));
    if (hit("[data-create]")) return create(hit("[data-create]").dataset.create);
    if (hit("[data-cancel-new]")) return cancelAdding();
    if (hit("[data-import]")) return q("[data-import-file]").click();
    if (hit("[data-report]")) return window.open(`report.html?project=${project().id}`, "_blank", "noopener");
    if (hit("[data-take-time]")) return takeTime();
    // nach diesem Klick öffnen, sonst schliesst ihn der Klick gleich wieder
    if (hit("[data-open-settings]")) return setTimeout(() => document.getElementById("settings").click());
    if (hit("[data-use]")) return use(Number(hit("[data-use]").dataset.use));
    if (hit("[data-move]")) return move(hit("[data-move]"));
    if (hit("[data-toggle]")) return toggleGateway(Number(hit("[data-toggle]").dataset.toggle));
    if (hit("[data-delete]")) return remove(hit("[data-delete]"));
    if (hit("[data-clear]")) return clearProject(hit("[data-clear]"));
    if (hit("[data-back]")) {
      view.detailOpen = false;
      render();
      return listEl.focus();
    }
    const r = hit("[role=option]");
    if (r && !hit("input, button, a")) select(r.dataset.key, { detail: true });
  });
  root.addEventListener("change", (ev) => {
    const el = ev.target;
    if (el.matches("[data-op]")) return switchOperation(el, allLines(project()).find((x) => x.id === Number(el.dataset.op)));
    if (el.matches("[data-field]")) return saveField(el);
    if (el.matches("[data-plan]")) return savePlan(el.dataset.plan);
    if (el.matches("[data-plan-time]")) return savePlan(el.dataset.planTime);
    if (el.matches("[data-thin]")) return saveThin(el);
    if (el.matches("[data-import-file]")) {
      const file = el.files[0];
      el.value = "";
      if (file) importProject(file);
    }
  });
  root.addEventListener("keydown", (ev) => {
    const el = ev.target;
    if (el.matches("[data-field]") && ev.key === "Escape") {
      el.value = el.dataset.saved;
      el.blur();
      ev.stopPropagation();
    }
    if (el.matches("input[data-field], input[data-plan-time]") && ev.key === "Enter") el.blur();
    if (el.matches("[data-new]") && ev.key === "Enter") create(el.dataset.new);
    if (el.matches("[data-new]") && ev.key === "Escape") {
      ev.stopPropagation();
      cancelAdding();
    }
  });
  root.addEventListener("focusin", (ev) => {
    if (ev.target.matches("[data-field=connection]") && !state.ports) request("ports").then((ports) => set({ ports })).then(renderPorts, () => {});
  });
  listEl.addEventListener("keydown", onListKey);
  bind("w", () => picked().kind === "line" && use(picked().l.id), t("an_key_use"), open);
}

/// Anschlüsse zur Auswahl erst laden, wenn man das Feld betritt (wie das Formular der Kopfzeile)
function renderPorts() {
  const dl = root.querySelector("#an-ports");
  if (dl) dl.innerHTML = state.ports.map((x) => `<option value="${esc(x.port)}">${esc(x.label || "")}</option>`).join("");
}

/// ↑↓ wählen, ←→ Gateway zu/auf, Enter Hauptaktion (Linie aktiv machen, Gateway auf/zu,
/// Projekt bearbeiten), Alt+↑↓ verschieben
function onListKey(ev) {
  const rows = selectable();
  const i = rows.findIndex((x) => x.key === view.sel);
  const x = picked();
  if ((ev.key === "ArrowDown" || ev.key === "ArrowUp") && ev.altKey) {
    if (x.kind === "project") return;
    ev.preventDefault();
    const b = root.querySelector(`[data-move="${x.kind === "line" ? "lines" : "gateways"}"][data-id="${(x.l || x.g).id}"][data-dir="${ev.key === "ArrowUp" ? -1 : 1}"]`);
    if (b && !b.disabled) move(b);
  } else if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    const next = rows[Math.max(0, Math.min(rows.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))];
    if (next) select(next.key, { focusRow: true });
  } else if ((ev.key === "ArrowLeft" || ev.key === "ArrowRight") && x.kind !== "project") {
    ev.preventDefault();
    toggleGateway(x.g.id, ev.key === "ArrowRight");
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    if (x.kind === "line") use(x.l.id);
    else if (x.kind === "gateway") toggleGateway(x.g.id);
    else q("[data-field=name]")?.focus();
  }
}
