// Neue Oberfläche: Start, Bereiche, Kopfzeile, Statusleiste (Konzept neue Oberfläche).
// Gemeinsam mit dem Bericht: js/texts.js (t, UI) und js/base.js (request, esc, …).

import { state, set, watch, activeLine, operatingLines, faulty } from "./store.js";
import { html, text, menu, closeMenu, outputWord, opPossible, opText } from "./view.js";
import { bind, keyList, onEscape } from "./keys.js";
import { loadStatus, loadTree, reload, useLine, report, switchOperation } from "./data.js";
import { follow, poll, logRow } from "./buslog.js";

const $ = (id) => document.getElementById(id);
const AREAS = ["anlage", "scan", "zaehler", "weitergabe", "werkzeuge", "simulator"];
const AREA_TAB = { scan: "tab_scan", zaehler: "tab_meters", weitergabe: "tab_transfer", anlage: "tab_project", werkzeuge: "tab_tools", simulator: "tab_sim" };
const STATUS_EVERY = 5000;
const TREE_EVERY = 15000;
const CONSOLE_KEEP = 300; // Zeilen in der Bus-Konsole; alles in Werkzeuge

// ------------------------------------------------------------ Texte

/// Feste Texte im Gerüst (data-t, data-title) in der gewählten Sprache
function applyTexts() {
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll("[data-t]")) el.textContent = t(el.dataset.t);
  for (const el of document.querySelectorAll("[data-title]")) {
    el.title = t(el.dataset.title);
    el.setAttribute("aria-label", el.title);
  }
}

// ------------------------------------------------------------ Kopfzeile

function renderProjects() {
  const sel = $("project");
  const active = state.status?.active?.project?.id;
  html(
    sel,
    (state.tree?.projects || []).map((p) => `<option value="${p.id}"${p.id === active ? " selected" : ""}>${esc(p.name)}</option>`).join("")
  );
  if (active) sel.value = String(active);
}

$("project").addEventListener("change", (ev) => {
  const p = state.tree.projects.find((x) => x.id === Number(ev.target.value));
  const line = p?.gateways.flatMap((g) => g.lines)[0];
  if (line) useLine(line.id).catch(report);
});

function renderLine() {
  const a = state.status?.active;
  if (!a) return;
  html($("line"), `<b>${esc(a.line.name)}</b><span class="sub">${esc(a.gateway.name)}</span>`);
  const project = state.tree?.projects.find((p) => p.id === a.project.id);
  html(
    $("line-menu"),
    (project?.gateways || [])
      .map(
        (g) =>
          `<div class="group">${esc(g.name)}</div>` +
          g.lines
            .map(
              (l) =>
                `<button type="button" role="menuitem" data-line="${l.id}"${l.id === a.line.id ? ' aria-current="true"' : ""}>` +
                `<span>${esc(l.name)}${l.operation ? ` <span class="dot on" title="${esc(t("op_chip"))}"></span>` : ""}</span>` +
                `<span class="sub mono">${esc(portText(l.connection) || t("ln_no_conn"))}</span></button>`
            )
            .join("")
      )
      .join("")
  );
}

menu($("line"), $("line-menu"));
$("line-menu").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-line]");
  if (!b) return;
  closeMenu();
  const id = Number(b.dataset.line);
  if (id !== state.status.active.line.id) useLine(id).catch(report);
});

/// Anschluss: Zustand als Wort und Farbe, daneben genau die passende Aktion
function renderConnection() {
  const s = state.status;
  if (!s) return;
  const line = s.active.line;
  const btn = $("conn-action");
  if (s.connected) {
    $("conn-state").title = "";
    html($("conn-state"), `<span class="dot on"></span><b>${esc(t("lb_connected"))}</b> <span class="mono">${esc(portText(s.connection.port))} · ${s.connection.baud}</span>`);
    text(btn, t("n_change"));
    btn.title = t("conn_change");
    btn.className = "quiet";
  } else if (line.connection) {
    // der Grund (z.B. „USB-Wandler abgezogen?“) steht ganz in der Statusleiste, hier nur der Zustand
    html($("conn-state"), `<span class="dot off"></span><b>${esc(t("lb_disconnected"))}</b> <span class="mono">${esc(portText(line.connection))}</span>`);
    $("conn-state").title = s.line?.error || "";
    text(btn, t("connect"));
    btn.title = "";
    btn.className = "";
  } else {
    html($("conn-state"), `<span class="dot warn"></span><b class="warn">${esc(t("ln_no_conn"))}</b>`);
    text(btn, t("ln_set_conn"));
    btn.title = "";
    btn.className = "";
  }
}

