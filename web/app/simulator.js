// Bereich Simulator: oben die Simulation (Auswahl, Zustand, Aktionen), links ihre Zähler, rechts
// der gewählte Zähler. Änderungen sammeln sich und werden mit „Speichern“ übernommen (die
// Simulation startet dabei neu). Vorlagen sind nur zum Ansehen und Kopieren.

import { state, watch } from "./store.js";
import { html, text, list, ask, toast, styles, texts, menu, closeMenu, explain } from "./view.js";
import { loadStatus, loadTree, report } from "./data.js";

texts(
  {
    si_dirty: "Ungespeicherte Änderungen",
    si_discard: "Verwerfen",
    si_in_use: "von der aktiven Linie benutzt",
    si_create: "Anlegen",
    si_new_named: "Neue Simulation",
    si_copy_named: "Kopie",
    si_pick: "Zähler in der Liste wählen (↑↓): hier stehen Adresse, Medium und Verhalten.",
    si_switch_q: "Ungespeicherte Änderungen verwerfen?",
    si_del_meter: "Zähler entfernen",
    si_add_n: (n, m) => `${n} × ${m} hinzufügen`,
    si_hints: (n) => (n === 1 ? "1 Hinweis" : `${n} Hinweise`),
  },
  {
    si_dirty: "Unsaved changes",
    si_discard: "Discard",
    si_in_use: "used by the active line",
    si_create: "Create",
    si_new_named: "New simulation",
    si_copy_named: "Copy",
    si_pick: "Pick a meter in the list (↑↓) to see address, medium and behaviour here.",
    si_switch_q: "Discard unsaved changes?",
    si_del_meter: "Remove meter",
    si_add_n: (n, m) => `Add ${n} × ${m}`,
    si_hints: (n) => (n === 1 ? "1 note" : `${n} notes`),
  }
);

const MAN = { heat: "KAM", cooling: "KAM", heat_old: "KAM", electricity: "ABB", gas: "ELS" };
/// Medium der Simulation → Schlüssel des Servers (für Symbol und Farbe)
const MEDIUM_KEY = { heat: "medium_heat_outlet", heat_old: "medium_heat_outlet", cooling: "medium_cooling_inlet", water: "medium_water", cold_water: "medium_cold_water", warm_water: "medium_warm_water", gas: "medium_gas", electricity: "medium_electricity" };

let root = null;
let sim = null; // /api/sim
let key = null; // gewählte Simulation
let draft = null; // Zähler der eigenen Simulation in Bearbeitung (null bei Vorlagen)
let dirty = false;
let active = null; // Index des gewählten Zählers
const q = (sel) => root.querySelector(sel);

