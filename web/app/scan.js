// Bereich Scan: Suchzeile oben, Treffer links (zeilenweise nachgeführt, auch während des Scans),
// Treffer mit Ursache und Abhilfe rechts (Konzept neue Oberfläche §3.2, Schritt 2).
// Auswertung (Auffälligkeiten, Merken, Beheben, Vergleich) gemeinsam: js/scanlogic.js.

import { state, set, watch, activeLine } from "./store.js";
import { html, text, list, ask, toast, styles, menu } from "./view.js";
import { bind } from "./keys.js";
import { loadMeters, loadTree, loadStatus, report } from "./data.js";
import "./scan-texte.js";
import { fillBaudMenu, checkBauds } from "./scan-baudraten.js";
import { sorter } from "./sortieren.js";


const FILTERS = ["all", "unique", "attention"];

/// Einstellungen je Linie: Suche (Art, Bereich, Baudraten), Filter, gewählter Treffer, Plan
const perLine = new Map();
let root = null;
let view = null;
let lineId = null;
/// Wiederholungen je Adresse im Primärscan (der Server nimmt höchstens 5)
const RETRIES = [0, 1, 2, 3];
/// Spalten der Trefferliste und wonach sie sortieren; leere Werte stehen immer unten
const COLUMNS = [
  ["pa", () => "PA", (h) => h.primary_address],
  ["baud", () => "Bd", (h) => h.baud],
  ["meter", () => t("col_meter"), (h) => hitName(h)],
  ["id", () => "ID", (h) => h.id || null],
  // Probleme zuerst
  ["state", () => t("n_col_state"), (h) => ({ collision: 0, no_data: 1, ok: 2 })[h.status]],
];
// gilt für alle Linien wie früher
const sort = sorter("hit_sort", COLUMNS, { title: () => t("sort_title"), of: (row) => row.h });
let job = null; // Scan der Linie (laufend oder letzter), wie /api/job bzw. /api/last-scan
let polling = false;
const open = () => state.area === "scan";
const q = (sel) => root.querySelector(sel);

export function mount(pane) {
  styles("scan");
  root = pane;
  root.innerHTML = `
    <div class="toolbar scanbar">
      <div class="seg" role="group" data-kinds></div>
      <span data-range></span>
      <div class="menu-host"><button type="button" class="field" data-baud-btn aria-haspopup="true"></button><div class="menu form" data-baud-menu hidden></div></div>
      <span class="sub" data-estimate></span>
      <span class="grow"></span>
      <span class="sub" data-progress></span>
      <button type="button" class="quiet" data-resume hidden></button>
      <button type="button" data-start></button>
      <p class="explain" data-explain></p>
    </div>
    <div class="toolbar resultbar" data-resultbar hidden>
      <b data-meta></b><span data-changes></span>
      <div class="seg" role="group" data-filters></div>
      <span class="grow"></span>
      <span class="modes"><span class="sub">${esc(t("remember_via"))}</span><div class="seg" role="group" data-modes></div></span>
      <span data-main></span>
    </div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" data-head></div>
        <div class="rows" role="listbox" tabindex="0" aria-label="${esc(t("tab_scan"))}" data-list></div>
        <div class="empty" data-empty></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>`;
  wire();
  watch(["status"], onLine);
  watch(["status", "meters", "tree"], render);
}

// ------------------------------------------------------------ Daten

async function onLine() {
  const id = state.status?.active?.line?.id;
  if (!id || id === lineId) return;
  lineId = id;
  if (!perLine.has(id)) perLine.set(id, { kind: "primary", from: 0, to: 250, retries: 1, prefix: "", bauds: null, filter: "all", active: null, plan: null, baudCheck: null });
  view = perLine.get(id);
  job = null;
  const running = await request("job").catch(() => null);
  job = running?.running && ["primary", "secondary"].includes(running.kind) ? running : await request("last-scan").catch(() => null);
  render();
  if (job?.running) poll();
}

const ctx = () => ({
  hits: job?.hits || [],
  busBaud: state.status?.connection?.baud || activeLine()?.baud || 2400,
  mode: state.status?.active?.line?.addressing || "primary",
});

