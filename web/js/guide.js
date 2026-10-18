"use strict";

// Nächster Schritt: zeigt neuen wie erfahrenen Benutzern, was als Nächstes zu tun ist, mit genau
// einem Knopf. Die Reiter 1–4 zeigen dazu den Fortschritt (✓ erledigt, Punkt = als Nächstes).

const GUIDE_TABS = ["project", "scan", "meters", "transfer"];

/// Aktive Linie im Projektbaum (Zähler, letzte Auslesung, Datenpunkte)
function guideLine() {
  if (!tree) return null;
  const id = tree.active.line.id;
  return tree.projects.flatMap((p) => p.gateways).flatMap((g) => g.lines).find((l) => l.id === id) || null;
}

/// Zustand → nächster Schritt; null = alles eingerichtet
function guideStep() {
  const line = guideLine();
  if (!line) return null;
  const hasConn = !!(line.connection || connected);
  const okHits = lastJob && !lastJob.running ? lastJob.hits.filter((h) => h.status === "ok").length : 0;
  const read = meters.some((m) => m.last_read);
  const dps = line.datapoints?.count || 0;
  const done = { project: hasConn, scan: meters.length > 0, meters: read, transfer: dps > 0 };
  let step = null;
  if (!hasConn) step = { key: "conn_set", tab: "project", target: "connect", go: () => openConnForm() };
  else if (!connected) step = { key: "connect", tab: "scan", target: "connect", arg: line.connection, go: () => $("connect").click() };
  else if (!meters.length && okHits)
    step = { key: "remember", tab: "scan", target: "remember-all", arg: okHits, go: () => goClick("scan", "remember-all", "meters") };
  else if (!meters.length)
    step = { key: lastJob ? "scan_none" : "scan", tab: "scan", target: "scan-primary", go: () => goClick("scan", "scan-primary") };
  else if (!read) step = { key: "read", tab: "meters", target: "meters-read-all", arg: meters.length, go: () => goClick("meters", "meters-read-all") };
  else if (!dps) step = { key: storageGet("tab") === "transfer" ? "pick" : "transfer", tab: "transfer", go: () => pickFirst() };
  return { step, done };
}

/// Reiter öffnen, Knopf zeigen und drücken (wenn er gerade geht); `then` = Reiter, in dem es danach
/// weitergeht (sobald die Zählerliste gefüllt ist)
function goClick(tabName, id, then = null) {
  showTab(tabName);
  const b = $(id);
  reveal(b, "center");
  if (b.disabled || b.hidden) return;
  b.click();
  if (!then) return;
  let tries = 0;
  const wait = setInterval(() => {
    if (meters.length || ++tries > 40) {
      clearInterval(wait);
      if (meters.length) {
        showTab(then);
        window.scrollTo({ top: 0, behavior: "smooth" });
      }
    }
  }, 250);
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

/// Verbindungsfeld zeigen, Port ins Auge fassen
function openConnForm() {
  showTab("scan");
  $("conn-edit").click();
  reveal($("sec-conn"), "start");
  setTimeout(() => $("port").focus({ preventScroll: true }), 300);
}

function renderGuide() {
  const box = $("guide");
  if (!box || !tree) return;
  const state = guideStep();
  const tab = storageGet("tab") || "scan";
  // Reiter 1–4: erledigt / als Nächstes
  const next = state?.step?.tab;
  for (const name of GUIDE_TABS) {
    const b = document.querySelector(`[data-tab-go=${name}]`);
    const no = b.querySelector(".tab-no");
    const done = !!state?.done[name];
    b.classList.toggle("done", done);
    b.classList.toggle("next", name === next && !done);
    no.innerHTML = done ? CHECK_SVG : String(GUIDE_TABS.indexOf(name) + 1);
  }
  // Knopf des nächsten Schritts hervorheben (nur einer)
  document.querySelectorAll(".go").forEach((e) => e.classList.remove("go"));
  const s = state?.step;
  if (s?.target && !jobRunning) $(s.target)?.classList.add("go");
  if (s?.key === "pick") document.querySelector("[data-tr-pick]")?.classList.add("go");
  // Karte: nur in den Reitern 1–4, nicht während der Bus arbeitet, ausgeblendet bis zum nächsten Schritt
  const hidden = !s || jobRunning || planRuns.length || !GUIDE_TABS.includes(tab) || storageGet("guide_hidden") === s.key;
  box.hidden = hidden;
  if (hidden) return;
  if (box.dataset.key === s.key + lang + (s.arg ?? "")) return;
  box.dataset.key = s.key + lang + (s.arg ?? "");
  const n = GUIDE_TABS.indexOf(s.tab) + 1;
  const segs = GUIDE_TABS.map((name) => `<span class="${state.done[name] ? "on" : name === s.tab ? "now" : ""}"></span>`).join("");
  box.innerHTML = `
    <div class="guide-no" aria-hidden="true">${n}</div>
    <div class="guide-text">
      <div class="guide-eyebrow"><span>${esc(t("guide_next", n))}</span><span class="guide-segs" aria-hidden="true">${segs}</span></div>
      <h2>${esc(t("guide_" + s.key))}</h2>
      <p>${esc(t("guide_" + s.key + "_body", s.arg))}</p>
    </div>
    <div class="guide-actions">
      <button type="button" class="guide-go" data-guide-go>${esc(t("guide_" + s.key + "_btn"))}<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg></button>
      <button type="button" class="icon-btn" data-guide-hide title="${esc(t("guide_hide"))}" aria-label="${esc(t("guide_hide"))}">${ICON_DEL}</button>
    </div>`;
  // Wechsel des Schritts: neu einblenden
  box.classList.remove("in");
  void box.offsetWidth;
  box.classList.add("in");
}

const CHECK_SVG =
  '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12l5 5L20 7"/></svg>';

$("guide").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-guide-hide]")) {
    const s = guideStep()?.step;
    if (s) storageSet("guide_hidden", s.key);
    renderGuide();
    return;
  }
  if (ev.target.closest("[data-guide-go]")) guideStep()?.step?.go();
});

// Linie ohne Anschluss (Reiter Projekt): direkt zum Verbindungsfeld
document.addEventListener("click", (ev) => {
  if (ev.target.closest("[data-guide-conn]")) openConnForm();
});