/// Verbinden mit dem gespeicherten Anschluss geht direkt; sonst das kleine Formular
$("conn-action").addEventListener("click", async (ev) => {
  const s = state.status;
  if (!s.connected && s.active.line.connection) {
    ev.stopImmediatePropagation(); // nicht auch noch das Formular öffnen (menu() hängt danach)
    try {
      await request("connect", { method: "POST", body: { port: s.active.line.connection, baud: s.active.line.baud } });
    } catch (e) {
      report(e);
    }
    return reload();
  }
});
menu($("conn-action"), $("conn-form"), { onOpen: renderConnForm });

async function renderConnForm() {
  const s = state.status;
  const current = s.connection?.port || s.active.line.connection || "";
  const baud = s.connection?.baud || s.active.line.baud || 2400;
  const box = $("conn-form");
  // die Suche nach COM-Ports kann unter Windows Sekunden dauern: zeigen, dass gesucht wird
  if (!state.ports) box.innerHTML = `<p class="sub"><span class="dot busy"></span> ${esc(t("n_ports_searching"))}</p>`;
  const ports = state.ports || (await request("ports").catch(() => []));
  set({ ports });
  const known = ports.some((p) => p.port === current);
  box.innerHTML = `
    <label class="row"><span>${esc(t("port"))}</span><select name="port">${ports
      .map((p) => `<option value="${esc(p.port)}"${p.port === current ? " selected" : ""}>${esc(p.label ? `${p.dev || portText(p.port)} · ${p.label}` : p.port)}</option>`)
      .join("")}</select></label>
    <label class="row"><span>${esc(t("port_manual"))}</span><input name="manual" class="mono" value="${known ? "" : esc(current)}" placeholder="COM3, /dev/ttyUSB0, tcp://…"></label>
    <label class="row"><span>${esc(t("baud"))}</span><select name="baud">${BAUDS
      .map((b) => `<option${b === baud ? " selected" : ""}>${b}</option>`)
      .join("")}</select></label>
    <div class="row end">${s.connected ? `<button type="button" class="quiet" data-disconnect>${esc(t("disconnect"))}</button>` : ""}<button type="submit" data-connect>${esc(t("connect"))}</button></div>`;
  box.querySelector("[name=manual]").addEventListener("keydown", (ev) => ev.key === "Enter" && box.querySelector("[data-connect]").click());
}

$("conn-form").addEventListener("click", async (ev) => {
  const box = $("conn-form");
  const go = ev.target.closest("[data-connect], [data-disconnect]");
  if (!go) return;
  try {
    if (go.dataset.disconnect !== undefined) await request("disconnect", { method: "POST" });
    else {
      const port = box.querySelector("[name=manual]").value.trim() || box.querySelector("[name=port]").value;
      await request("connect", { method: "POST", body: { port, baud: Number(box.querySelector("[name=baud]").value) } });
    }
    closeMenu(true);
  } catch (e) {
    report(e);
  }
  reload();
});

/// Dauerbetrieb der Linie: Schalter zeigt Zustand und schaltet; ohne Plan gesperrt mit Grund daneben
function renderOperation() {
  const l = activeLine();
  if (!l) return;
  const p = l.plan_info;
  const sw = $("op-switch");
  sw.checked = !!l.operation;
  sw.disabled = !opPossible(l);
  const fg = l.operation && state.status && !state.status.background;
  $("op").classList.toggle("on", !!l.operation);
  // nächster Termin und „nicht als Dienst“ als Detail; sichtbar steht das an „N im Dauerbetrieb“
  $("op").title = [l.operation && p.next ? t("plan_next", slotText(p.next)) : "", fg ? t("ps_fg") : ""].filter(Boolean).join(" · ");
  text($("op-text"), opText(l));
}

