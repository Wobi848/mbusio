"use strict";

// Nächster Schritt: zeigt neuen wie erfahrenen Benutzern, was als Nächstes zu tun ist, mit genau
// einem Knopf. Die Reiter des Ablaufs zeigen dazu den Fortschritt (✓ erledigt, Punkt = als Nächstes).
// Service endet nach dem Auslesen beim Bericht; im Dauerbetrieb geht es weiter mit Plan, Werten und
// Ausgang, und die Weitergabe wird Reiter 4 (Konzept Bedienung, Abschnitt 9).

const GUIDE_TABS = ["project", "scan", "meters", "transfer"];
/// Reiter des Ablaufs: Weitergabe nur im Dauerbetrieb
const guideTabs = (op) => (op ? GUIDE_TABS : GUIDE_TABS.slice(0, 3));
/// „Dauerbetrieb einrichten“ gedrückt (Projekt-ID), gilt bis etwas eingerichtet ist
const wantsOperation = () => storageGet("guide_op") === String(tree.active.project.id);
/// Knopf, den die Karte gerade hervorhebt
let guideMarked = null;

/// Zustand → nächster Schritt; null = alles eingerichtet
function guideStep() {
  const line = tree && findLine(tree.active.line.id);
  if (!line) return null;
  const hasConn = !!(line.connection || connected);
  const okHits = lastJob && !lastJob.running ? lastJob.hits.filter((h) => h.status === "ok").length : 0;
  const read = meters.some((m) => m.last_read);
  const dps = line.datapoints?.count || 0;
  const op = inOperation() || wantsOperation();
  const done = { project: hasConn, scan: meters.length > 0, meters: read, transfer: dps > 0 && outputOn() };
  const fix = connected && fixState();
  const scanBtn = lastJob?.kind === "secondary" ? "scan-secondary" : "scan-primary";
  let step = null;
  // Anschluss und Verbinden zeigt die Linien-Leiste mit ihrem Knopf (bar: nicht in der Leiste unten)
  if (!hasConn) step = { key: "conn_set", tab: "project", target: "lb-action", bar: true, go: () => openConnForm() };
  else if (!connected) step = { key: "connect", tab: "scan", target: "lb-action", bar: true, arg: line.connection, go: () => $("connect").click() };
  else if (fix === "fix")
    step = {
      key: "fix",
      tab: "scan",
      mark: () => document.querySelector("[data-fix-all]"),
      go: openFixPlan,
      alt: meters.length ? null : { key: "anyway", go: rememberAll },
    };
  else if (fix === "check") step = { key: "check", tab: "scan", target: scanBtn, go: () => goClick("scan", scanBtn) };
  else if (!meters.length && okHits)
    step = { key: "remember", tab: "scan", target: "remember-all", arg: okHits, go: rememberAll };
  else if (!meters.length)
    step = { key: lastJob ? "scan_none" : "scan", tab: "scan", target: "scan-primary", go: () => goClick("scan", "scan-primary") };
  else if (!read) step = { key: "read", tab: "meters", target: "meters-read-all", arg: meters.length, go: () => goClick("meters", "meters-read-all") };
  else if (!op) step = { key: "ready", tab: "meters", go: () => $("project-report").click(), alt: { key: "op", go: startOperation } };
  else if (!line.plan_info?.meters) step = { key: "plan", tab: "project", target: "project-edit", go: openPlanForm };
  else if (!dps) {
    const pick = storageGet("tab") === "transfer";
    step = { key: pick ? "pick" : "transfer", tab: "transfer", mark: pick ? () => document.querySelector("[data-tr-pick]") : null, go: () => pickFirst() };
  }
  else if (!outputOn()) step = { key: "output", tab: "transfer", go: openOutput };
  return { step, done, op };
}

/// Reiter öffnen, Knopf zeigen und seine Aktion ausführen (wenn er gerade geht); true = ausgeführt
async function goClick(tabName, id) {
  showTab(tabName);
  const b = $(id);
  reveal(b, "center");
  if (b.disabled || b.hidden) return false;
  await b.onclick();
  return true;
}

/// Alle merken, danach geht es in der Zählerliste weiter
async function rememberAll() {
  if (!(await goClick("scan", "remember-all")) || !meters.length) return;
  showTab("meters");
  window.scrollTo({ top: 0, behavior: "smooth" });
}

/// Weitergabe: Werteauswahl des ersten Zählers aufklappen
async function pickFirst() {
  if (storageGet("tab") !== "transfer") {
    showTab("transfer");
    return;
  }
  const b = document.querySelector("[data-tr-pick]");
  if (b) {
    b.click();
    reveal(b, "start");
  }
}

/// Plan „Alle beheben“ öffnen (Knopf in den Auffälligkeiten)
function openFixPlan() {
  showTab("scan");
  const b = document.querySelector("[data-fix-all]");
  reveal(b, "center");
  b?.click();
}

/// Service fertig, Dauerbetrieb gewünscht: weiter mit Plan, Werten, Ausgang
function startOperation() {
  storageSet("guide_op", String(tree.active.project.id));
  renderGuide();
}