const bauds = () => view.bauds || [ctx().busBaud];

/// Fortschritt abfragen, Treffer zeilenweise nachführen; am Ende Auffälligkeiten bei der Linie speichern
async function poll() {
  if (polling) return;
  polling = true;
  try {
    while (job?.running) {
      await new Promise((r) => setTimeout(r, 600));
      job = await request("job");
      render();
    }
    await saveIssues();
    loadTree().catch(() => {});
  } finally {
    polling = false;
    loadStatus().catch(() => {});
  }
}

async function saveIssues() {
  if (!job || job.kind === "read_all") return;
  // Anzahl wie im Filter „Auffällig“ (betroffene Treffer); die Erklärungen fassen manche zusammen
  const c = ctx();
  await request(`lines/${lineId}/issues`, { method: "PUT", body: { count: scanLogic.attention(c).size, items: scanLogic.issues(c) } }).catch(() => {});
}

async function start(body = null) {
  if (job?.running) return request("job/cancel", { method: "POST" }).catch(report);
  view.plan = null;
  view.active = null;
  try {
    job =
      view.kind === "primary"
        ? await request("scan/primary", { method: "POST", body: body || { from: view.from, to: view.to, retries: view.retries, bauds: bauds() } })
        : await request("scan/secondary", { method: "POST", body: body || { bauds: bauds(), prefix: view.prefix } });
    // Statusleiste gleich auf „Bus arbeitet“, nicht erst bei der nächsten Abfrage
    loadStatus().catch(() => {});
    render();
    poll();
  } catch (e) {
    report(e);
  }
}

// ------------------------------------------------------------ Zeichnen

/// Zustand eines Treffers als Wort und Farbe: [klasse, text, titel]
function hitState(h, a, i) {
  const c = ctx();
  if (h.status === "collision") return ["err", t("st_collision"), h.primary_address != null ? t("warn_collision_pa", h.primary_address, h.baud) : t("warn_collision_id", h.id)];
  if (h.status === "no_data") {
    const [what, tip] = scanLogic.explain(h.reason);
    return ["err", t("st_no_data"), `${what} ${tip}`];
  }
  if (a.rowDupPa.has(i)) return ["warn", `PA ${h.primary_address} ${t("sc_not_unique")}`, t("warn_fix_hint")];
  if (h.baud && h.baud !== c.busBaud) return ["warn", `${h.baud} Bd`, t("other_baud_title", h.baud, c.busBaud)];
  if (a.rowDupIdSame.has(i)) return ["warn", t("sc_same_id"), t("plan_manual_sameid", h.id)];
  if (scanLogic.listed(h, state.meters || [], c)) return ["ok", t("sc_in_list"), ""];
  const r = scanLogic.rememberAddress(h, c);
  return r.address ? ["info", t("sc_new"), ""] : ["warn", t("sc_not_unique"), r.reason];
}

function shown(a) {
  const att = scanLogic.attention(ctx(), a);
  return sort.sorted((job?.hits || []).map((h, i) => ({ h, i })).filter(({ i }) => view.filter === "all" || (view.filter === "attention") === att.has(i)));
}

/// Klick auf eine Spalte: auf → ab → wie gescannt
function toggleSort(col) {
  sort.toggle(col);
  render();
}