$("op-switch").addEventListener("change", (ev) => switchOperation(ev.target, activeLine()));

/// Kopfzeile rechts: was dieses Gerät im Hintergrund tut, über alle Projekte
function renderOpAll() {
  const ops = operatingLines();
  const b = $("op-all");
  b.hidden = !ops.length;
  if (!ops.length) return;
  const warn = !state.status?.background || ops.some(({ l }) => l.plan_info?.result?.line_down || l.plan_info?.result?.offline);
  html(b, `<span class="dot ${warn ? "warn" : "on"}"></span>${esc(t("n_op_count", ops.length))}`);
  html(
    $("op-all-menu"),
    (state.status?.background ? "" : `<div class="group warn">${esc(t("ps_fg"))}</div>`) +
      ops
        .map(({ p, g, l }) => {
          const r = l.plan_info?.result;
          const bad = r?.line_down || r?.offline;
          return `<button type="button" role="menuitem" data-line="${l.id}">` +
            `<span>${esc(`${p.name} › ${g.name} › ${l.name}`)}</span>` +
            `<span class="sub">${bad ? `<span class="chip warn">${esc(t(r.offline ? "plan_offline" : "plan_line_down"))}</span>` : esc(l.plan_info?.next ? t("plan_next", slotText(l.plan_info.next)) : "")}</span></button>`;
        })
        .join("")
  );
}

menu($("op-all"), $("op-all-menu"));
$("op-all-menu").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-line]");
  if (!b) return;
  closeMenu();
  useLine(Number(b.dataset.line)).catch(report);
});

// ------------------------------------------------------------ Einstellungen

menu($("settings"), $("settings-menu"), {
  onOpen: () => {
    $("lang").value = lang;
    $("theme").value = themeChoice;
  },
});
$("lang").addEventListener("change", (ev) => {
  lang = ev.target.value;
  storageSet("lang", lang);
  location.reload(); // Sprache betrifft auch Texte vom Server; einmaliger Wechsel, kein Arbeitsablauf
});
$("theme").addEventListener("change", (ev) => {
  themeChoice = ev.target.value;
  storageSet("theme", themeChoice);
  applyTheme();
});
$("keys-help").addEventListener("click", () => {
  closeMenu();
  showKeys();
});

/// Übersicht der Tasten, die gerade wirken (?); Escape oder Klick daneben schliesst
function showKeys() {
  document.querySelector(".keys-help")?.remove();
  const box = document.createElement("div");
  box.className = "keys-help";
  box.setAttribute("role", "dialog");
  box.setAttribute("aria-label", t("n_keys"));
  const fixed = [
    ["↑ ↓", t("n_key_rows")],
    [t("n_key_space"), t("n_key_select")],
    ["Ctrl+A", t("n_key_all")],
    ["Enter", t("n_key_enter")],
    ["Esc", t("n_key_esc")],
  ];
  box.innerHTML = `<b>${esc(t("n_keys"))}</b><dl>${[...keyList().map((k) => [k.key.toUpperCase(), k.label]), ...fixed]
    .map(([k, l]) => `<dt><kbd>${esc(k)}</kbd></dt><dd>${esc(l)}</dd>`)
    .join("")}</dl>`;
  document.body.append(box);
  const close = () => {
    release();
    box.remove();
    document.removeEventListener("click", outside, true);
  };
  const outside = (ev) => !box.contains(ev.target) && close();
  const release = onEscape(close);
  setTimeout(() => document.addEventListener("click", outside, true), 0);
}

// ------------------------------------------------------------ Bereiche