/// Projektformular beim Ausleseplan öffnen (der Plan gilt für alle Linien des Projekts)
function openPlanForm() {
  showTab("project");
  $("project-edit").click();
  const sel = $("tree-form").querySelector("[name=plan_kind]");
  reveal(sel, "center");
  sel.focus({ preventScroll: true });
}

/// Weitergabe: Einstellungen des Ausgangs aufklappen
function openOutput() {
  transfer.outputOpen = "mqtt";
  showTab("transfer");
  renderTransfer();
  reveal($("sec-outputs"), "start");
}

function renderGuide() {
  const box = $("guide");
  if (!box || !tree) return;
  const state = guideStep();
  const tabs = guideTabs(state?.op);
  // Reiter des Ablaufs: ✓ erledigt, Anzahl (Zähler, Datenpunkte), Punkt = als Nächstes
  const next = state?.step?.tab;
  const line = findLine(tree.active.line.id);
  const counts = { meters: meters.length, transfer: line?.datapoints?.count || 0 };
  for (const name of GUIDE_TABS) {
    const b = document.querySelector(`[data-tab-go=${name}]`);
    const inFlow = tabs.includes(name);
    const done = inFlow && !!state?.done[name];
    b.classList.toggle("done", done);
    b.classList.toggle("next", inFlow && name === next && !done);
    const n = b.querySelector(".ltab-n");
    const ok = b.querySelector(".ltab-ok");
    if (n) {
      n.textContent = counts[name] || "";
      n.hidden = !counts[name];
    }
    if (ok) {
      ok.innerHTML = ICON_CHECK;
      ok.hidden = !done || !!counts[name];
    }
  }
  // Knopf des nächsten Schritts hervorheben (nur einer)
  const s = state?.step;
  guideMarked?.classList.remove("go");
  guideMarked = jobRunning || !s ? null : s.mark ? s.mark() : s.target ? $(s.target) : null;
  guideMarked?.classList.add("go");
  // in der Leiste unten: nicht während der Bus arbeitet, ausgeblendet bis zum nächsten Schritt
  const hidden = !s || s.bar || jobRunning || planRuns.length || storageGet("guide_hidden") === s.key;
  const open = storageGet("guide_open") === "1";
  box.hidden = hidden;
  $("guide-more").hidden = hidden || !open;
  if (hidden) return;
  const key = [s.key, lang, s.arg, tabs.length, open].join("|");
  if (box.dataset.key === key) return;
  box.dataset.key = key;
  const n = tabs.indexOf(s.tab) + 1;
  const ready = s.key === "ready";
  const segs = tabs.map((name) => `<span class="${state.done[name] ? "on" : name === s.tab ? "now" : ""}"></span>`).join("");
  $("guide-more").innerHTML = `<span class="guide-segs" aria-hidden="true">${segs}</span><p>${esc(t("guide_" + s.key + "_body", s.arg))}</p>`;
  const more = t(open ? "guide_less" : "guide_more");
  box.innerHTML = `
    <span class="guide-count" title="${esc(ready ? t("guide_finished") : t("guide_next", n, tabs.length))}">${ready ? ICON_CHECK : `${n}/${tabs.length}`}</span>
    <span class="guide-title">${esc(t("guide_" + s.key))}</span>
    ${s.alt ? `<button type="button" class="small ghost" data-guide-alt>${esc(t("guide_" + s.alt.key + "_btn"))}</button>` : ""}
    <button type="button" class="small guide-go" data-guide-go>${esc(t("guide_" + s.key + "_btn"))}${ICON_NEXT}</button>
    <button type="button" class="icon-btn guide-toggle" data-guide-toggle aria-expanded="${open}" title="${esc(more)}" aria-label="${esc(more)}">${ICON_UP}</button>
    ${ui.iconBtn(ICON_DEL, t("guide_hide"), { "guide-hide": "" })}`;
  // Wechsel des Schritts: neu einblenden
  box.classList.remove("in");
  void box.offsetWidth;
  box.classList.add("in");
}

const ICON_CHECK = svg('<path d="M5 12l5 5L20 7"/>', 11, 3.2);
const ICON_NEXT = svg('<path d="M5 12h14M13 6l6 6-6 6"/>', 14);

$("guide").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-guide-hide]")) {
    const s = guideStep()?.step;
    if (s) storageSet("guide_hidden", s.key);
    renderGuide();
    return;
  }
  if (ev.target.closest("[data-guide-toggle]")) {
    storageSet("guide_open", storageGet("guide_open") === "1" ? "0" : "1");
    renderGuide();
    return;
  }
  if (ev.target.closest("[data-guide-go]")) guideStep()?.step?.go();
  if (ev.target.closest("[data-guide-alt]")) guideStep()?.step?.alt?.go();
});

// Linie ohne Anschluss (Reiter Projekt): direkt zum Verbindungsfeld
document.addEventListener("click", (ev) => {
  if (!ev.target.closest("[data-guide-conn]")) return;
  ev.preventDefault();
  openConnForm();
});
