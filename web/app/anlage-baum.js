// Bereich Anlage: Zeilen des Projektbaums (Projekt, Gateways, Linien, Felder für Neues). Reine
// Bausteine; `ctx`: { sel (gewählter Schlüssel), closed (zugeklappte Gateways), activeId }.

import { opText } from "./view.js";
import { allLines, sum, opSwitch, lineState, lineInfo } from "./anlage-zustand.js";

export function rowHtml(x, ctx) {
  if (x.kind === "project") return projectRow(x.p, ctx);
  if (x.kind === "gateway") return gatewayRow(x, ctx);
  if (x.kind === "line") return lineRow(x, ctx);
  if (x.kind === "new")
    return `<div class="row new"><input data-new="${x.what}" placeholder="${esc(t(`an_new_${x.what}_ph`))}" aria-label="${esc(
      t(`an_new_${x.what}_ph`)
    )}"><button type="button" data-create="${x.what}">${esc(t("an_create"))}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button></div>`;
  return `<div class="row add">${x.empty ? `<span class="sub">${esc(t("an_no_gateways"))}</span>` : ""}<button type="button" class="${
    x.empty ? "" : "quiet"
  }" data-add-gateway>${esc(t("gw_add"))}</button></div>`;
}

const rowAttrs = (key, cls, sel) => `class="row ${cls}${sel === key ? " active" : ""}" role="option" id="an-${key.replace(":", "-")}" aria-selected="${sel === key}"`;

function projectRow(p, { sel }) {
  const lines = allLines(p);
  const issues = sum(lines, (l) => l.issues?.count);
  return `<div ${rowAttrs("p", "pj", sel)}>
    <span class="name"><span class="cut" title="${esc(p.name)}">${esc(p.name)}</span></span>
    <span class="cut sub">${esc([p.customer, p.site].filter(Boolean).join(" · "))}</span>
    <span class="num">${sum(lines, (l) => l.meters)}</span>
    <span class="num">${sum(lines, (l) => l.datapoints?.count)}</span>
    <span class="cut sub">${p.plan && p.plan !== "off" ? esc(planText(p.plan)) : ""}</span>
    <span class="cut">${issues ? `<span class="chip warn">${esc(t("ln_issues", issues))}</span>` : ""}</span>
    <span></span>
  </div>`;
}

function gatewayRow({ g, i, n }, { sel, closed: shut }) {
  const closed = shut.has(g.id);
  const ops = g.lines.filter((l) => l.operation).length;
  const issues = sum(g.lines, (l) => l.issues?.count);
  return `<div ${rowAttrs(`g:${g.id}`, "gw", sel)} aria-expanded="${!closed}">
    <span class="name"><button type="button" class="icon chev" data-toggle="${g.id}" aria-label="${esc(g.name)}" aria-expanded="${!closed}">${closed ? "▸" : "▾"}</button><span class="cut" title="${esc(g.name)}">${esc(g.name)}</span></span>
    <span class="cut sub">${g.address ? `<span class="mono">${esc(g.address)}</span> · ` : ""}${esc(g.lines.length ? t("ps_lines", g.lines.length) : t("an_no_lines"))}</span>
    <span class="num">${sum(g.lines, (l) => l.meters)}</span>
    <span class="num">${sum(g.lines, (l) => l.datapoints?.count)}</span>
    <span class="cut">${ops ? `<span class="chip ok">${esc(t("n_op_count", ops))}</span>` : ""}</span>
    <span class="cut">${issues ? `<span class="chip warn">${esc(t("ln_issues", issues))}</span>` : ""}</span>
    <span class="acts"><button type="button" class="quiet" data-add-line="${g.id}">${esc(t("ln_add"))}</button>${moveButtons("gateways", g.id, i, n)}</span>
  </div>`;
}

function lineRow({ l, i, n }, { sel, activeId }) {
  const current = l.id === activeId;
  return `<div ${rowAttrs(`l:${l.id}`, "ln" + (current ? " current" : ""), sel)}>
    <span class="name"><span class="cut" title="${esc(l.name)}">${esc(l.name)}</span>${current ? ` <span class="chip ok">${esc(t("ln_active"))}</span>` : ""}</span>
    <span class="cut mono">${l.connection ? esc(`${portText(l.connection)} · ${l.baud}`) : `<span class="warn">${esc(t("ln_no_conn"))}</span>`}</span>
    <span class="num">${l.meters}</span>
    <span class="num">${l.datapoints?.count || ""}</span>
    <span class="op">${opSwitch(l)}<span class="cut ${l.operation ? "ok" : "sub"}">${esc(opText(l))}</span></span>
    <span class="cut state" title="${esc(lineInfo(l))}">${lineState(l)}</span>
    <span class="acts">${current ? "" : `<button type="button" class="quiet" data-use="${l.id}" title="${esc(t("an_use_title"))}">${esc(t("ln_use"))}</button>`}${moveButtons(
      "lines",
      l.id,
      i,
      n
    )}</span>
  </div>`;
}

/// ↑↓ zum Verschieben; ganz oben bzw. unten gesperrt, bei nur einem Eintrag keine
const moveButtons = (what, id, i, n) =>
  n < 2
    ? ""
    : [-1, 1]
        .map(
          (dir) =>
            `<button type="button" class="icon" data-move="${what}" data-id="${id}" data-dir="${dir}"${(dir < 0 ? i === 0 : i === n - 1) ? " disabled" : ""} title="${esc(
              t(dir < 0 ? "move_up" : "move_down")
            )}" aria-label="${esc(t(dir < 0 ? "move_up" : "move_down"))}">${dir < 0 ? "↑" : "↓"}</button>`
        )
        .join("");
