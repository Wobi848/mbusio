// Bereich Werkzeuge: ein Ziel oben für alle Werkzeuge (Zähler aus der Liste oder Adresse von Hand),
// darunter Auslesen, Primäradresse, Baudrate, Reset; daneben Telegramm-Log, Rohbytes und Analyse
// (Konzept neue Oberfläche §3.2).

import { state, watch, activeLine } from "./store.js";
import { html, text, list, ask, toast, styles, texts } from "./view.js";
import { bind, onEscape } from "./keys.js";
import { loadMeters, report } from "./data.js";

texts(
  {
    wz_target: "Ziel",
    wz_target_ph: "Zähler suchen oder Adresse (PA, ID, Sekundär)",
    wz_use_address: (a) => `Adresse ${a} verwenden`,
    wz_no_target: "Kein Ziel · Zähler wählen oder Adresse eingeben",
    wz_no_match: "Kein Zähler passt · PA (0–250), ID (8 Ziffern) oder Sekundäradresse (16 Hex) eingeben",
    wz_clear_target: "Ziel lösen",
    wz_sim: "Simulator",
    wz_sim_title: "Virtuelle Anlage ohne Hardware: üben, testen, vorführen",
    wz_locked_target: "gesperrt: kein Ziel",
    wz_locked_conn: "gesperrt: nicht verbunden",
    wz_read_what: "Werte und Rohtelegramme",
    wz_from_baud: (b) => `läuft mit ${b} Bd, nicht wie der Bus`,
    wz_free: "Freie suchen",
    wz_free_found: (n) => `PA ${n} ist frei`,
    wz_result_empty: "Noch nichts ausgelesen. Ziel wählen, dann „Auslesen“ (R).",
    wz_raw_frames: "Rohtelegramme",
    wz_not_listed: "nicht in der Zählerliste",
    wz_listed: "in der Zählerliste",
    wz_log_empty: "Noch keine Telegramme. Jede Anfrage an den Bus erscheint hier.",
    wz_close: "Schliessen",
    wz_key_read: "Ziel auslesen",
  },
  {
    wz_target: "Target",
    wz_target_ph: "Search meter or address (PA, ID, secondary)",
    wz_use_address: (a) => `Use address ${a}`,
    wz_no_target: "No target · pick a meter or enter an address",
    wz_no_match: "No meter matches · enter PA (0–250), ID (8 digits) or secondary address (16 hex)",
    wz_clear_target: "Clear target",
    wz_sim: "Simulator",
    wz_sim_title: "Virtual plant without hardware: practise, test, demonstrate",
    wz_locked_target: "locked: no target",
    wz_locked_conn: "locked: not connected",
    wz_read_what: "Values and raw telegrams",
    wz_from_baud: (b) => `runs at ${b} Bd, not like the bus`,
    wz_free: "Find free",
    wz_free_found: (n) => `PA ${n} is free`,
    wz_result_empty: "Nothing read yet. Pick a target, then “Read” (R).",
    wz_raw_frames: "Raw telegrams",
    wz_not_listed: "not in the meter list",
    wz_listed: "in the meter list",
    wz_log_empty: "No telegrams yet. Every request to the bus appears here.",
    wz_close: "Close",
    wz_key_read: "Read target",
  }
);

const BAUDS = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];
const LOG_EVERY = 1000;
const LOG_KEEP = 1000;
const PICKS_MAX = 50;
const ADDRESS = /^(\d{1,3}|\d{8}|[0-9a-f]{16})$/i;

/// Ziel und letzte Auslesung je Linie (bleiben beim Wechsel)
const perLine = new Map();
let root = null;
let view = null;
let lineId = null;
let picks = []; // Einträge der Zielauswahl: { meter } oder { address }
let pickAt = 0;
let releasePicks = null;
let log = []; // Telegramm-Log, älteste zuerst
let logSeq = 0;

const open = () => state.area === "werkzeuge";
const q = (sel) => root.querySelector(sel);

