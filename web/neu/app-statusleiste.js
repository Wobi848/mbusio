// Statusleiste: was der Bus macht (Klick: Bus-Konsole), Auffälligkeiten (Klick: im Scan zeigen),
// Ausgänge und der nächste Schritt. Event-Handler hängen beim Laden des Moduls an.

import { state, set, watch, activeLine, faulty } from "./store.js";
import { html, outputWord } from "./view.js";
import { onEscape } from "./keys.js";
import { follow, poll, logRow } from "./buslog.js";

const CONSOLE_KEEP = 300; // Zeilen in der Bus-Konsole; alles in Werkzeuge

/// Anzeigen an den Zustand hängen und das Protokoll abfragen, solange es jemand zeigt
export function startStatusleiste() {
  watch(["tree", "status"], renderStatus);
  watch(["tree", "status", "meters", "scanNew"], renderNext);
  watch(["log"], () => renderConsole());
  follow(() => consoleOpen || state.area === "werkzeuge");
}

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
  if (!l.meters) {
    // gefunden heisst noch nicht gemerkt: erst „neue merken“ bringt sie in die Zählerliste
    const fresh = state.scanNew?.line === l.id ? state.scanNew.n : 0;
    return fresh ? ["guide_merge", go("scan", "[data-merge]"), fresh] : ["guide_scan", go("scan", "[data-start]")];
  }
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
export async function goTo(area, ...sels) {
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