function render() {
  if (!view) return;
  const c = ctx();
  const a = scanLogic.analyze(c);
  const running = !!job?.running;
  const connected = !!state.status?.connected;

  // Suchzeile
  html(q("[data-kinds]"), ["primary", "secondary"].map((k) => `<button type="button" data-kind="${k}" aria-pressed="${view.kind === k}">${esc(t(k === "primary" ? "scan_kind_pa" : "scan_kind_sa"))}</button>`).join(""));
  html(
    q("[data-range]"),
    view.kind === "primary"
      ? `<label class="inline"><span class="sub">${esc(t("sc_from_to"))}</span><input type="number" min="0" max="250" data-from value="${view.from}"><span class="sub">–</span><input type="number" min="0" max="250" data-to value="${view.to}" aria-label="${esc(t("sc_to"))}"></label>
        <label class="inline" title="${esc(t("sc_retries_title"))}"><span class="sub">${esc(t("sc_retries"))}</span><select data-retries>${RETRIES.map((n) => `<option value="${n}"${n === view.retries ? " selected" : ""}>${n}</option>`).join("")}</select></label>`
      : `<label class="inline"><span class="sub">${esc(t("scan_prefix"))}</span><input data-prefix class="mono" maxlength="7" value="${esc(view.prefix)}" placeholder="${esc(t("scan_prefix_ph"))}"></label>`
  );
  text(q("[data-explain]"), t(view.kind === "primary" ? "sc_explain_primary" : "sc_explain_secondary"));
  text(q("[data-baud-btn]"), t("sc_baud_menu", bauds().join(", ")));
  text(
    q("[data-estimate]"),
    view.kind === "primary" && state.status?.connection ? t("estimate", t("duration", scanLogic.duration({ from: view.from, to: view.to, retries: view.retries, bauds: bauds(), conn: state.status.connection }))) : ""
  );
  const go = q("[data-start]");
  text(go, running ? t("cancel") : t(view.kind === "primary" ? "scan_primary_start" : "scan_secondary_start"));
  go.className = running ? "quiet" : view.plan?.done ? "tonal" : "";
  go.disabled = !connected && !running;
  go.title = connected ? `${go.textContent} (S)` : t("idle_off");
  text(q("[data-progress]"), running ? t("sc_running", job.done, job.total, job.hits.length) : !connected ? t("idle_off") : "");
  const resume = q("[data-resume]");
  resume.hidden = !(view.kind === "secondary" && job?.kind === "secondary" && !running && job.resume);
  if (!resume.hidden) text(resume, t("scan_resume", job.resume.mask.replace(/F+$/, "") || "0", job.resume.baud || ""));

  // Ergebnis
  const bar = q("[data-resultbar]");
  bar.hidden = !job;
  const rows = shown(a);
  if (job) {
    text(q("[data-meta]"), `${t("job_" + job.kind)} · ${t("found", job.hits.length)}`);
    const chg = scanLogic.changes(job);
    const n = (k) => (chg ? [...chg.rows.values()].filter((r) => r.kind === k).length : 0);
    html(
      q("[data-changes]"),
      chg && n("new") + n("changed") + chg.missing.length
        ? [n("new") && `<span class="chip">${esc(t("chg_n_new", n("new")))}</span>`, chg.missing.length && `<span class="chip warn" data-missing title="${esc(chg.missing.map(hitLabel).join("\n"))}">${esc(t("chg_n_missing", chg.missing.length))}</span>`, n("changed") && `<span class="chip">${esc(t("chg_n_changed", n("changed")))}</span>`].filter(Boolean).join(" ")
        : ""
    );
    const att = scanLogic.attention(c, a).size;
    const counts = { all: job.hits.length, attention: att, unique: job.hits.length - att };
    html(q("[data-filters]"), FILTERS.map((k) => `<button type="button" data-filter="${k}" aria-pressed="${view.filter === k}">${esc(t("hf_" + k))} <span class="n${k === "attention" && counts[k] ? " warn" : ""}">${counts[k]}</span></button>`).join(""));
    html(q("[data-modes]"), ["primary", "secondary"].map((m) => `<button type="button" data-mode="${m}" aria-pressed="${c.mode === m}" title="${esc(t(m === "primary" ? "mode_pa_title" : "mode_sa_title"))}">${m === "primary" ? "PA" : "ID"}</button>`).join(""));
    html(q("[data-main]"), running ? "" : mainAction(c, a));
  }
  html(q("[data-head]"), sort.headHtml());
  list(q("[data-list]"), rows, ({ i }) => i, ({ h, i }) => row(h, i, a));
  const empty = q("[data-empty]");
  empty.hidden = rows.length > 0;
  html(empty, !job ? esc(t("sc_never")) : running ? "" : job.hits.length ? esc(t("meters_none_match")) : esc(t("sc_none_found")));
  renderDetail(c, a);
  set({ scanNew: { line: lineId, n: job && !running ? newOnes(c).length : 0 } });
}

