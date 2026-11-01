// Bereich Übersicht: wie die aktive Linie steht, in vier Karten; jede führt an die Stelle, an der man
// es ändert. Der nächste Schritt steht darüber (app-statusleiste.js, Konzept Bedienung §17).

import { state, watch, activeLine, faulty } from "./store.js";
import { html, styles, opText, outputChips } from "./view.js";
import { loadMeters, takeTime, report } from "./data.js";
import { goTo } from "./app-statusleiste.js";
import { lineState, serviceNotice, diskHtml } from "./anlage-zustand.js";
import { backupState } from "./weitergabe-ausgaenge.js";
import "./anlage-texte.js";
import "./weitergabe-texte.js";
import "./uebersicht-texte.js";

let root = null;
let backup = null; // /api/backup/auto, neu geladen bei jedem Öffnen
let shownLine = null; // Linie, zu der die Zählerliste geladen ist

const open = () => state.area === "uebersicht";

const chip = (cls, word) => `<span class="chip ${cls}">${esc(word)}</span>`;

const card = (title, body, action = "") => `<div class="card"><h2>${esc(title)}</h2>${body}${action ? `<div class="acts">${action}</div>` : ""}</div>`;

const link = (href, word) => `<a class="quiet" href="${href}">${esc(word)}</a>`;

const row = (label, body) => `<div class="kv"><span class="sub">${esc(label)}</span><span>${body}</span></div>`;

function metersCard(l) {
  const bad = (state.meters || []).filter(faulty).length;
  const ok = Math.max(0, l.meters - bad - l.unread);
  const chips = [ok && chip("ok", t("ov_ok", ok)), bad && chip("err", t("ov_faulty", bad)), l.unread && chip("", t("ov_unread", l.unread))].filter(Boolean).join(" ");
  return card(
    t("tab_meters"),
    `<div class="big"><span class="mono">${l.meters}</span></div><div class="chips">${chips}</div>`,
    `${bad ? `<button type="button" class="quiet" data-show-faulty>${esc(t("guide_faulty_btn"))}</button>` : ""}${link("#/meters", t("ov_to_meters"))}`
  );
}

function operationCard(l, s) {
  return card(
    t("op_chip"),
    `<p class="word">${esc(opText(l))}</p><div class="chips">${lineState(l)}</div>${l.operation && !s.background ? `<div class="chips">${serviceNotice(s)}</div>` : ""}`,
    link("#/site", t("ov_change_plan"))
  );
}

function transferCard(l, s) {
  const chips = outputChips(s);
  return card(
    t("tab_transfer"),
    `<div class="big"><span class="mono">${l.datapoints?.count || 0}</span> <span class="sub">${esc(t("an_col_dp"))}</span></div><div class="chips">${chips || `<span class="sub">${esc(t("ov_no_output"))}</span>`}</div>`,
    link("#/transfer", t("ov_to_transfer"))
  );
}

function backupRow() {
  if (!backup) return "";
  const [cls, word, sentence] = backupState(backup);
  return row(t("wg_bk_title"), `${chip(cls, word)}${sentence ? ` <span class="sub">${esc(sentence)}</span>` : ""}`);
}

function deviceCard(s) {
  const time = s.time.confirmed ? esc(t("ps_time_ok", s.time.source)) : `${chip("warn", t("time_unsure"))} <button type="button" class="tonal" data-take-time>${esc(t("ps_time_take"))}</button>`;
  const w = s.warnings;
  const messages = w?.open ? `<button type="button" class="${w.unseen ? "tonal" : "quiet"}" data-show-messages>${esc(t("n_warn", w.unseen || w.open))}</button>` : esc(t("n_warn_empty"));
  const rows =
    row(t("ps_time"), time) +
    row(t("ps_disk"), `${diskHtml(s.disk)} <button type="button" class="quiet" data-open-settings>${esc(t("settings_short"))}</button>`) +
    backupRow() +
    row(t("ov_messages"), messages);
  return `<div class="device">${card(`${t("ov_device")} · mbusio ${s.version}`, `<div class="kvs">${rows}</div>`)}</div>`;
}

function render() {
  const l = activeLine();
  const s = state.status;
  if (!root || !open() || !l || !s) return;
  html(root, metersCard(l) + operationCard(l, s) + transferCard(l, s) + deviceCard(s));
}

async function loadBackup() {
  if (!open()) return;
  try {
    backup = await request("backup/auto");
    render();
  } catch (e) {
    report(e);
  }
}

/// Die Zählerliste lädt sonst erst der Bereich Zähler; „gestört“ braucht sie schon hier
function loadLineMeters() {
  const line = state.status?.active?.line?.id;
  if (!open() || !line || line === shownLine) return;
  shownLine = line;
  loadMeters().catch(report);
}

export function mount(pane) {
  styles("uebersicht");
  pane.innerHTML = `<div class="cards"></div>`;
  root = pane.firstElementChild;
  pane.addEventListener("click", (ev) => {
    const hit = (sel) => ev.target.closest(sel);
    if (hit("[data-show-faulty]")) return goTo("zaehler", "[data-filter=err]");
    if (hit("[data-take-time]")) return takeTime();
    if (hit("[data-show-messages]")) return document.querySelector("[data-warn-open]")?.click();
    // nach diesem Klick öffnen, sonst schliesst ihn der Klick gleich wieder
    if (hit("[data-open-settings]")) setTimeout(() => document.getElementById("settings").click());
  });
  watch(["tree", "status", "meters", "area"], render);
  watch(["area"], loadBackup);
  watch(["status", "area"], loadLineMeters);
}