export function mount(pane) {
  styles("simulator");
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <select data-sim aria-label="${esc(t("sim_choose"))}"></select>
      <span data-chips></span>
      <button type="button" data-use></button>
      <div class="menu-host"><button type="button" class="quiet" data-copy-btn>${esc(t("sim_copy"))} …</button><div class="menu form" data-copy-menu hidden></div></div>
      <div class="menu-host"><button type="button" class="quiet" data-new-btn>${esc(t("sim_new"))} …</button><div class="menu form" data-new-menu hidden></div></div>
      <button type="button" class="quiet danger-text" data-del></button>
      <span class="grow"></span>
      <span class="sub" data-count></span>
      ${explain(t("sim_explain"))}
    </div>
    <div class="toolbar sub-bar" data-editbar></div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" aria-hidden="true"><span>${esc(t("sim_col_pa"))}</span><span>${esc(t("sim_col_id"))}</span><span>${esc(t("sim_col_man"))}</span><span>${esc(t("sim_col_medium"))}</span><span>${esc(
          t("sim_col_baud")
        )}</span><span>${esc(t("sim_col_fault"))}</span></div>
        <div class="rows" role="listbox" tabindex="0" data-list></div>
        <div class="empty" data-empty hidden></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>`;
  wire();
  watch(["status"], () => sim && render());
  load();
}

async function load(pick = null) {
  try {
    sim = await request("sim");
  } catch (e) {
    return report(e);
  }
  const conn = sim.line.connection || "";
  const used = conn === "SIMULATOR" ? "praxis" : conn.startsWith("SIMULATOR:") ? conn.slice(10) : null;
  const want = pick || key || used || sim.scenarios[0].key;
  choose(sim.scenarios.some((s) => s.key === want) ? want : sim.scenarios[0].key);
}

const scenario = () => sim.scenarios.find((s) => s.key === key);
const conn = (k) => (k === "praxis" ? "SIMULATOR" : `SIMULATOR:${k}`);
const meters = () => (draft ? draft.meters : scenario().meters);

function choose(k) {
  key = k;
  const s = scenario();
  draft = s.builtin ? null : { name: s.name, meters: s.meters.map((m) => ({ ...m })) };
  setDirty(false);
  active = null;
  render();
}

/// Ungespeicherte Änderungen; verworfen werden sie nur mit „Verwerfen“, nie still mit Escape
function setDirty(on) {
  dirty = on;
}

function discard() {
  choose(key);
}

// ------------------------------------------------------------ Zeichnen

function warnings(ms) {
  const out = [];
  const count = (f) => ms.reduce((m, x) => m.set(x[f], (m.get(x[f]) || 0) + 1), new Map());
  for (const [pa, n] of count("primary")) if (n > 1 && Number(pa) !== 0) out.push(t("sim_dup_pa", pa));
  for (const [id, n] of count("id")) if (n > 1) out.push(t("sim_dup_id", id));
  const zero = ms.filter((m) => Number(m.primary) === 0).length;
  if (zero) out.push(t("sim_pa0", zero));
  return out;
}

function render() {
  if (!sim || !root) return;
  const s = scenario();
  html(q("[data-sim]"), sim.scenarios.map((x) => `<option value="${esc(x.key)}"${x.key === key ? " selected" : ""}>${esc(x.name)}</option>`).join(""));
  q("[data-sim]").value = key;
  const inUse = (state.status?.active?.line?.connection || sim.line.connection) === conn(key);
  html(
    q("[data-chips]"),
    (s.builtin ? `<span class="chip">${esc(t("sim_template"))}</span> ` : "") + (inUse ? `<span class="chip ok">● ${esc(t("si_in_use"))}</span>` : "")
  );
  const use = q("[data-use]");
  text(use, t("sim_use"));
  use.hidden = inUse;
  use.className = dirty ? "tonal" : ""; // bei ungespeicherten Änderungen ist „Speichern“ der Hauptknopf
  const del = q("[data-del]");
  text(del, t("sim_delete"));
  del.hidden = s.builtin;
  // benutzt eine Linie die Simulation, lehnt der Server ab: nicht erst anbieten
  del.disabled = inUse;
  del.title = inUse ? t("si_in_use") : "";
  const ms = meters();
  text(q("[data-count]"), t("sim_count", ms.length, ms.filter((m) => m.fault !== "none").length));
  const warn = warnings(ms);
  html(
    q("[data-editbar]"),
    (s.builtin ? `<span class="sub">${esc(t("sim_builtin_note"))}</span>` : `<button type="button" class="tonal" data-add>${esc(t("sim_add"))}</button>
      <span class="tab-sep" aria-hidden="true"></span>
      <button type="button" class="quiet" data-add-many>${esc(t("sim_add_many"))}</button><input type="number" min="1" max="100" value="10" data-n aria-label="${esc(
        t("sim_add_n")
      )}"><select data-n-medium aria-label="${esc(t("sim_col_medium"))}">${sim.media
        .map((m) => `<option value="${m}"${m === "water" ? " selected" : ""}>${esc(t("sim_m_" + m))}</option>`)
        .join("")}</select>`) +
      // Hinweise sind oft gewollt (Vorlage mit Fehlern): ein Chip, die Liste im Tooltip
      `<span class="grow"></span>${warn.length ? `<span class="chip warn" title="${esc(warn.join("\n"))}">⚠ ${esc(t("si_hints", warn.length))}</span>` : ""}` +
      (dirty ? ` <span class="sub">${esc(t("si_dirty"))}</span><button type="button" class="quiet" data-discard>${esc(t("si_discard"))}</button><button type="button" data-save>${esc(t("sim_save"))}</button>` : "")
  );
  list(q("[data-list]"), ms.map((m, i) => ({ m, i })), ({ i }) => i, row);
  const empty = q("[data-empty]");
  empty.hidden = ms.length > 0;
  text(empty, t("sim_empty"));
  renderDetail();
}

function row({ m, i }) {
  return `<div class="row${i === active ? " active" : ""}" role="option" id="sim-${i}">
    <span class="mono">${m.primary}</span>
    <span class="mono">${esc(m.id)}</span>
    <span class="mono sub">${esc(m.manufacturer)}</span>
    <span class="name">${mediumIcon(MEDIUM_KEY[m.medium], true)}${esc(t("sim_m_" + m.medium))}</span>
    <span class="mono sub">${m.baud ?? esc(t("sim_baud_bus"))}</span>
    <span>${m.fault === "none" ? `<span class="sub">${esc(t("sim_f_none"))}</span>` : `<span class="chip warn">${esc(t("sim_f_" + m.fault))}</span>`}</span>
  </div>`;
}

function renderDetail() {
  const box = q("[data-detail]");
  const m = meters()[active];
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input, select");
  if (typing && box.dataset.shown === `${key}:${active}`) return;
  box.dataset.shown = `${key}:${active}`;
  box.classList.toggle("open", !!m);
  if (!draft) {
    html(box, m ? meterText(m) : `<p class="sub">${esc(t("sim_builtin_note"))}</p>`);
    return;
  }
  if (!m)
    return html(
      box,
      `<div class="fields"><label><span>${esc(t("sim_name"))}</span><input data-name value="${esc(draft.name)}"></label></div><p class="sub">${esc(t("si_pick"))}</p>`
    );
  const sel = (f, opts) => `<select data-f="${f}">${opts.map(([v, l]) => `<option value="${esc(v)}"${String(m[f] ?? "") === String(v) ? " selected" : ""}>${esc(l)}</option>`).join("")}</select>`;
  html(
    box,
    `<button type="button" class="quiet back" data-back>← ${esc(t("tab_sim"))}</button>
    <header class="dhead">${mediumIcon(MEDIUM_KEY[m.medium])}<b>${esc(t("sim_m_" + m.medium))} · ${esc(m.id)}</b></header>
    <div class="fields">
      <label><span>${esc(t("sim_col_pa"))}</span><input data-f="primary" type="number" min="0" max="250" class="mono" value="${m.primary}"></label>
      <label><span>${esc(t("sim_col_id"))}</span><input data-f="id" class="mono" maxlength="8" value="${esc(m.id)}"></label>
      <label><span>${esc(t("sim_col_man"))}</span><input data-f="manufacturer" class="mono" maxlength="3" value="${esc(m.manufacturer)}"></label>
      <label><span>${esc(t("sim_col_medium"))}</span>${sel("medium", sim.media.map((v) => [v, t("sim_m_" + v)]))}</label>
      <label><span>${esc(t("sim_col_baud"))}</span>${sel("baud", [["", t("sim_baud_bus")], ...BAUDS.map((b) => [b, b])])}</label>
      <label><span>${esc(t("sim_col_fault"))}</span>${sel("fault", sim.faults.map((v) => [v, t("sim_f_" + v)]))}</label>
    </div>
    <div class="actions"><span class="grow"></span><button type="button" class="quiet danger-text" data-del-meter>${esc(t("si_del_meter"))}</button></div>`
  );
}

function meterText(m) {
  return `<button type="button" class="quiet back" data-back>← ${esc(t("tab_sim"))}</button>
    <header class="dhead">${mediumIcon(MEDIUM_KEY[m.medium])}<b>${esc(t("sim_m_" + m.medium))} · ${esc(m.id)}</b></header>
    <div class="vals">${[
      [t("sim_col_pa"), m.primary],
      [t("sim_col_id"), m.id],
      [t("sim_col_man"), m.manufacturer],
      [t("sim_col_baud"), m.baud ?? t("sim_baud_bus")],
      [t("sim_col_fault"), t("sim_f_" + m.fault)],
    ]
      .map(([k, v]) => `<div class="vrow"><span>${esc(k)}</span><span class="mono">${esc(v)}</span><span></span></div>`)
      .join("")}</div>
    <p class="sub">${esc(t("sim_builtin_note"))}</p>`;
}

// ------------------------------------------------------------ Aktionen

/// nächster freier Zähler: PA nach der grössten, ID fortlaufend
function next(medium) {
  const ms = draft.meters;
  const pa = Math.min(250, ms.reduce((m, x) => Math.max(m, Number(x.primary) || 0), 0) + 1);
  const ids = new Set(ms.map((m) => m.id));
  let id = ms.reduce((m, x) => Math.max(m, Number(x.id) || 0), 10000000) + 1;
  while (ids.has(String(id).padStart(8, "0"))) id++;
  return { primary: pa, id: String(id).padStart(8, "0"), manufacturer: MAN[medium] || "SEN", version: 1, medium, baud: null, fault: "none" };
}

function changed() {
  setDirty(true);
  render();
}

async function save() {
  try {
    const r = await request(`sim/${encodeURIComponent(key)}`, { method: "PUT", body: { name: draft.name, meters: draft.meters } });
    setDirty(false);
    toast(t("sim_saved", r.restarted));
    await load(key);
    loadStatus().catch(() => {});
  } catch (e) {
    report(e);
  }
}

async function use(button) {
  if (dirty && !(await ask(button, t("si_switch_q"), t("si_discard")))) return;
  try {
    const r = await request(`sim/${encodeURIComponent(key)}/use`, { method: "POST" });
    toast(t("sim_used", r.connection));
    await Promise.all([loadStatus(), loadTree(), load(key)]);
  } catch (e) {
    report(e);
  }
}

async function create(from, name) {
  try {
    const r = await request("sim", { method: "POST", body: { name, from } });
    closeMenu();
    await load(r.key);
  } catch (e) {
    report(e);
  }
}

function nameForm(menuEl, from) {
  const suggest = from ? t("sim_copy_name", scenario().name) : t("sim_new_name");
  html(
    menuEl,
    `<label class="row"><span>${esc(t("sim_name"))}</span><input data-new-name value="${esc(suggest)}" autofocus></label>
    <div class="row end"><button type="button" data-create="${from ? esc(from) : ""}">${esc(t("si_create"))}</button></div>`
  );
  setTimeout(() => menuEl.querySelector("[data-new-name]")?.select(), 0);
}

// ------------------------------------------------------------ Bedienung

function wire() {
  menu(q("[data-copy-btn]"), q("[data-copy-menu]"), { onOpen: () => nameForm(q("[data-copy-menu]"), key) });
  menu(q("[data-new-btn]"), q("[data-new-menu]"), { onOpen: () => nameForm(q("[data-new-menu]"), null) });
  root.addEventListener("click", async (ev) => {
    const b = (sel) => ev.target.closest(sel);
    if (b("[data-use]")) return use(b("[data-use]"));
    if (b("[data-create]")) {
      const panel = b("[data-create]").closest(".menu");
      const name = panel.querySelector("[data-new-name]").value.trim();
      if (name) return create(b("[data-create]").dataset.create || null, name);
      return;
    }
    if (b("[data-del]")) {
      if (await ask(b("[data-del]"), t("confirm_sim_delete", scenario().name), t("delete"))) {
        try {
          await request(`sim/${encodeURIComponent(key)}`, { method: "DELETE" });
          key = null;
          setDirty(false);
          await load();
        } catch (e) {
          report(e);
        }
      }
      return;
    }
    if (b("[data-save]")) return save();
    if (b("[data-discard]")) return discard();
    if (b("[data-add]")) {
      draft.meters.push(next("water"));
      active = draft.meters.length - 1;
      return changed();
    }
    if (b("[data-add-many]")) {
      const n = Math.max(1, Math.min(100, Number(q("[data-n]").value) || 1));
      const medium = q("[data-n-medium]").value;
      for (let k = 0; k < n; k++) draft.meters.push(next(medium));
      return changed();
    }
    if (b("[data-del-meter]")) {
      draft.meters.splice(active, 1);
      active = null;
      return changed();
    }
    if (b("[data-back]")) {
      active = null;
      return render();
    }
    const r = b("[data-list] [role=option]");
    if (r) {
      active = Number(r.dataset.key);
      render();
    }
  });
  root.addEventListener("keydown", (ev) => {
    if (ev.target.matches("[data-new-name]") && ev.key === "Enter") ev.target.closest(".menu").querySelector("[data-create]").click();
  });
  root.addEventListener("change", async (ev) => {
    const el = ev.target;
    if (el.matches("[data-sim]")) {
      const want = el.value;
      el.value = key;
      if (dirty && !(await ask(el, t("si_switch_q"), t("si_discard")))) return;
      return choose(want);
    }
    if (el.matches("[data-name]")) {
      draft.name = el.value.trim() || draft.name;
      return changed();
    }
    if (!el.matches("[data-f]") || !draft || active == null) return;
    const m = draft.meters[active];
    const f = el.dataset.f;
    if (f === "primary") m.primary = Math.max(0, Math.min(250, Number(el.value) || 0));
    else if (f === "baud") m.baud = el.value ? Number(el.value) : null;
    else if (f === "manufacturer") m.manufacturer = el.value.trim().toUpperCase();
    else if (f === "id") m.id = el.value.trim().padStart(8, "0");
    else m[f] = el.value;
    changed();
  });
  q("[data-list]").addEventListener("keydown", (ev) => {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    ev.preventDefault();
    const n = meters().length;
    if (!n) return;
    active = active == null ? 0 : Math.max(0, Math.min(n - 1, active + (ev.key === "ArrowDown" ? 1 : -1)));
    render();
    root.querySelector(`#sim-${active}`)?.scrollIntoView({ block: "nearest" });
  });
}
