"use strict";

// Reiter, Hilfe und Start (zuletzt laden)

// ------------------------------------------------------------ Reiter, Hilfe, Verbindung

const TABS = ["project", "scan", "meters", "tools", "sim"];

function showTab(name) {
  if (!TABS.includes(name)) name = "scan";
  document.querySelectorAll("main [data-tab]").forEach((s) => (s.hidden = s.dataset.tab !== name));
  document.querySelectorAll("[data-tab-go]").forEach((b) => {
    b.classList.toggle("active", b.dataset.tabGo === name);
    b.setAttribute("aria-selected", b.dataset.tabGo === name);
  });
  storageSet("tab", name);
  if (name !== "meters" && liveId) {
    stopLive();
    renderMeters();
  }
  if (name === "sim") loadSim().catch((e) => toast(e.message, true));
}

/// Element zeigen: richtigen Reiter öffnen, dann hinscrollen
function reveal(el, block = "nearest") {
  if (!el) return;
  const section = el.closest("[data-tab]");
  if (section?.hidden) showTab(section.dataset.tab);
  requestAnimationFrame(() => el.scrollIntoView({ behavior: "smooth", block }));
}

document.querySelectorAll("[data-tab-go]").forEach(
  (b) =>
    (b.onclick = () => {
      showTab(b.dataset.tabGo);
      window.scrollTo({ top: 0 });
    })
);

// Links auf Bereiche (Statusleiste, Pfad, "zurück zur Trefferliste" …) öffnen den passenden Reiter
document.addEventListener("click", (ev) => {
  const a = ev.target.closest('a[href^="#"]');
  if (!a || a.getAttribute("href") === "#") return;
  const el = document.getElementById(a.getAttribute("href").slice(1));
  if (!el || !el.closest("[data-tab]")) return;
  ev.preventDefault();
  reveal(el, "start");
});

$("help-toggle").onclick = () => {
  const on = !document.body.classList.contains("show-help");
  document.body.classList.toggle("show-help", on);
  $("help-toggle").setAttribute("aria-pressed", on);
  storageSet("help", on ? "1" : "0");
};
if (storageGet("help") === "1") {
  document.body.classList.add("show-help");
  $("help-toggle").setAttribute("aria-pressed", "true");
}

$("conn-edit").onclick = () => {
  connEditing = true;
  $("sec-conn").classList.remove("compact");
  $("conn-summary").hidden = true;
};
$("conn-sum-disconnect").onclick = () => $("disconnect").click();

showTab(storageGet("tab") || "scan");

// ------------------------------------------------------------ Start

setLang(lang);
loadPorts().catch((e) => toast(e.message, true));
loadTree()
  .then(() => {
    // nicht verbunden: Anschluss der aktiven Linie in die Felder übernehmen
    const l = tree.active.line;
    if (!connected && l.connection) {
      $("port-manual").value = l.connection;
      $("baud").value = l.baud;
      $("timeout").value = l.timeout_ms;
    }
  })
  .catch((e) => toast(e.message, true));
// letztes Scan-Ergebnis nach dem Neuladen wieder anzeigen (läuft ein Scan, übernimmt pollJob);
// erst nach dem Status, denn die Bewertung braucht die Bus-Baudrate
refreshStatus()
  .then(() => (jobRunning ? null : api("last-scan")))
  .then((job) => {
    if (job && job.hits.length) renderJob(job);
  })
  .catch(() => {});
loadMeters().catch((e) => toast(e.message, true));
loadMqtt().catch(() => {});
setInterval(() => pollLog().catch(() => {}), 1000);
setInterval(() => !jobRunning && refreshStatus().catch(() => {}), 5000);