/// Genau ein Hauptknopf für das Ergebnis: beheben, sonst neue merken, sonst „alle in Liste“
function mainAction(c, a) {
  if (view.plan?.done) return `<button type="button" data-start-check>${esc(t("sc_check"))}</button>`;
  const steps = scanLogic.planSteps(c, a).steps.length;
  if (steps) return `<button type="button" data-fix>${esc(t("sc_fix_n", steps))}</button>`;
  const fresh = newOnes(c).length;
  // Kollision im Primärscan = doppelte PA: erst der Sekundärscan macht sie behebbar; merken bleibt daneben möglich
  const doubled = job.kind === "primary" ? c.hits.filter((h) => h.status === "collision" && h.primary_address != null).length : 0;
  if (doubled)
    return `${fresh ? `<button type="button" class="quiet" data-merge>${esc(t("sc_merge_n", fresh))}</button>` : ""}<button type="button" data-secondary title="${esc(t("sc_resolve_title"))}">${esc(t("sc_resolve_n", doubled))}</button>`;
  if (fresh) return `<button type="button" data-merge title="${esc(t("sc_key_merge"))} (M)">${esc(t("sc_merge_n", fresh))}</button>`;
  const ok = c.hits.some((h) => h.status === "ok");
  return ok ? `<span class="ok-text">${esc(t("sc_all_listed"))}</span> <a href="#/meters">${esc(t("sc_to_meters"))} →</a>` : "";
}

/// Treffer kurz: PA und ID, z.B. für „fehlt seit dem letzten Scan“
const hitLabel = (h) => [h.primary_address != null ? `PA ${h.primary_address}` : "", h.id ? `ID ${h.id}` : ""].filter(Boolean).join(" · ");

const newOnes = (c) => scanLogic.rememberable(c.hits, c).filter((h) => !scanLogic.listed(h, state.meters || [], c));

/// Name wie in der Zählerliste, sonst Medium · Hersteller
function hitName(h) {
  const known = h.id && (state.meters || []).find((m) => m.meter_id === h.id);
  return known?.name || (h.medium ? [label(h.medium), h.manufacturer].filter(Boolean).join(" · ") : t(h.status === "collision" ? "hit_many" : "hit_unknown"));
}

function row(h, i, a) {
  const [cls, word, title] = hitState(h, a, i);
  const name = hitName(h);
  const chg = scanLogic.changes(job)?.rows.get(i);
  return `<div class="row${i === view.active ? " active" : ""}" role="option" id="hit-${i}">
    <span class="mono">${h.primary_address ?? "–"}</span>
    <span class="mono sub">${h.baud || ""}</span>
    <span class="name">${mediumIcon(h.medium, true)}<span class="cut">${esc(name)}</span></span>
    <span class="mono sub">${esc(h.id || "")}${h.version != null ? ` · v${h.version}` : ""}</span>
    <span><span class="chip ${cls === "info" ? "" : cls}" title="${esc(title)}">${esc(word)}</span>${chg ? ` <span class="chip">${esc(chg.text)}</span>` : ""}</span>
  </div>`;
}

