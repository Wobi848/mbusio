// Bereich Anlage: Details rechts für Projekt, Gateway und Linie (Felder, Plan, Dauerbetrieb).
// Reine Bausteine; gespeichert wird in anlage.js.

import { opText } from "./view.js";
import { allLines, sum, opSwitch, lineState } from "./anlage-zustand.js";

const PLAN_KINDS = ["off", "monthly", "daily", "hourly", "15min"];
/// Pläne mit Uhrzeit
export const TIMED = ["daily", "monthly"];
const THIN_DAYS = [0, 30, 90, 365];

/// Details des gewählten Elements (`x` aus picked()); `ctx`: { p (Projekt), activeId (aktive
/// Linie), ports (bekannte Anschlüsse) }
export function detailHtml(x, ctx) {
  if (x.kind === "project") return projectDetail(x.p);
  if (x.kind === "gateway") return gatewayDetail(x.g);
  return lineDetail(x.l, x.g, ctx);
}

const field = (kind, name, label_, value, extra = "") =>
  `<label><span>${esc(label_)}</span><input data-kind="${kind}" data-field="${name}" value="${esc(value)}" data-saved="${esc(value)}"${extra}></label>`;

const head = (eyebrow, name, sub = "") => `<header class="dhead"><div><span class="eyebrow">${esc(eyebrow)}</span><div><b>${esc(name)}</b></div>${sub ? `<div class="sub">${sub}</div>` : ""}</div></header>`;

function projectDetail(p) {
  const lines = allLines(p);
  return `${head(t("project"), p.name, esc(t("an_lines_meters", t("ps_lines", lines.length), sum(lines, (l) => l.meters), sum(lines, (l) => l.datapoints?.count))))}
    <div class="fields">
      ${field("project", "name", t("f_name"), p.name)}
      ${field("project", "customer", t("f_customer"), p.customer)}
      ${field("project", "site", t("f_site"), p.site)}
      ${field("project", "technician", t("f_technician"), p.technician)}
      ${field("project", "order_no", t("f_order"), p.order_no)}
      <label><span>${esc(t("f_remarks"))}</span><textarea data-kind="project" data-field="remarks" data-saved="${esc(p.remarks)}" rows="3">${esc(p.remarks)}</textarea></label>
      ${planField("project", p.plan || "off", null)}
      <label><span>${esc(t("thin"))}</span><select data-thin>${THIN_DAYS.map(
        (d) => `<option value="${d}"${(p.thin_days || 0) === d ? " selected" : ""}>${esc(d ? t("thin_days", d) : t("thin_off"))}</option>`
      ).join("")}</select></label>
    </div>
    <p class="sub">${esc(t("thin_hint"))}</p>
    <div class="actions">
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-clear>${esc(t("project_clear"))}</button>
      <button type="button" class="quiet danger-text" data-delete="project">${esc(t("project_delete"))}</button>
    </div>`;
}

function gatewayDetail(g) {
  return `${head(t("form_gateway"), g.name, esc(t("an_lines_meters", t("ps_lines", g.lines.length), sum(g.lines, (l) => l.meters), sum(g.lines, (l) => l.datapoints?.count))))}
    <div class="fields">
      ${field("gateway", "name", t("f_name"), g.name)}
      ${field("gateway", "address", t("f_address"), g.address, ' class="mono"')}
      ${field("gateway", "note", t("f_note"), g.note)}
    </div>
    <div class="actions">
      <button type="button" class="tonal" data-add-line="${g.id}">${esc(t("ln_add"))}</button>
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-delete="gateway">${esc(t("delete"))}</button>
    </div>`;
}

function lineDetail(l, g, { p, activeId, ports: known }) {
  const current = l.id === activeId;
  const ports = (known || []).map((x) => `<option value="${esc(x.port)}">${esc(x.label || "")}</option>`).join("");
  return `${head(t("form_line"), l.name, `${esc(g.name)} · ${esc(t("an_meters_dp", l.meters, l.datapoints?.count || 0))}`)}
    <div class="actions">${
      current ? `<span class="chip ok">${esc(t("ln_active"))}</span>` : `<button type="button" class="tonal" data-use="${l.id}" title="${esc(t("an_use_title"))}">${esc(t("ln_use"))}</button>`
    }</div>
    <div class="fields">
      ${field("line", "name", t("f_name"), l.name)}
      ${field("line", "connection", t("f_connection"), l.connection, ` class="mono" list="an-ports" placeholder="${esc(t("f_connection_ph"))}"`)}
      <datalist id="an-ports">${ports}</datalist>
      <label><span>${esc(t("baud"))}</span><select data-kind="line" data-field="baud">${BAUDS.map((b) => `<option${b === l.baud ? " selected" : ""}>${b}</option>`).join("")}</select></label>
      ${planField("line", l.plan ?? null, t("plan_inherit_project", planText(p.plan || "off")))}
    </div>
    <label class="opline${l.operation ? " on" : ""}">${opSwitch(l, true)}<b>${esc(t("op_chip"))}</b><span class="${l.operation ? "ok" : "sub"}">${esc(opText(l))}</span></label>
    <div class="chips">${lineState(l)}</div>
    <div class="actions">
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-delete="line">${esc(t("delete"))}</button>
    </div>`;
}

/// Ausleseplan als Art und Uhrzeit (nur täglich, monatlich); `inherit` = Text für „wie Projekt“
function planField(kind, value, inherit) {
  const [k, time] = value ? value.split(" ") : ["", null];
  const opts = (inherit ? [["", inherit]] : []).concat(PLAN_KINDS.map((x) => [x, t("plan_opt_" + x)]));
  return `<label><span>${esc(t("plan"))}</span><span class="planfield"><select data-plan="${kind}">${opts
    .map(([v, txt]) => `<option value="${v}"${v === k ? " selected" : ""}>${esc(txt)}</option>`)
    .join("")}</select><input data-plan-time="${kind}" class="mono" value="${esc(time || "00:05")}" maxlength="5" placeholder="HH:MM" aria-label="${esc(t("plan_time"))}"${
    TIMED.includes(k) ? "" : " hidden"
  }></span></label>`;
}
