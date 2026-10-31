// Meldungen (Konzept E-Mail 5.1): Probleme, die mbusio im Hintergrund erkennt, in der Statusleiste;
// Liste mit Sprung an die Stelle und „gesehen“. Behobene verschwinden von selbst.

import { state, watch } from "./store.js";
import { html } from "./view.js";
import { onEscape } from "./keys.js";
import { useLine, report } from "./data.js";
import { goTo } from "./app-statusleiste.js";

let open = false;
let release = null;
let items = [];

export function startMeldungen() {
  watch(["status"], renderChip);
  // neue oder behobene Probleme: offene Liste nachladen
  let last = "";
  watch(["status"], () => {
    const w = JSON.stringify(state.status?.warnings || {});
    if (w !== last && open) load();
    last = w;
  });
}

function renderChip() {
  const w = state.status?.warnings;
  html(
    $("warnings"),
    w?.open
      ? `<button type="button" class="chip ${w.unseen ? "warn" : "quiet"}" data-warn-open title="${esc(t("n_warn_title"))}" aria-expanded="${open}">${esc(t("n_warn", w.unseen || w.open))}</button>`
      : ""
  );
}

async function load() {
  try {
    items = (await request("warnings")).items;
  } catch (e) {
    report(e);
  }
  renderList();
}

function row(w) {
  return `<div class="wrow${w.seen ? " seen" : ""}">
    <span class="when mono">${esc(slotText(w.since))}</span>
    <span class="what">${esc(w.text)}${w.place ? `<span class="sub"> · ${esc(w.place)}</span>` : ""}</span>
    <button type="button" class="tonal" data-warn-go="${esc(w.key)}">${esc(t("n_warn_go"))}</button>
    ${w.seen ? "" : `<button type="button" class="quiet" data-warn-seen="${esc(w.key)}">${esc(t("n_warn_seen"))}</button>`}
  </div>`;
}

function renderList() {
  html($("warn-list"), items.length ? items.map(row).join("") : `<div class="empty">${esc(t("n_warn_empty"))}</div>`);
  $("warn-seen-all").hidden = !items.some((w) => !w.seen);
}

function toggle(to = !open) {
  open = to;
  $("warnpanel").hidden = !open;
  release?.();
  release = open ? onEscape(() => toggle(false)) : null;
  renderChip();
  if (open) load();
}

async function seen(keys) {
  try {
    await request("warnings/seen", { method: "POST", body: { keys } });
    for (const w of items) if (!keys.length || keys.includes(w.key)) w.seen = true;
    renderList();
    state.status.warnings.unseen = items.filter((w) => !w.seen).length;
    renderChip();
  } catch (e) {
    report(e);
  }
}

/// An die Stelle des Problems: Zähler in seiner Linie, Linie in der Anlage, Abrechnung des Projekts
async function go(w) {
  toggle(false);
  if (w.line && w.line !== state.status?.active?.line?.id) await useLine(w.line).catch(report);
  if (w.meter) return goTo("zaehler", `#meter-${w.meter}`);
  if (w.kind === "line") return (location.hash = "#/site");
  if (w.project) return (location.hash = `#/transfer/billing?project=${w.project}`);
  if (w.kind === "mqtt") return (location.hash = "#/transfer");
  if (w.kind === "disk") document.getElementById("settings").click();
}

$("warnings").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-warn-open]")) toggle();
});
$("warn-close").addEventListener("click", () => toggle(false));
$("warn-seen-all").addEventListener("click", () => seen([]));
$("warnpanel").addEventListener("click", (ev) => {
  const s = ev.target.closest("[data-warn-seen]");
  if (s) return seen([s.dataset.warnSeen]);
  const g = ev.target.closest("[data-warn-go]");
  if (g) go(items.find((w) => w.key === g.dataset.warnGo));
  if (ev.target.closest("a[href]")) toggle(false);
});