function renderDetail(c, a) {
  const box = q("[data-detail]");
  if (view.plan) return html(box, planView());
  const h = job?.hits[view.active];
  box.classList.toggle("open", !!h);
  if (!h) return html(box, `<p class="sub">${esc(t("sc_pick"))}</p>`);
  const [cls, word, title] = hitState(h, a, view.active);
  const r = scanLogic.rememberAddress(h, c);
  const listed = scanLogic.listed(h, state.meters || [], c);
  const actions = [];
  if (h.status === "collision" && h.primary_address != null) actions.push(`<button type="button" class="tonal" data-secondary>${esc(t("start_secondary"))}</button>`);
  if (a.rowDupPa.has(view.active) && h.secondary_address) actions.push(`<button type="button" class="tonal" data-free-pa>${esc(t("sc_fix_free"))}</button>`);
  if (h.status === "ok" && h.baud && h.baud !== c.busBaud && h.secondary_address) actions.push(`<button type="button" class="tonal" data-fix-baud>${esc(t("sc_fix_baud", c.busBaud))}</button>`);
  if (h.status === "ok" && r.address && !listed) actions.push(`<button type="button" data-merge-one>${esc(t("remember"))}</button>`);
  if (h.status === "ok") actions.push(`<button type="button" class="quiet" data-read-hit>${esc(t("read"))}</button>`);
  const vals = view.values?.i === view.active ? view.values.list : null;
  html(
    box,
    `<button type="button" class="quiet back" data-back>← ${esc(t("tab_scan"))}</button>
    <header class="dhead">${mediumIcon(h.medium)}<div><b>${esc(h.id ? `ID ${h.id}` : h.primary_address != null ? `PA ${h.primary_address}` : "?")}</b>
      <div class="sub mono">${esc([h.primary_address != null ? `PA ${h.primary_address}` : "", h.baud ? `${h.baud} Bd` : "", h.manufacturer, h.medium ? label(h.medium) : "", h.version != null ? `v${h.version}` : ""].filter(Boolean).join(" · "))}</div></div></header>
    <p class="chip ${cls === "info" ? "" : cls} wide">${esc(word)}${title ? ` – ${esc(title)}` : ""}</p>
    ${!r.address && h.status === "ok" && !listed ? `<p class="sub">${esc(r.reason)}</p>` : ""}
    <div class="actions">${actions.join("")}</div>
    ${h.secondary_address ? `<p class="sub mono">SA ${esc(h.secondary_address)}</p>` : ""}
    ${vals ? `<div class="vals">${vals.map((v) => `<div class="vrow"><span>${esc(label(v.quantity))}${v.storage ? ` · ${v.storage}` : ""}</span><span class="mono">${esc(v.value_text)} ${esc(v.unit)}</span><span></span></div>`).join("")}</div>` : ""}
    ${h.raw ? `<details><summary class="sub">${esc(t("sc_telegram"))}</summary><p class="mono raw">${esc(h.raw)}</p></details>` : ""}`
  );
}

function planView() {
  const p = view.plan;
  const desc = (s) => (s.kind === "baud" ? t("plan_baud", s.name, s.from, s.to) : t("plan_pa", s.name, s.from, s.to ?? "…"));
  const todo = p.steps.filter((s) => !s.skip).length;
  // freie Adressen werden noch gesucht (fragt den Bus ab): erst danach ausführen
  const searching = p.steps.some((s) => s.kind === "pa" && s.to == null);
  return `<header class="dhead"><b>${esc(t("sc_plan_head"))}</b></header>
    <div class="vals">${p.steps
      .map(
        (s, k) => `<label class="vrow"><span>${esc(desc(s))}</span><span class="${s.state === "err" ? "err-text" : s.state === "ok" ? "ok-text" : "sub"}">${
          s.state === "ok" ? "✓" : s.state === "err" ? esc(s.error) : s.state === "run" ? "…" : ""
        }</span><input type="checkbox" data-step="${k}"${s.skip ? "" : " checked"}${p.running || p.done ? " disabled" : ""}></label>`
      )
      .join("")}</div>
    ${p.manual.length ? `<p class="sub"><b>${esc(t("sc_plan_manual"))}</b></p><ul class="manual">${p.manual.map((m) => `<li>${esc(m)}</li>`).join("")}</ul>` : ""}
    <div class="actions">${
      p.done
        ? `<span class="sub">${esc(t("plan_done", p.steps.filter((s) => s.state === "ok").length, p.steps.filter((s) => s.state).length))}</span>`
        : `${searching ? `<span class="sub"><span class="dot busy"></span> ${esc(t("plan_busy_free"))}</span>` : ""}<button type="button" data-plan-run${todo && !p.running && !searching ? "" : " disabled"}>${esc(t("plan_run", todo))}</button>`
    }<button type="button" class="quiet" data-plan-close${p.running ? " disabled" : ""}>${esc(t(p.done ? "plan_close" : "cancel"))}</button></div>`;
}