/// Adresse ↔ Bereich: #/zaehler, #/scan …; Zurück-Taste und Lesezeichen funktionieren
function readHash() {
  const area = location.hash.replace(/^#\/?/, "").split(/[/?]/)[0];
  set({ area: AREAS.includes(area) ? area : "zaehler" });
}
addEventListener("hashchange", readHash);

function renderTabs() {
  for (const a of document.querySelectorAll(".tabs [data-area]")) {
    a.title = `${t(AREA_TAB[a.dataset.area])} (${AREAS.indexOf(a.dataset.area) + 1})`;
    if (a.dataset.area === state.area) a.setAttribute("aria-current", "page");
    else a.removeAttribute("aria-current");
  }
  const l = activeLine();
  text($("n-meters"), l?.meters ? String(l.meters) : "");
  text($("n-dp"), l?.datapoints?.count ? String(l.datapoints.count) : "");
  $("tab-sim").hidden = !(l?.connection || "").startsWith("SIMULATOR") && state.area !== "simulator";
}

/// Arbeitsfläche: jeder Bereich hat eine eigene Fläche, die beim Wechsel bleibt (Scroll, Auswahl,
/// Filter). Ein Bereich ist die Datei neu/<bereich>.js mit `mount(fläche)`, geladen beim ersten
/// Öffnen.
const panes = {};
function renderWork() {
  for (const a of AREAS) {
    if (!panes[a] && a === state.area) {
      panes[a] = document.createElement("section");
      panes[a].className = "pane";
      panes[a].dataset.area = a;
      $("work").append(panes[a]);
      import(`./${a}.js`).then((m) => m.mount(panes[a]));
    }
    if (panes[a]) panes[a].hidden = a !== state.area;
  }
}

AREAS.forEach((a, i) =>
  bind(String(i + 1), () => (location.hash = `#/${a}`), t("n_key_area", t(AREA_TAB[a])))
);
bind("/", () => document.querySelector("#work .pane:not([hidden]) [data-search]")?.focus(), t("n_key_search"));
bind("?", showKeys, t("n_key_help"));

// ------------------------------------------------------------ Statusleiste

/// Was der Bus gerade macht (Statusleiste und Kopf der Bus-Konsole)
function busState(s) {
  const run = s.plans?.[0];
  return s.job_running
    ? `<span class="dot busy"></span>${esc(t("lb_busy"))}`
    : run
      ? `<span class="dot busy"></span>${esc(t("plan_running", run.label, run.done, run.total))}`
      : s.connected
        ? `<span class="dot on"></span>${esc(t("idle_ready", portText(s.connection.port), s.connection.baud))}`
        : `<span class="dot off"></span>${esc(t("idle_off"))}${s.line?.error ? ` <span class="warn cut" title="${esc(s.line.error)}">· ${esc(s.line.error)}</span>` : ""}`;
}

function renderStatus() {
  const s = state.status;
  if (!s) return;
  html($("bus"), busState(s));
  html($("console-state"), busState(s));
  const issues = activeLine()?.issues;
  html(
    $("issues"),
    issues
      ? issues.count
        ? `<button type="button" class="chip warn" data-show-issues title="${esc(t("n_issues_title"))}">${esc(t("issues", issues.count))}</button>`
        : `<span class="chip ok">${esc(t("issues_none"))}</span>`
      : ""
  );
  const outs = (s.outputs || []).filter((o) => o.on);
  html(
    $("outputs"),
    outs
      .map((o) => {
        const [cls, word] = outputWord(s[o.key]?.state);
        return `<span class="chip ${cls}" title="${esc(s[o.key]?.error || "")}">${esc(o.label)} · ${esc(word)}</span>`;
      })
      .join(" ")
  );
}

// ------------------------------------------------------------ Nächster Schritt

/// Der eine nächste Schritt aus dem Zustand (Konzept §3.4): [Text-Schlüssel, Aktion, Argument für
/// den Erklärtext im Tooltip] oder null.
/// Der Knopf führt an die Stelle, an der man es auch selbst täte.
function nextStep() {
  const s = state.status;
  const l = activeLine();
  if (!s || !l) return null;
  const go = (area, sel) => () => goTo(area, sel);
  if (!l.connection) return ["guide_conn_set", () => $("conn-action").click()];
  if (!s.connected) return ["guide_connect", () => $("conn-action").click(), portText(l.connection)];
  if (!l.meters) return ["guide_scan", go("scan", "[data-start]")];
  if (!l.last_read) return ["guide_read", go("zaehler", "[data-read-all]"), l.meters];
  const bad = (state.meters || []).filter(faulty).length;
  if (bad) return ["guide_faulty_btn", go("zaehler", "[data-filter=err]"), bad];
  if (!l.operation) return ["guide_ready_btn", () => window.open("report.html", "_blank", "noopener")];
  if (!l.datapoints?.count) return ["guide_transfer", () => (location.hash = "#/zaehler")];
  if (!(s.outputs || []).some((o) => o.on)) return ["guide_output", () => (location.hash = "#/weitergabe")];
  return null;
}

/// In einen Bereich wechseln und dort nacheinander klicken, sobald das jeweilige Element da ist
/// (ein Bereich lädt beim ersten Öffnen erst seine Daten)
async function goTo(area, ...sels) {
  location.hash = `#/${area}`;
  for (const sel of sels) {
    let el = null;
    for (let i = 0; i < 40 && !el; i++) {
      el = document.querySelector(`#work .pane[data-area=${area}] ${sel}`);
      if (!el) await new Promise((r) => setTimeout(r, 100));
    }
    el?.click();
  }
}

let nextAction = null;
function renderNext() {
  const step = nextStep();
  const b = $("next");
  b.hidden = !step;
  if (!step) return;
  const [key, run, arg] = step;
  html(b, `<span class="next-label">${esc(t("n_next"))}: </span>${esc(t(key))} →`);
  b.title = t(key.replace(/_btn$/, "") + "_body", arg);
  nextAction = run;
}
$("next").addEventListener("click", () => nextAction?.());

// Auffälligkeiten: im Scan zeigen, die erste gewählt (rechts Ursache und Abhilfe)
$("issues").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-show-issues]")) goTo("scan", "[data-filter=attention]", "[data-list] [role=option]");
});