export function mount(pane) {
  styles("werkzeuge");
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <b class="eyebrow">${esc(t("wz_target"))}</b>
      <div class="pick">
        <input type="search" data-search data-target-input autocomplete="off" role="combobox" aria-expanded="false" aria-controls="wz-picks"
          placeholder="${esc(t("wz_target_ph"))}" aria-label="${esc(t("wz_target"))}">
        <div class="menu" id="wz-picks" role="listbox" data-picks hidden></div>
      </div>
      <span class="tstate" data-tstate></span>
      <button type="button" class="icon" data-clear title="${esc(t("wz_clear_target"))}" aria-label="${esc(t("wz_clear_target"))}">×</button>
      <span class="grow"></span>
      <a class="quiet" href="#/simulator" data-sim-link title="${esc(t("wz_sim_title"))}">${esc(t("wz_sim"))} →</a>
    </div>
    <div class="tools">
      <fieldset class="tool" data-tool>
        <h3>${esc(t("read"))}</h3>
        <p class="sub" data-now="read"></p>
        <div class="trow"><button type="button" data-read title="${esc(t("read"))} (R)">${esc(t("read"))}</button></div>
      </fieldset>
      <fieldset class="tool" data-tool>
        <h3>${esc(t("set_primary"))}</h3>
        <p class="sub" data-now="pa"></p>
        <div class="trow">
          <input type="number" min="0" max="250" class="mono" data-new-pa aria-label="${esc(t("new_primary"))}" placeholder="${esc(t("new_primary"))}">
          <button type="button" class="quiet" data-free>${esc(t("wz_free"))}</button>
          <button type="button" class="tonal" data-set-pa>${esc(t("set"))}</button>
        </div>
      </fieldset>
      <fieldset class="tool" data-tool>
        <h3>${esc(t("baud_title"))}</h3>
        <p class="sub" data-now="baud"></p>
        <div class="trow">
          <select data-new-baud aria-label="${esc(t("new_baud"))}">${BAUDS.map((b) => `<option value="${b}">${b} Bd</option>`).join("")}</select>
          <button type="button" class="tonal" data-switch>${esc(t("switch"))}</button>
          <button type="button" class="quiet" data-detect>${esc(t("detect_baud"))}</button>
        </div>
      </fieldset>
      <fieldset class="tool" data-tool>
        <h3>${esc(t("reset_title"))}</h3>
        <p class="sub" data-now="reset"></p>
        <div class="trow"><button type="button" class="quiet danger-text" data-reset>${esc(t("reset"))}</button></div>
      </fieldset>
    </div>
    <div class="wsplit">
      <section class="result" data-result aria-live="polite"></section>
      <section class="logpart">
        <div class="lhead">
          <b>${esc(t("telegram_log"))}</b>
          <label class="follow"><input type="checkbox" class="switch" data-follow checked><span>${esc(t("follow"))}</span></label>
          <span class="grow"></span>
          <button type="button" class="quiet" data-clear-log>${esc(t("clear_log"))}</button>
        </div>
        <div class="lines mono" data-log></div>
        <div class="empty" data-log-empty>${esc(t("wz_log_empty"))}</div>
        <div class="rawrow">
          <input class="mono" data-hex placeholder="10 40 01 41 16" aria-label="${esc(t("raw_bytes"))}" title="${esc(t("raw_hint"))}">
          <button type="button" class="tonal" data-analyze>${esc(t("analyze"))}</button>
          <button type="button" class="quiet" data-raw-send>${esc(t("send"))}</button>
        </div>
        <div class="analysis" data-analysis hidden></div>
      </section>
    </div>`;
  wire();
  watch(["status"], onLine);
  watch(["status", "meters"], render);
  setInterval(() => open() && !document.hidden && pollLog().catch(() => {}), LOG_EVERY);
  pollLog().catch(() => {});
}

/// Linie gewechselt: Ziel dieser Linie holen, Zählerliste laden
function onLine() {
  const id = state.status?.active?.line?.id;
  if (!id || id === lineId) return;
  lineId = id;
  if (!perLine.has(id)) perLine.set(id, { target: null, reading: null });
  view = perLine.get(id);
  loadMeters().then(targetFromHash).catch(report);
}

/// Andere Bereiche (z.B. Scan) schicken ein Ziel über die Adresse: #/werkzeuge?ziel=5&baud=9600
function targetFromHash() {
  const params = new URLSearchParams(location.hash.split("?")[1] || "");
  if (!open() || !params.get("ziel")) return;
  setTarget(params.get("ziel"), Number(params.get("baud")) || null);
  history.replaceState(null, "", "#/werkzeuge");
}

// ------------------------------------------------------------ Ziel

const busBaud = () => state.status?.connection?.baud || state.status?.active?.line?.baud || 2400;

/// Zähler der Liste zu einer Adresse (PA, ID, Sekundäradresse oder seine gespeicherte Adresse)
function meterFor(address) {
  const a = String(address).toUpperCase();
  return (state.meters || []).find((m) => [m.address, m.meter_id, m.secondary_address, m.primary_address].some((x) => x != null && String(x).toUpperCase() === a));
}

function setTarget(address, baud = null) {
  const m = meterFor(address);
  view.target = { address: String(address).trim(), baud: baud || m?.baud || null };
  q("[data-target-input]").value = "";
  closePicks();
  render();
}

/// Baudrate für Befehle, wenn der Zähler nicht mit der Bus-Baudrate läuft
const fromBaud = () => (view.target?.baud && view.target.baud !== busBaud() ? view.target.baud : null);

function targetPa() {
  const m = meterFor(view.target.address);
  if (m?.primary_address != null) return m.primary_address;
  return /^\d{1,3}$/.test(view.target.address) ? Number(view.target.address) : null;
}

function renderTarget() {
  const box = q("[data-tstate]");
  q("[data-clear]").hidden = !view.target;
  if (!view.target) return html(box, `<span class="chip warn">${esc(t("wz_no_target"))}</span>`);
  const m = meterFor(view.target.address);
  const pa = targetPa();
  const ident = /^\d{1,3}$/.test(view.target.address) ? "" : view.target.address;
  const facts = [pa != null ? `PA ${pa}` : "", ident ? `ID ${ident}` : "", `${view.target.baud || busBaud()} Bd`].filter(Boolean).join(" · ");
  html(
    box,
    `${m ? `${mediumIcon(m.medium, true)}<b class="cut">${esc(m.name || t("meter_unnamed"))}</b>` : ""}<span class="mono">${esc(facts)}</span>` +
      (fromBaud() ? ` <span class="chip warn">${esc(t("wz_from_baud", fromBaud()))}</span>` : "")
  );
}

function renderPicks() {
  const s = q("[data-target-input]").value.trim();
  const low = s.toLowerCase();
  const meters = (state.meters || [])
    .filter((m) => !low || [m.name, m.location, m.meter_id, m.address, m.secondary_address, m.primary_address].some((x) => String(x ?? "").toLowerCase().includes(low)))
    .slice(0, PICKS_MAX);
  picks = [...(ADDRESS.test(s) && !meterFor(s) ? [{ address: s }] : []), ...meters.map((meter) => ({ meter }))];
  pickAt = Math.min(pickAt, Math.max(0, picks.length - 1));
  html(
    q("[data-picks]"),
    picks.length
      ? picks
          .map((p, i) => {
            const at = i === pickAt ? ' aria-selected="true"' : "";
            if (p.address) return `<div role="option" data-pick="${i}"${at}><b>${esc(t("wz_use_address", p.address))}</b></div>`;
            const b = meterBlock(p.meter);
            const ident = [b.ident, b.pa != null && b.pa !== "" ? `PA ${b.pa}` : ""].filter(Boolean).join(" · ");
            return `<div role="option" data-pick="${i}"${at}><span class="cut">${mediumIcon(p.meter.medium, true)}${esc(p.meter.name || t("meter_unnamed"))}</span><span class="sub mono">${esc(ident)}</span></div>`;
          })
          .join("")
      : `<div class="sub hint">${esc(t(state.meters?.length ? "wz_no_match" : "n_no_meters"))}</div>`
  );
}

function openPicks() {
  renderPicks();
  q("[data-picks]").hidden = false;
  q("[data-target-input]").setAttribute("aria-expanded", "true");
  releasePicks ??= onEscape(closePicks);
}

function closePicks() {
  q("[data-picks]").hidden = true;
  q("[data-target-input]").setAttribute("aria-expanded", "false");
  releasePicks?.();
  releasePicks = null;
}

function choosePick(i) {
  const p = picks[i];
  if (p) setTarget(p.address || p.meter.address, p.meter?.baud || null);
}

// ------------------------------------------------------------ Zeichnen

function render() {
  if (!view) return;
  renderTarget();
  const reason = !view.target ? t("wz_locked_target") : !state.status?.connected ? t("wz_locked_conn") : "";
  for (const f of root.querySelectorAll("[data-tool]")) f.disabled = !!reason;
  const now = (key, value) => {
    const el = q(`[data-now=${key}]`);
    text(el, reason || value);
    el.classList.toggle("warn", !!reason);
  };
  const pa = view.target ? targetPa() : null;
  now("read", t("wz_read_what"));
  now("pa", pa != null ? t("target_now", `PA ${pa}`) : t("new_primary"));
  now("baud", view.target ? t("target_now", `${view.target.baud || busBaud()} Bd`) : "");
  now("reset", t("reset_short"));
  renderResult();
}

function renderResult() {
  const r = view.reading;
  const box = q("[data-result]");
  if (!r) return html(box, `<p class="empty">${esc(t("wz_result_empty"))}</p>`);
  const h = r.header || {};
  const listed = meterFor(h.secondary_address || "") || meterFor(h.id || "") || meterFor(r.target);
  const meta = [
    ["meta_id", h.id],
    ["meta_manufacturer", h.manufacturer],
    ["meta_medium", h.medium && label(h.medium)],
    ["meta_version", h.version],
    ["meta_primary", r.primary_address],
    ["meta_secondary", h.secondary_address],
    ["meta_access", h.access_no],
    ["meta_status", h.status != null ? "0x" + h.status.toString(16).padStart(2, "0").toUpperCase() : null],
  ].filter(([, v]) => v != null && v !== "");
  html(
    box,
    `<header class="rhead"><b>${esc(listed?.name || r.target)}</b><span class="sub">${esc(r.timestamp)}${r.time_unsure ? ` · ${esc(t("reading_time_unsure"))}` : ""}</span>
      <span class="grow"></span>${
        listed
          ? `<span class="chip ok">✓ ${esc(t("wz_listed"))}</span>`
          : `<span class="chip">${esc(t("wz_not_listed"))}</span><button type="button" class="tonal" data-remember>${esc(t("remember"))}</button>`
      }</header>
    <dl class="meta">${meta.map(([k, v]) => `<div><dt>${esc(t(k))}</dt><dd class="mono">${esc(v)}</dd></div>`).join("")}</dl>
    <table class="rtable"><thead><tr><th>#</th><th>${esc(t("th_desc"))}</th><th class="num">${esc(t("th_value"))}</th><th>${esc(t("th_unit"))}</th><th>${esc(t("th_kind"))}</th><th>DIF | VIF</th></tr></thead>
    <tbody>${r.records.map(recordRow).join("")}</tbody></table>
    <h3>${esc(t("wz_raw_frames"))}</h3>
    ${(r.raw_frames || []).map((f) => `<button type="button" class="frame mono" data-hex="${esc(f)}" title="${esc(t("log_click"))}">${esc(f)}</button>`).join("")}`
  );
}

function recordRow(x, i) {
  const extra = [x.storage ? `${t("storage")} ${x.storage}` : "", x.tariff ? `${t("tariff")} ${x.tariff}` : "", x.subunit ? `${t("subunit")} ${x.subunit}` : ""].filter(Boolean);
  const notes = x.notes?.length ? ` (${x.notes.map(label).join(", ")})` : "";
  return `<tr><td class="sub mono">${i + 1}</td><td>${esc(label(x.quantity) + notes)}</td><td class="num mono"><b>${esc(x.value_text)}</b></td><td>${esc(x.unit)}</td>
    <td class="sub">${esc([label(x.function), ...extra].join(" · "))}</td><td class="sub mono">${esc(x.dif)} | ${esc(x.vif)}</td></tr>`;
}

// ------------------------------------------------------------ Werkzeuge

/// Knopf während der Anfrage sperren; Fehler als Meldung
async function run(button, fn) {
  button.disabled = true;
  try {
    await fn();
  } catch (e) {
    report(e);
  } finally {
    button.disabled = false;
  }
}

async function readTarget(button) {
  if (!view.target || !state.status?.connected) return;
  await run(button, async () => {
    text(button, t("reading_busy"));
    try {
      view.reading = await request("read", { method: "POST", body: { address: view.target.address, baud: fromBaud() } });
    } finally {
      text(button, t("read"));
    }
    renderResult();
    loadMeters().catch(() => {});
  });
}

async function findFree(button) {
  await run(button, async () => {
    const skip = [...new Set((state.meters || []).map((m) => m.primary_address).filter((a) => a > 0))];
    const r = await request("free-address", { method: "POST", body: { from: 1, skip } });
    q("[data-new-pa]").value = r.address;
    toast(t("wz_free_found", r.address));
  });
}

async function setPa(button) {
  const value = q("[data-new-pa]").value;
  if (value === "") return q("[data-new-pa]").focus();
  const address = view.target.address;
  if (!(await ask(button, t("confirm_set", address, value), t("set")))) return;
  await run(button, async () => {
    await request("set-address", { method: "POST", body: { address, new_address: Number(value), from_baud: fromBaud() } });
    // steht der Zähler mit seiner Primäradresse in der Liste, die neue dort eintragen
    const m = meterFor(address);
    if (m && /^\d+$/.test(m.address)) await request(`meters/${m.id}`, { method: "PUT", body: { ...m, address: value } });
    if (/^\d{1,3}$/.test(address)) view.target.address = value;
    await loadMeters();
    toast(t("address_set", value));
  });
}

async function switchBaud(button) {
  const baud = Number(q("[data-new-baud]").value);
  const address = view.target.address;
  if (!(await ask(button, t("confirm_switch", address, baud), t("switch")))) return;
  await run(button, async () => {
    await request("switch-baud", { method: "POST", body: { address, baud, from_baud: fromBaud() } });
    view.target.baud = baud;
    const m = meterFor(address);
    // eigene Baudrate nur speichern, wenn sie vom Bus abweicht
    if (m) await request(`meters/${m.id}`, { method: "PUT", body: { ...m, baud: baud === busBaud() ? null : baud } });
    await loadMeters();
    toast(t(m ? "baud_switched_list" : "baud_switched", baud));
  });
}

async function detectBaud(button) {
  await run(button, async () => {
    text(button, t("detect_busy"));
    try {
      const r = await request("detect-baud", { method: "POST", body: { address: view.target.address } });
      if (!r.bauds.length) return toast(t("detect_none"), { error: true });
      // läuft er woanders, gehen die Befehle mit seiner Baudrate; Umstellen auf den Bus liegt nahe
      view.target.baud = r.bauds.includes(busBaud()) ? null : r.bauds[0];
      q("[data-new-baud]").value = String(busBaud());
      toast(t("detect_found", r.bauds.join(", ")));
      render();
    } finally {
      text(button, t("detect_baud"));
    }
  });
}

async function reset(button) {
  if (!(await ask(button, t("confirm_reset", view.target.address), t("reset")))) return;
  await run(button, async () => {
    await request("reset", { method: "POST", body: { address: view.target.address, from_baud: fromBaud() } });
    toast(t("reset_done"));
  });
}

async function remember(button) {
  const h = view.reading.header || {};
  // gemerkt wie die Linie: über Sekundär- oder Primäradresse
  const primary = activeLine()?.addressing === "primary";
  const address = primary ? String(view.reading.primary_address) : h.secondary_address || view.reading.target;
  await run(button, async () => {
    await request("meters", { method: "POST", body: { address, baud: fromBaud() } });
    await loadMeters();
    toast(t("remembered"));
  });
}

// ------------------------------------------------------------ Log, Rohbytes, Analyse

async function pollLog() {
  const entries = await request("log?since=" + logSeq);
  if (entries.length) {
    logSeq = entries[entries.length - 1].seq;
    log = [...log, ...entries].slice(-LOG_KEEP);
  }
  renderLog();
}

function renderLog() {
  const box = q("[data-log]");
  list(box, log, (e) => e.seq, logRow);
  q("[data-log-empty]").hidden = log.length > 0;
  if (q("[data-follow]").checked) box.scrollTop = box.scrollHeight;
}

function logRow(e) {
  const frame = e.dir === "TX" || e.dir === "RX";
  return `<div class="l d-${esc(e.dir)}"${frame ? ` data-hex="${esc(e.text)}" title="${esc(t("log_click"))}"` : ""}><span class="sub">${esc(e.time)}</span><b>${esc(e.dir)}</b><span>${esc(e.text)}</span></div>`;
}

async function clearLog() {
  try {
    await request("log", { method: "DELETE" });
    log = [];
    renderLog();
  } catch (e) {
    report(e);
  }
}

/// Telegramm erklären lassen (sendet nichts)
async function analyze(hex) {
  if (!hex) return q("[data-hex]").focus();
  q("[data-hex]").value = hex;
  try {
    const a = await request("analyze", { method: "POST", body: { hex } });
    showAnalysis(
      `<b>${esc(a.summary)}</b>`,
      (a.problems.length ? a.problems.map((p) => `<p class="chip err wide">${esc(p)}</p>`).join("") : `<p class="chip ok wide">${esc(t("an_clean"))}</p>`) +
        `<table class="atable"><thead><tr><th>${esc(t("an_bytes"))}</th><th>${esc(t("an_field"))}</th><th>${esc(t("an_meaning"))}</th></tr></thead><tbody>${a.fields
          .map((f) => `<tr${f.bad ? ' class="bad"' : ""}><td class="mono">${esc(f.bytes || "–")}</td><td>${esc(f.name)}</td><td>${esc(f.value)}</td></tr>`)
          .join("")}</tbody></table>`
    );
  } catch (e) {
    report(e);
  }
}

async function sendRaw(button) {
  const hex = q("[data-hex]").value.trim();
  if (!hex) return q("[data-hex]").focus();
  await run(button, async () => {
    const r = await request("raw", { method: "POST", body: { hex } });
    const lines = [];
    if (r.decoded?.header) lines.push(`ID ${r.decoded.header.id} · ${r.decoded.header.manufacturer} · ${label(r.decoded.header.medium)}`);
    for (const x of r.decoded?.records || []) lines.push(`${label(x.quantity)}: ${x.value_text} ${x.unit}`.trim());
    showAnalysis(
      `<b>${esc(t("raw_" + r.kind))}</b>`,
      (r.hex && r.kind !== "ack" ? `<button type="button" class="frame mono" data-hex="${esc(r.hex)}" title="${esc(t("log_click"))}">${esc(r.hex)}</button>` : "") +
        lines.map((l) => `<div>${esc(l)}</div>`).join("")
    );
  });
}

function showAnalysis(head, body) {
  const box = q("[data-analysis]");
  box.hidden = false;
  html(box, `<div class="ahead">${head}<span class="grow"></span><button type="button" class="icon" data-close-analysis title="${esc(t("wz_close"))}" aria-label="${esc(t("wz_close"))}">×</button></div>${body}`);
}

// ------------------------------------------------------------ Bedienung

function wire() {
  const input = q("[data-target-input]");
  input.addEventListener("focus", openPicks);
  input.addEventListener("input", () => {
    pickAt = 0;
    openPicks();
  });
  input.addEventListener("blur", closePicks);
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      if (q("[data-picks]").hidden) return openPicks();
      pickAt = Math.max(0, Math.min(picks.length - 1, pickAt + (ev.key === "ArrowDown" ? 1 : -1)));
      renderPicks();
      q(`[data-pick="${pickAt}"]`)?.scrollIntoView({ block: "nearest" });
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      if (picks.length) choosePick(pickAt);
    }
  });
  // Klick in die Auswahl soll das Feld nicht erst verlassen (sonst schliesst sie vorher)
  q("[data-picks]").addEventListener("mousedown", (ev) => ev.preventDefault());

  root.addEventListener("click", (ev) => {
    const el = ev.target;
    const on = (sel) => el.closest(sel);
    if (on("[data-pick]")) return choosePick(Number(on("[data-pick]").dataset.pick));
    if (on("[data-clear]")) {
      view.target = null;
      render();
      return input.focus();
    }
    if (on("[data-read]")) return readTarget(on("[data-read]"));
    if (on("[data-free]")) return findFree(on("[data-free]"));
    if (on("[data-set-pa]")) return setPa(on("[data-set-pa]"));
    if (on("[data-switch]")) return switchBaud(on("[data-switch]"));
    if (on("[data-detect]")) return detectBaud(on("[data-detect]"));
    if (on("[data-reset]")) return reset(on("[data-reset]"));
    if (on("[data-remember]")) return remember(on("[data-remember]"));
    if (on("[data-clear-log]")) return clearLog();
    if (on("[data-analyze]")) return analyze(q("[data-hex]").value.trim());
    if (on("[data-raw-send]")) return sendRaw(on("[data-raw-send]"));
    if (on("[data-close-analysis]")) return (q("[data-analysis]").hidden = true);
    if (on("[data-hex]") && !on("input")) return analyze(on("[data-hex]").dataset.hex);
  });
  q("[data-new-pa]").addEventListener("keydown", (ev) => ev.key === "Enter" && setPa(q("[data-set-pa]")));
  q("[data-hex]").addEventListener("keydown", (ev) => ev.key === "Enter" && analyze(ev.target.value.trim()));
  addEventListener("hashchange", () => view && targetFromHash());
  bind("r", () => readTarget(q("[data-read]")), t("wz_key_read"), open);
}