// ------------------------------------------------------------ Aktionen

async function mergeNew(hits) {
  const c = ctx();
  const list_ = hits.map((h) => scanLogic.meterFromHit(h, c));
  try {
    const r = await request("meters/import", { method: "POST", body: { meters: list_ } });
    toast(t("remembered_all", r.added, list_.length - r.added));
    await loadMeters();
    loadTree().catch(() => {});
  } catch (e) {
    report(e);
  }
}

async function openPlan() {
  const c = ctx();
  const p = scanLogic.planSteps(c);
  view.plan = { ...p, running: false, done: false };
  render();
  // freie Adressen vorher suchen, damit der Plan konkrete Zahlen zeigt
  const skip = new Set(c.hits.map((h) => h.primary_address).filter((x) => x != null && x > 0));
  for (const s of p.steps.filter((x) => x.kind === "pa")) {
    const r = await request("free-address", { method: "POST", body: { from: 1, skip: [...skip] } }).catch(report);
    if (!r) return;
    s.to = r.address;
    skip.add(r.address);
    render();
  }
}

async function runPlan() {
  const p = view.plan;
  const todo = p.steps.filter((s) => !s.skip).sort((x, y) => (x.kind === "baud" ? 0 : 1) - (y.kind === "baud" ? 0 : 1));
  p.running = true;
  const switched = new Set();
  for (const s of todo) {
    s.state = "run";
    render();
    try {
      if (s.kind === "baud") {
        await request("switch-baud", { method: "POST", body: { address: s.target, baud: s.to, from_baud: s.from } });
        switched.add(s.target);
      } else await request("set-address", { method: "POST", body: { address: s.target, new_address: s.to, from_baud: switched.has(s.target) ? null : s.fromBaud } });
      s.state = "ok";
    } catch (e) {
      s.state = "err";
      s.error = e.message;
    }
  }
  p.running = false;
  p.done = true;
  // der Server hat Trefferliste und Zählerliste nachgeführt
  job = await request("last-scan").catch(() => job);
  await loadMeters().catch(() => {});
  render();
  await saveIssues();
  loadTree().catch(() => {}); // Statusleiste und Anlage zeigen die neue Zahl
}

/// Einen Treffer beheben (freie PA bzw. Bus-Baudrate), mit Rückfrage am Knopf
async function fixOne(button, kind) {
  const c = ctx();
  const h = job.hits[view.active];
  const name = h.id || h.secondary_address;
  try {
    if (kind === "pa") {
      const skip = c.hits.map((x) => x.primary_address).filter((x) => x != null && x > 0);
      const { address } = await request("free-address", { method: "POST", body: { from: 1, skip } });
      if (!(await ask(button, t("sc_q_pa", name, h.primary_address, address), t("sc_change")))) return;
      await request("set-address", { method: "POST", body: { address: h.secondary_address, new_address: address, from_baud: h.baud !== c.busBaud ? h.baud : null } });
    } else {
      if (!(await ask(button, t("sc_q_baud", name, h.baud, c.busBaud), t("sc_change")))) return;
      await request("switch-baud", { method: "POST", body: { address: h.secondary_address, baud: c.busBaud, from_baud: h.baud } });
    }
    job = await request("last-scan").catch(() => job);
    await loadMeters().catch(() => {});
    render();
    await saveIssues();
    loadTree().catch(() => {}); // Statusleiste und Anlage zeigen die neue Zahl
  } catch (e) {
    report(e);
  }
}

async function readHit() {
  const h = job.hits[view.active];
  const i = view.active;
  try {
    const r = await request("read", { method: "POST", body: { address: scanLogic.address(h, ctx()), baud: h.baud !== ctx().busBaud ? h.baud : null } });
    view.values = { i, list: r.records.filter((x) => x.value != null) };
  } catch (e) {
    report(e);
  }
  render();
}

