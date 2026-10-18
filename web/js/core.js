"use strict";

// Grundlagen der App: Sprache umschalten, Bus-Anfragen, Meldungen, Knöpfe (Basis in base.js)

async function setLang(l) {
  lang = l;
  storageSet("lang", l);
  document.documentElement.lang = l;
  document.querySelectorAll("[data-lang]").forEach((b) => b.classList.toggle("active", b.dataset.lang === l));
  document.querySelectorAll("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n)));
  document.querySelectorAll("[data-i18n-ph]").forEach((el) => (el.placeholder = t(el.dataset.i18nPh)));
  document.querySelectorAll("[data-i18n-title]").forEach((el) => (el.title = t(el.dataset.i18nTitle)));
  labels = await api("labels?lang=" + l).catch(() => ({}));
  updateExportLinks();
  if (lastReading) renderReading(lastReading);
  if (lastJob) renderJob(lastJob);
  renderMeters();
  renderTree();
  renderHistory();
  renderBusbar();
  // Bezeichnungen der Datenpunkte kommen vom Server in der gewählten Sprache
  if (transfer.data) loadTransfer().catch(() => {});
  if (sim) loadSim().catch(() => {});
  refreshStatus().catch(() => {});
}

document.querySelectorAll("[data-lang]").forEach((b) => (b.onclick = () => setLang(b.dataset.lang)));


// Design: auto (wie System), hell oder dunkel; das Setzen beim Laden steht in index.html
function setTheme(theme) {
  if (theme === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  storageSet("theme", theme);
  document.querySelectorAll("[data-theme-set]").forEach((b) => b.classList.toggle("active", b.dataset.themeSet === theme));
}
document.querySelectorAll("[data-theme-set]").forEach((b) => (b.onclick = () => setTheme(b.dataset.themeSet)));
setTheme(["light", "dark"].includes(storageGet("theme")) ? storageGet("theme") : "auto");

// ------------------------------------------------------------ Helfer

let connected = false;
let jobRunning = false;
let logSeq = 0;
let lastReading = null;
let lastJob = null;
/// Art des zuletzt angezeigten Jobs (auch "Alle auslesen"), z.B. für die Abbruch-Frage
let currentJobKind = null;
let connBaud = null;
let activeLineId = null;
/// Verbindungsformular trotz Verbindung offen ("Ändern")
let connEditing = false;
/// laufende geplante Auslesungen, Zustand der Uhrzeit, irgendwo ein Plan eingeschaltet
let planRuns = [];
let timeState = null;
let planActive = false;
let disk = null;
let mqttStatus = null;
/// zuletzt gespeicherte Auffälligkeiten (nur bei Änderung neu speichern)
let savedIssues = null;

function saveIssues(items) {
  if (!activeLineId) return;
  const body = { count: items.length, items };
  const text = JSON.stringify(body);
  if (text === savedIssues) return;
  savedIssues = text;
  request(`lines/${activeLineId}/issues`, { method: "PUT", body })
    .then(() => tree && loadTree())
    .catch(() => {});
}
let checking = false;
let conn = null;
let baudsTouched = false;
const BAUDS = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];

/// Anfragen, die den Bus belegen: Pfad, Text, Bereich für "Ansehen"
const BUS_ACTIONS = [
  [/^read$|^meters\/\d+\/read$/, "busy_read", "#sec-read"],
  [/^set-address$/, "busy_set", "#sec-config"],
  [/^switch-baud$/, "busy_baud", "#sec-config"],
  [/^detect-baud$/, "busy_detect", "#sec-config"],
  [/^reset$/, "busy_reset", "#sec-config"],
  [/^raw$/, "busy_raw", "#sec-log"],
  [/^free-address$/, "busy_free", "#sec-config"],
];

async function api(path, options = {}) {
  // "Alle beheben" zeigt seinen eigenen Fortschritt in der Leiste
  const busy = !planRunning && options.method === "POST" && BUS_ACTIONS.find(([re]) => re.test(path));
  if (!busy) return request(path, options);
  busbar({ text: t(busy[1]), link: busy[2] });
  try {
    return await request(path, options);
  } finally {
    busbar(null);
  }
}


onToken = () => updateExportLinks();

function updateExportLinks() {
  $("meters-export").href = `api/meters/csv?lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  const extra = `&lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  $("export-csv").href = "api/export?format=csv" + extra;
  $("export-json").href = "api/export?format=json" + extra;
}

function toast(msg, isError = false) {
  const el = $("toast");
  el.textContent = msg;
  el.className = isError ? "error" : "";
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), isError ? 6000 : 3000);
}


async function guarded(button, fn) {
  button.disabled = true;
  try {
    await fn();
  } catch (e) {
    toast(e.message, true);
  } finally {
    // Knöpfe ohne Verbindungsbedarf wieder freigeben; die anderen regelt updateButtons
    button.disabled = false;
    updateButtons();
  }
}

function updateButtons() {
  document.querySelectorAll(".needs-conn").forEach((b) => {
    b.disabled = !connected || jobRunning || checking;
    // gesperrt ohne Verbindung: sagen warum (Knöpfe mit eigenem Hinweis behalten ihn)
    if (!b.dataset.i18nTitle) b.title = connected ? "" : t("need_conn");
  });
  // ohne markierte Baudrate gibt es nichts zu prüfen oder zu scannen
  const none = !$("scan-bauds").querySelector("input:checked");
  $("bauds-empty").hidden = !none;
  if (none) ["scan-primary", "scan-secondary"].forEach((id) => ($(id).disabled = true));
  const all = !$("scan-bauds").querySelector("input:not(:checked)");
  $("bauds-all").textContent = t(all ? "bauds_none" : "bauds_all");
  $("bauds-all").disabled = checking || jobRunning;
  $("connect").disabled = jobRunning;
  $("disconnect").disabled = !connected || jobRunning;
  $("connect").hidden = connected;
  $("disconnect").hidden = !connected;
  $("scan-cancel").hidden = !jobRunning;
  // guide.js lädt erst nach den Reitern
  if (typeof renderGuide === "function") renderGuide();
}