// ------------------------------------------------------------ Bus-Konsole

let consoleOpen = false;
let releaseConsole = null;

function toggleConsole(open = !consoleOpen) {
  consoleOpen = open;
  $("console").hidden = !open;
  $("bus").setAttribute("aria-expanded", String(open));
  releaseConsole?.();
  releaseConsole = open ? onEscape(() => toggleConsole(false)) : null;
  if (open) {
    poll().catch(() => {});
    renderConsole(true);
  }
}

/// Neueste Zeilen; unten bleiben, solange man unten ist (zum Lesen hochscrollen hält an)
function renderConsole(toEnd = false) {
  if (!consoleOpen) return;
  const box = $("console-log");
  const atEnd = toEnd || box.scrollHeight - box.scrollTop - box.clientHeight < 30;
  const rows = (state.log || []).slice(-CONSOLE_KEEP);
  html(box, rows.length ? rows.map(logRow).join("") : `<div class="empty">${esc(t("n_console_empty"))}</div>`);
  if (atEnd) box.scrollTop = box.scrollHeight;
}

$("bus").addEventListener("click", () => toggleConsole());
$("console-close").addEventListener("click", () => toggleConsole(false));
$("console").addEventListener("click", (ev) => {
  // ganzes Protokoll oder ein Telegramm: in Werkzeuge weiter, dort ist das Protokoll ohnehin offen
  if (ev.target.closest("a[href='#/werkzeuge']")) return toggleConsole(false);
  const frame = ev.target.closest("[data-hex]");
  if (!frame) return;
  set({ analyze: frame.dataset.hex });
  location.hash = "#/werkzeuge";
  toggleConsole(false);
});

// ------------------------------------------------------------ Start

async function start() {
  applyTexts();
  labels = await request("labels").catch(() => ({}));
  watch(["tree", "status"], renderProjects);
  watch(["tree", "status"], renderLine);
  watch(["status"], renderConnection);
  watch(["tree", "status"], renderOperation);
  watch(["tree", "status"], renderOpAll);
  watch(["tree", "status", "area"], renderTabs);
  watch(["area"], renderWork);
  watch(["tree", "status"], renderStatus);
  watch(["tree", "status", "meters"], renderNext);
  const fromAddress = location.hash.length > 2;
  readHash();
  await reload().catch(report);
  // ohne Adresse dort anfangen, wo es weitergeht: noch keine Zähler → Scan, sonst Zähler (§3.4)
  if (!fromAddress) location.hash = activeLine()?.meters ? "#/zaehler" : "#/scan";
  setInterval(() => loadStatus().catch(() => {}), STATUS_EVERY);
  watch(["log"], () => renderConsole());
  follow(() => consoleOpen || state.area === "werkzeuge");
  setInterval(() => loadTree().catch(() => {}), TREE_EVERY);
}

start();