async function setMode(button, mode) {
  const c = ctx();
  if (mode === c.mode) return;
  const n = (state.meters || []).length;
  if (n && !(await ask(button, t("sc_mode_q", mode === "primary" ? "PA" : "ID", n), t("sc_change")))) return;
  try {
    await request(`lines/${lineId}/addressing`, { method: "PUT", body: { mode } });
    await Promise.all([loadStatus(), loadMeters()]);
  } catch (e) {
    report(e);
  }
}

function select(i, focus = false) {
  view.active = i;
  view.plan = null;
  render();
  const el = root.querySelector(`#hit-${i}`);
  el?.scrollIntoView({ block: "nearest" });
  if (el) q("[data-list]").setAttribute("aria-activedescendant", el.id);
  if (focus) q("[data-list]").focus();
}

// ------------------------------------------------------------ Bedienung

function wire() {
  root.addEventListener("click", async (ev) => {
    const el = ev.target;
    const b = (sel) => el.closest(sel);
    if (b("[data-sort]")) return toggleSort(b("[data-sort]").dataset.sort);
    if (b("[data-kind]")) return (view.kind = b("[data-kind]").dataset.kind), render();
    if (b("[data-filter]")) return (view.filter = b("[data-filter]").dataset.filter), render();
    if (b("[data-mode]")) return setMode(b("[data-mode]"), b("[data-mode]").dataset.mode);
    if (b("[data-start]")) return start();
    if (b("[data-start-check]")) return start();
    if (b("[data-resume]")) return start({ bauds: bauds(), prefix: view.prefix, resume: job.resume });
    if (b("[data-baud-check]")) return checkBauds(q("[data-baud-menu]"), view, ctx().busBaud).then(render);
    if (b("[data-merge]")) return mergeNew(newOnes(ctx()));
    if (b("[data-merge-one]")) return mergeNew([job.hits[view.active]]);
    if (b("[data-fix]")) return openPlan();
    if (b("[data-plan-run]")) return runPlan();
    if (b("[data-plan-close]")) return (view.plan = null), render();
    if (b("[data-free-pa]")) return fixOne(b("[data-free-pa]"), "pa");
    if (b("[data-fix-baud]")) return fixOne(b("[data-fix-baud]"), "baud");
    if (b("[data-secondary]")) return (view.kind = "secondary"), start();
    if (b("[data-read-hit]")) return readHit();
    if (b("[data-back]")) return (view.active = null), render();
    const r = b("[data-list] [role=option]");
    if (r) select(Number(r.dataset.key));
  });
  root.addEventListener("change", (ev) => {
    const el = ev.target;
    if (el.matches("[data-from]")) view.from = Math.max(0, Math.min(250, Number(el.value) || 0));
    if (el.matches("[data-to]")) view.to = Math.max(0, Math.min(250, Number(el.value) || 250));
    if (el.matches("[data-retries]")) view.retries = Number(el.value);
    if (el.matches("[data-prefix]")) view.prefix = el.value.replace(/\D/g, "").slice(0, 7);
    if (el.matches("[data-baud]")) view.bauds = [...q("[data-baud-menu]").querySelectorAll("[data-baud]:checked")].map((x) => Number(x.value));
    if (el.matches("[data-step]")) view.plan.steps[Number(el.dataset.step)].skip = !el.checked;
    render();
  });
  q("[data-list]").addEventListener("keydown", (ev) => {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    ev.preventDefault();
    const rows = shown(scanLogic.analyze(ctx()));
    const k = rows.findIndex((x) => x.i === view.active);
    const next = rows[Math.max(0, Math.min(rows.length - 1, k + (ev.key === "ArrowDown" ? 1 : -1)))];
    if (next) select(next.i, true);
  });
  menu(q("[data-baud-btn]"), q("[data-baud-menu]"), { onOpen: () => fillBaudMenu(q("[data-baud-menu]"), view, ctx().busBaud) });
  bind("s", () => !job?.running && start(), t("sc_key_scan"), open);
  bind("m", () => mergeNew(newOnes(ctx())), t("sc_key_merge"), open);
}
