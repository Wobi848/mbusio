// Bereich Anlage: Projektbaum (Gateways › Linien) links mit festen Spalten, rechts Projekt,
// Gateway oder Linie bearbeiten (Konzept neue Oberfläche §3.2, Schritt 3). Zustand der Anlage
// als Raster oben, nur wenn eine Linie des Projekts im Dauerbetrieb ist.

import { state, set, watch } from "./store.js";
import { html, text, list, ask, toast, styles, texts, outputWord, opPossible, opText } from "./view.js";
import { bind } from "./keys.js";
import { loadTree, loadMeters, loadStatus, reload, useLine, report, switchOperation } from "./data.js";

texts(
  {
    an_col_name: "Name",
    an_col_conn: "Anschluss",
    an_col_meters: "Zähler",
    an_col_dp: "Datenpunkte",
    an_col_op: "Dauerbetrieb · Plan",
    an_col_state: "Zustand",
    an_new_project: "+ Projekt",
    an_new_project_ph: "Name des neuen Projekts",
    an_new_gateway_ph: "Name des Gateways",
    an_new_line_ph: "Name der Linie",
    an_create: "Anlegen",
    an_created: (x) => `„${x}“ angelegt`,
    an_deleted: (x) => `„${x}“ gelöscht`,
    an_no_gateways: "Noch kein Gateway in diesem Projekt.",
    an_no_lines: "keine Linie",
    an_line_down: "gestört",
    an_read_at: (x) => `gelesen ${x}`,
    an_use_title: "Diese Linie aktiv machen: Scan, Zähler und Werkzeuge arbeiten dann auf ihr (Enter)",
    an_pick: "Projekt, Gateway oder Linie in der Liste wählen (↑↓).",
    an_key_use: "Gewählte Linie aktiv machen",
    an_time_bad: "Uhrzeit als HH:MM eingeben, z.B. 00:05",
    an_lines_meters: (lines, meters, dp) => `${lines} · ${meters} Zähler · ${dp} Datenpunkte`,
    an_meters_dp: (meters, dp) => `${meters} Zähler · ${dp} Datenpunkte`,
    an_svc_how: "Anleitung",
  },
  {
    an_col_name: "Name",
    an_col_conn: "Connection",
    an_col_meters: "Meters",
    an_col_dp: "Datapoints",
    an_col_op: "Continuous · plan",
    an_col_state: "State",
    an_new_project: "+ Project",
    an_new_project_ph: "Name of the new project",
    an_new_gateway_ph: "Name of the gateway",
    an_new_line_ph: "Name of the line",
    an_create: "Create",
    an_created: (x) => `“${x}” created`,
    an_deleted: (x) => `“${x}” deleted`,
    an_no_gateways: "No gateway in this project yet.",
    an_no_lines: "no line",
    an_line_down: "faulty",
    an_read_at: (x) => `read ${x}`,
    an_use_title: "Make this line active: scan, meters and tools then work on it (Enter)",
    an_pick: "Pick a project, gateway or line in the list (↑↓).",
    an_key_use: "Make the selected line active",
    an_time_bad: "Enter the time as HH:MM, e.g. 00:05",
    an_lines_meters: (lines, meters, dp) => `${lines} · ${meters} meters · ${dp} datapoints`,
    an_meters_dp: (meters, dp) => `${meters} meters · ${dp} datapoints`,
    an_svc_how: "Guide",
  }
);

const PLAN_KINDS = ["off", "monthly", "daily", "hourly", "15min"];
const TIMED = ["daily", "monthly"];
const THIN_DAYS = [0, 30, 90, 365];
const README = "https://github.com/Wobi848/mbusio";
const open = () => state.area === "anlage";

/// Einstellungen je Projekt: gewähltes Element („p“, „g:3“, „l:7“), zugeklappte Gateways,
/// Feld für ein neues Gateway („gateway“) oder eine neue Linie (Gateway-ID)
const perProject = new Map();
let root = null;
let view = null;
let projectId = null;
let addingProject = false;

export function mount(pane) {
  styles("anlage");
  root = pane;
  root.innerHTML = `
    <div class="toolbar">
      <span class="newproj" data-newproj></span>
      <button type="button" class="quiet" data-import>${esc(t("import"))}</button>
      <input type="file" accept=".json,application/json" data-import-file hidden>
      <span class="grow"></span>
      <a class="quiet" data-export download>${esc(t("export"))}</a>
      <button type="button" data-report title="${esc(t("report_title"))}">${esc(t("report"))}</button>
    </div>
    <div class="plant" data-plant hidden></div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" aria-hidden="true"><span>${esc(t("an_col_name"))}</span><span>${esc(t("an_col_conn"))}</span><span class="num">${esc(
          t("an_col_meters")
        )}</span><span class="num">${esc(t("an_col_dp"))}</span><span>${esc(t("an_col_op"))}</span><span>${esc(t("an_col_state"))}</span><span></span></div>
        <div class="rows" role="listbox" tabindex="0" aria-label="${esc(t("tab_project"))}" data-list></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>`;
  wire();
  watch(["status", "tree"], onProject);
  watch(["status", "tree"], render);
}

const q = (sel) => root.querySelector(sel);

/// Projekt des Kopfzeilen-Menüs; die Anlage zeigt immer das aktive Projekt
const project = () => state.tree?.projects.find((p) => p.id === state.status?.active?.project?.id) || null;
const allLines = (p) => p.gateways.flatMap((g) => g.lines);
const sum = (lines, f) => lines.reduce((n, l) => n + (f(l) || 0), 0);
const activeLineId = () => state.status?.active?.line?.id;

function onProject() {
  const id = state.status?.active?.project?.id;
  if (!id || id === projectId) return;
  projectId = id;
  if (!perProject.has(id)) perProject.set(id, { sel: `l:${activeLineId()}`, closed: new Set(), adding: null, detailOpen: false });
  view = perProject.get(id);
}

/// Element zum Schlüssel der Auswahl
function picked() {
  const p = project();
  const [kind, id] = view.sel.split(":");
  if (kind === "g") {
    const g = p.gateways.find((x) => x.id === Number(id));
    if (g) return { kind: "gateway", g };
  }
  if (kind === "l") {
    for (const g of p.gateways) {
      const l = g.lines.find((x) => x.id === Number(id));
      if (l) return { kind: "line", g, l };
    }
  }
  return { kind: "project", p };
}

/// Zeilen der Liste: Projekt, je Gateway seine Linien (wenn aufgeklappt), Felder für Neues
function items(p) {
  const out = [{ key: "p", kind: "project", p }];
  p.gateways.forEach((g, i) => {
    out.push({ key: `g:${g.id}`, kind: "gateway", g, i, n: p.gateways.length });
    if (!view.closed.has(g.id)) g.lines.forEach((l, k) => out.push({ key: `l:${l.id}`, kind: "line", g, l, i: k, n: g.lines.length }));
    if (view.adding === g.id) out.push({ key: `new-l:${g.id}`, kind: "new", what: "line" });
  });
  out.push(view.adding === "gateway" ? { key: "new-g", kind: "new", what: "gateway" } : { key: "add-g", kind: "add", empty: !p.gateways.length });
  return out;
}

const selectable = () => items(project()).filter((x) => ["project", "gateway", "line"].includes(x.kind));

// ------------------------------------------------------------ Zeichnen

function render() {
  if (!view || !project()) return;
  const p = project();
  if (picked().kind === "project" && view.sel !== "p") view.sel = "p";
  renderToolbar(p);
  renderPlant(p);
  list(q("[data-list]"), items(p), (x) => x.key, row);
  q("[data-list]").setAttribute("aria-activedescendant", `an-${view.sel.replace(":", "-")}`);
  renderDetail();
}

function renderToolbar(p) {
  html(
    q("[data-newproj]"),
    addingProject
      ? `<input data-new="project" placeholder="${esc(t("an_new_project_ph"))}" aria-label="${esc(t("an_new_project_ph"))}"><button type="button" data-create="project">${esc(
          t("an_create")
        )}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button>`
      : `<button type="button" class="tonal" data-add-project>${esc(t("an_new_project"))}</button>`
  );
  q("[data-export]").href = apiHref(`projects/${p.id}/export`);
  // der Bericht ist der nächste Schritt, sobald etwas ausgelesen ist (Konzept §3.4)
  q("[data-report]").className = allLines(p).some((l) => l.last_read) ? "" : "quiet";
}

function row(x) {
  if (x.kind === "project") return projectRow(x.p);
  if (x.kind === "gateway") return gatewayRow(x);
  if (x.kind === "line") return lineRow(x);
  if (x.kind === "new")
    return `<div class="row new"><input data-new="${x.what}" placeholder="${esc(t(`an_new_${x.what}_ph`))}" aria-label="${esc(
      t(`an_new_${x.what}_ph`)
    )}"><button type="button" data-create="${x.what}">${esc(t("an_create"))}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button></div>`;
  return `<div class="row add">${x.empty ? `<span class="sub">${esc(t("an_no_gateways"))}</span>` : ""}<button type="button" class="${
    x.empty ? "" : "quiet"
  }" data-add-gateway>${esc(t("gw_add"))}</button></div>`;
}

const rowAttrs = (key, cls) => `class="row ${cls}${view.sel === key ? " active" : ""}" role="option" id="an-${key.replace(":", "-")}" aria-selected="${view.sel === key}"`;

function projectRow(p) {
  const lines = allLines(p);
  const issues = sum(lines, (l) => l.issues?.count);
  return `<div ${rowAttrs("p", "pj")}>
    <span class="name">${esc(p.name)}</span>
    <span class="cut sub">${esc([p.customer, p.site].filter(Boolean).join(" · "))}</span>
    <span class="num">${sum(lines, (l) => l.meters)}</span>
    <span class="num">${sum(lines, (l) => l.datapoints?.count)}</span>
    <span class="cut sub">${p.plan && p.plan !== "off" ? esc(planText(p.plan)) : ""}</span>
    <span class="cut">${issues ? `<span class="chip warn">${esc(t("ln_issues", issues))}</span>` : ""}</span>
    <span></span>
  </div>`;
}

function gatewayRow({ g, i, n }) {
  const closed = view.closed.has(g.id);
  const ops = g.lines.filter((l) => l.operation).length;
  const issues = sum(g.lines, (l) => l.issues?.count);
  return `<div ${rowAttrs(`g:${g.id}`, "gw")} aria-expanded="${!closed}">
    <span class="name"><button type="button" class="icon chev" data-toggle="${g.id}" aria-label="${esc(g.name)}" aria-expanded="${!closed}">${closed ? "▸" : "▾"}</button>${esc(g.name)}</span>
    <span class="cut sub">${g.address ? `<span class="mono">${esc(g.address)}</span> · ` : ""}${esc(g.lines.length ? t("ps_lines", g.lines.length) : t("an_no_lines"))}</span>
    <span class="num">${sum(g.lines, (l) => l.meters)}</span>
    <span class="num">${sum(g.lines, (l) => l.datapoints?.count)}</span>
    <span class="cut">${ops ? `<span class="chip ok">${esc(t("n_op_count", ops))}</span>` : ""}</span>
    <span class="cut">${issues ? `<span class="chip warn">${esc(t("ln_issues", issues))}</span>` : ""}</span>
    <span class="acts"><button type="button" class="quiet" data-add-line="${g.id}">${esc(t("ln_add"))}</button>${moveButtons("gateways", g.id, i, n)}</span>
  </div>`;
}

function lineRow({ l, i, n }) {
  const current = l.id === activeLineId();
  return `<div ${rowAttrs(`l:${l.id}`, "ln" + (current ? " current" : ""))}>
    <span class="name">${esc(l.name)}${current ? ` <span class="chip ok">${esc(t("ln_active"))}</span>` : ""}</span>
    <span class="cut mono">${l.connection ? esc(`${portText(l.connection)} · ${l.baud}`) : `<span class="warn">${esc(t("ln_no_conn"))}</span>`}</span>
    <span class="num">${l.meters}</span>
    <span class="num">${l.datapoints?.count || ""}</span>
    <span class="op">${opSwitch(l)}<span class="cut ${l.operation ? "ok" : "sub"}">${esc(opText(l))}</span></span>
    <span class="cut">${lineState(l)}</span>
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

const planned = (l) => !!l.plan_info?.meters;

/// Schalter Dauerbetrieb: ohne Anschluss oder Plan gesperrt, der Grund steht daneben (opText)
const opSwitch = (l, big = false) =>
  `<input type="checkbox" class="switch${big ? " big" : ""}" data-op="${l.id}"${l.operation ? " checked" : ""}${
    opPossible(l) ? "" : " disabled"
  } aria-label="${esc(`${t("op_chip")} ${l.name}`)}">`;


/// Zustand der Linie: Probleme als Wort und Farbe zuerst, sonst letzter bzw. nächster Termin
function lineState(l) {
  const p = l.plan_info || {};
  const r = p.result;
  const chips = [];
  if (l.held) chips.push(["", t("plan_held"), t("plan_held_title")]);
  else if (l.service && !l.service.connected && l.service.error) chips.push(["warn", t("plan_offline"), l.service.error]);
  if (r?.line_down) chips.push(["warn", t("an_line_down"), r.error || t("plan_line_down")]);
  if (p.disturbed) chips.push(["warn", t("plan_disturbed", p.disturbed)]);
  if (l.datapoints?.invalid) chips.push(["warn", t("ln_dp_invalid", l.datapoints.invalid), t("ln_dp_invalid_title")]);
  if (l.issues?.count) chips.push(["warn", t("ln_issues", l.issues.count), l.issues.items.join("\n")]);
  if (p.paused) chips.push(["", t("plan_paused", p.paused)]);
  if (p.load_warn) chips.push(["warn", t("plan_load", Math.round(p.load * 100))]);
  const info =
    r?.time && !r.offline && !r.line_down
      ? t("plan_last", r.ok, r.total)
      : l.operation && p.next && !l.held
        ? t("plan_next", slotText(p.next))
        : l.last_read
          ? t("an_read_at", slotText(l.last_read))
          : "";
  return chips.map(([cls, txt, title]) => `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(txt)}</span>`).join(" ") + (info ? ` <span class="sub">${esc(info)}</span>` : "");
}

/// Zustand der Anlage als Raster; jeder Teil führt zur Stelle, an der man es behebt
function renderPlant(p) {
  const box = q("[data-plant]");
  const s = state.status;
  const lines = allLines(p).filter((l) => l.operation);
  box.hidden = !lines.length || !s;
  if (box.hidden) return;
  const cell = (title, body) => `<div class="cell"><span class="eyebrow">${esc(title)}</span><span class="body">${body}</span></div>`;
  const chip = (cls, txt, title = "") => `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(txt)}</span>`;
  const cells = [];
  if (!s.background) {
    const win = s.os === "windows";
    cells.push(
      cell(
        t("op_chip"),
        `${chip("warn", t("ps_fg"), t(win ? "ps_fg_win" : "ps_fg_linux"))} <a href="${README}#${win ? "windows-als-dienst" : "raspberry-pi-als-dienst"}" target="_blank" rel="noopener">${esc(
          t("an_svc_how")
        )}</a>`
      )
    );
  }
  const planLines = lines.filter(planned);
  const next = planLines.map((l) => l.plan_info.next).filter(Boolean).sort()[0];
  const disturbed = sum(planLines, (l) => l.plan_info.disturbed);
  const paused = sum(planLines, (l) => l.plan_info.paused);
  const down = planLines.filter((l) => l.plan_info.result?.line_down).length;
  cells.push(
    cell(
      t("plan"),
      `${esc(t("ps_lines", planLines.length))}${next ? ` · ${esc(t("plan_next", slotText(next)))}` : ""}` +
        (disturbed ? " " + chip("warn", t("plan_disturbed", disturbed)) : "") +
        (paused ? " " + chip("", t("plan_paused", paused)) : "") +
        (down ? " " + chip("warn", t("ps_lines_down", down)) : "")
    )
  );
  if (s.time)
    cells.push(
      cell(
        t("ps_time"),
        s.time.confirmed
          ? esc(t("ps_time_ok", s.time.source))
          : `${chip("warn", t("time_unsure"))} <button type="button" class="tonal" data-take-time>${esc(t("ps_time_take"))}</button>`
      )
    );
  if (s.disk) {
    const pct = Math.round(s.disk.free_pct);
    cells.push(cell(t("ps_disk"), s.disk.level === "ok" ? esc(t("ps_disk_free", pct)) : chip("warn", t(s.disk.level === "full" ? "disk_full" : "disk_low", pct))));
  }
  for (const o of (s.outputs || []).filter((o) => o.on)) {
    const [cls, word] = outputWord(s[o.key]?.state);
    cells.push(cell(o.label, `${chip(cls, word, s[o.key]?.error || "")} <a href="#/weitergabe">${esc(t("ps_open_transfer"))}</a>`));
  }
  html(box, cells.join(""));
}

// ------------------------------------------------------------ Details

function renderDetail() {
  const box = q("[data-detail]");
  box.classList.toggle("open", view.detailOpen);
  // wer gerade in ein Feld tippt, verliert nichts: neu gezeichnet wird erst danach
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input:not([type=checkbox]), select, textarea");
  if (typing && box.dataset.sel === view.sel) return;
  box.dataset.sel = view.sel;
  const x = picked();
  const back = `<button type="button" class="quiet back" data-back>← ${esc(t("tab_project"))}</button>`;
  if (x.kind === "project") return html(box, back + projectDetail(x.p));
  if (x.kind === "gateway") return html(box, back + gatewayDetail(x.g));
  html(box, back + lineDetail(x.l, x.g));
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

function lineDetail(l, g) {
  const current = l.id === activeLineId();
  const p = project();
  const ports = (state.ports || []).map((x) => `<option value="${esc(x.port)}">${esc(x.label || "")}</option>`).join("");
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

// ------------------------------------------------------------ Aktionen

function select(key, { focusRow = false, detail = false } = {}) {
  view.sel = key;
  if (detail) view.detailOpen = true;
  render();
  const el = root.querySelector(`#an-${key.replace(":", "-")}`);
  el?.scrollIntoView({ block: "nearest" });
  if (focusRow) q("[data-list]").focus();
}

/// Feld beim Verlassen speichern; Escape setzt es auf den gespeicherten Wert zurück
async function saveField(input) {
  const value = input.dataset.field === "baud" ? Number(input.value) : input.value.trim();
  if (String(value) === input.dataset.saved) return;
  const x = picked();
  try {
    if (input.dataset.kind === "project") {
      const p = x.p;
      const body = { name: p.name, customer: p.customer, site: p.site, technician: p.technician, order_no: p.order_no, remarks: p.remarks };
      await request(`projects/${p.id}`, { method: "PUT", body: { ...body, [input.dataset.field]: value } });
    } else if (input.dataset.kind === "gateway") {
      const g = x.g;
      await request(`gateways/${g.id}`, { method: "PUT", body: { name: g.name, address: g.address, note: g.note, [input.dataset.field]: value } });
    } else {
      const l = x.l;
      const body = { name: l.name, connection: l.connection, baud: l.baud, timeout_ms: l.timeout_ms, [input.dataset.field]: value };
      await request(`lines/${l.id}`, { method: "PUT", body });
      // aktive Linie mit neuem Anschluss: gleich damit verbinden
      if (l.id === activeLineId() && input.dataset.field !== "name") await useLine(l.id);
    }
    input.dataset.saved = String(value);
    input.classList.add("saved");
    setTimeout(() => input.classList.remove("saved"), 900);
    await reload();
  } catch (e) {
    report(e);
  }
}

async function savePlan(kind) {
  const sel = q(`[data-plan="${kind}"]`);
  const time = q(`[data-plan-time="${kind}"]`);
  time.hidden = !TIMED.includes(sel.value);
  const hhmm = time24(time.value);
  time.classList.toggle("invalid", !time.hidden && !hhmm);
  if (!time.hidden && !hhmm) return toast(t("an_time_bad"), { error: true });
  if (hhmm) time.value = hhmm;
  const plan = !sel.value ? null : time.hidden ? sel.value : `${sel.value} ${hhmm}`;
  const x = picked();
  try {
    const r = await request(kind === "project" ? `projects/${x.p.id}/plan` : `lines/${x.l.id}/plan`, { method: "PUT", body: { plan } });
    if (r.warnings?.length) toast(t("plan_saved_warn", r.warnings.join(" ")), { error: true });
    await loadTree();
    loadMeters().catch(() => {}); // geerbte Pläne der Zähler ändern sich mit
  } catch (e) {
    report(e);
  }
}

async function saveThin(select_) {
  const days = Number(select_.value) || null;
  try {
    await request(`projects/${project().id}/thin`, { method: "PUT", body: { days } });
    await loadTree();
  } catch (e) {
    report(e);
  }
}

async function use(id) {
  const l = allLines(project()).find((x) => x.id === id);
  try {
    await useLine(id);
    toast(t("activated", l?.name || ""));
  } catch (e) {
    report(e);
  }
}

async function move(button) {
  try {
    await request(`${button.dataset.move}/${button.dataset.id}/move`, { method: "POST", body: { dir: Number(button.dataset.dir) } });
    await loadTree();
  } catch (e) {
    report(e);
  }
}

async function create(what) {
  const input = root.querySelector(`[data-new="${what}"]`);
  const name = input.value.trim();
  if (!name) return input.focus();
  try {
    if (what === "project") {
      await request("projects", { method: "POST", body: { name } });
      addingProject = false;
      await reload(); // das neue Projekt ist aktiv; die Anlage folgt über den Zustand
    } else if (what === "gateway") {
      const g = await request(`projects/${project().id}/gateways`, { method: "POST", body: { name, address: "", note: "" } });
      view.adding = null;
      await loadTree();
      select(`g:${g.id}`);
    } else {
      const gw = view.adding;
      const l = await request(`gateways/${gw}/lines`, { method: "POST", body: { name, connection: "", baud: 2400 } });
      view.adding = null;
      view.closed.delete(gw);
      await loadTree();
      select(`l:${l.id}`, { detail: true });
      q("[data-field=connection]")?.focus(); // nächster Schritt einer neuen Linie
    }
    toast(t("an_created", name));
  } catch (e) {
    report(e);
  }
}

function startAdding(what) {
  if (what === "project") addingProject = true;
  else view.adding = what;
  render();
  root.querySelector(`[data-new="${what === "project" ? "project" : what === "gateway" ? "gateway" : "line"}"]`)?.focus();
}

function cancelAdding() {
  addingProject = false;
  view.adding = null;
  render();
}

async function remove(button) {
  const x = picked();
  const what = button.dataset.delete;
  const name = (x.l || x.g || x.p).name;
  const question = t({ project: "confirm_project_delete", gateway: "confirm_gw_delete", line: "confirm_ln_delete" }[what], name);
  if (!(await ask(button, question, t("delete")))) return;
  const path = { project: `projects/${x.p?.id}`, gateway: `gateways/${x.g?.id}`, line: `lines/${x.l?.id}` }[what];
  try {
    await request(path, { method: "DELETE" });
    view.sel = what === "line" ? `g:${x.g.id}` : "p";
    view.detailOpen = false;
    await reload();
    toast(t("an_deleted", name));
  } catch (e) {
    report(e);
  }
}

/// Projekt für einen neuen Einsatz leeren (Konzept Bedienung §12): Linien und Einstellungen bleiben
async function clearProject(button) {
  const p = project();
  if (!(await ask(button, t("confirm_project_clear", sum(allLines(p), (l) => l.meters), p.name), t("project_clear").replace(/\s*…$/, "")))) return;
  try {
    await request(`projects/${p.id}/clear`, { method: "POST" });
    await reload();
    loadMeters().catch(() => {});
    toast(t("project_cleared", p.name));
  } catch (e) {
    report(e);
  }
}

async function importProject(file) {
  try {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("project_import_bad"));
    }
    const r = await request("projects/import", { method: "POST", body: data });
    await loadTree();
    const p = state.tree.projects.find((x) => x.id === r.project);
    const first = p && allLines(p)[0];
    if (first) await useLine(first.id);
    toast(t("project_imported", p?.name || ""));
  } catch (e) {
    report(e);
  }
}

async function takeTime() {
  try {
    const r = await request("time", { method: "POST", body: { epoch_ms: Date.now() } });
    toast(t("time_taken", Math.round(r.offset_s)));
    await loadStatus();
  } catch (e) {
    report(e);
  }
}

function toggleGateway(id, show = view.closed.has(id)) {
  if (show) view.closed.delete(id);
  else view.closed.add(id);
  // die Auswahl verschwindet nicht mit einer zugeklappten Linie, sie geht an ihr Gateway
  if (!show && picked().g?.id === id) view.sel = `g:${id}`;
  render();
}

// ------------------------------------------------------------ Bedienung

function wire() {
  const listEl = q("[data-list]");
  root.addEventListener("click", (ev) => {
    const el = ev.target;
    const hit = (sel) => el.closest(sel);
    if (hit("[data-add-project]")) return startAdding("project");
    if (hit("[data-add-gateway]")) return startAdding("gateway");
    if (hit("[data-add-line]")) return startAdding(Number(hit("[data-add-line]").dataset.addLine));
    if (hit("[data-create]")) return create(hit("[data-create]").dataset.create);
    if (hit("[data-cancel-new]")) return cancelAdding();
    if (hit("[data-import]")) return q("[data-import-file]").click();
    if (hit("[data-report]")) return window.open(`report.html?project=${project().id}`, "_blank", "noopener");
    if (hit("[data-take-time]")) return takeTime();
    if (hit("[data-use]")) return use(Number(hit("[data-use]").dataset.use));
    if (hit("[data-move]")) return move(hit("[data-move]"));
    if (hit("[data-toggle]")) return toggleGateway(Number(hit("[data-toggle]").dataset.toggle));
    if (hit("[data-delete]")) return remove(hit("[data-delete]"));
    if (hit("[data-clear]")) return clearProject(hit("[data-clear]"));
    if (hit("[data-back]")) {
      view.detailOpen = false;
      render();
      return listEl.focus();
    }
    const r = hit("[role=option]");
    if (r && !hit("input, button, a")) select(r.dataset.key, { detail: true });
  });
  root.addEventListener("change", (ev) => {
    const el = ev.target;
    if (el.matches("[data-op]")) return switchOperation(el, allLines(project()).find((x) => x.id === Number(el.dataset.op)));
    if (el.matches("[data-field]")) return saveField(el);
    if (el.matches("[data-plan]")) return savePlan(el.dataset.plan);
    if (el.matches("[data-plan-time]")) return savePlan(el.dataset.planTime);
    if (el.matches("[data-thin]")) return saveThin(el);
    if (el.matches("[data-import-file]")) {
      const file = el.files[0];
      el.value = "";
      if (file) importProject(file);
    }
  });
  root.addEventListener("keydown", (ev) => {
    const el = ev.target;
    if (el.matches("[data-field]") && ev.key === "Escape") {
      el.value = el.dataset.saved;
      el.blur();
      ev.stopPropagation();
    }
    if (el.matches("input[data-field], input[data-plan-time]") && ev.key === "Enter") el.blur();
    if (el.matches("[data-new]") && ev.key === "Enter") create(el.dataset.new);
    if (el.matches("[data-new]") && ev.key === "Escape") {
      ev.stopPropagation();
      cancelAdding();
    }
  });
  root.addEventListener("focusin", (ev) => {
    if (ev.target.matches("[data-field=connection]") && !state.ports) request("ports").then((ports) => set({ ports })).then(renderPorts, () => {});
  });
  listEl.addEventListener("keydown", onListKey);
  bind("w", () => picked().kind === "line" && use(picked().l.id), t("an_key_use"), open);
}

/// Anschlüsse zur Auswahl erst laden, wenn man das Feld betritt (wie das Formular der Kopfzeile)
function renderPorts() {
  const dl = root.querySelector("#an-ports");
  if (dl) dl.innerHTML = state.ports.map((x) => `<option value="${esc(x.port)}">${esc(x.label || "")}</option>`).join("");
}

/// ↑↓ wählen, ←→ Gateway zu/auf, Enter Hauptaktion (Linie aktiv machen, Gateway auf/zu,
/// Projekt bearbeiten), Alt+↑↓ verschieben
function onListKey(ev) {
  const rows = selectable();
  const i = rows.findIndex((x) => x.key === view.sel);
  const x = picked();
  if ((ev.key === "ArrowDown" || ev.key === "ArrowUp") && ev.altKey) {
    if (x.kind === "project") return;
    ev.preventDefault();
    const b = root.querySelector(`[data-move="${x.kind === "line" ? "lines" : "gateways"}"][data-id="${(x.l || x.g).id}"][data-dir="${ev.key === "ArrowUp" ? -1 : 1}"]`);
    if (b && !b.disabled) move(b);
  } else if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    const next = rows[Math.max(0, Math.min(rows.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))];
    if (next) select(next.key, { focusRow: true });
  } else if ((ev.key === "ArrowLeft" || ev.key === "ArrowRight") && x.kind !== "project") {
    ev.preventDefault();
    toggleGateway(x.g.id, ev.key === "ArrowRight");
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    if (x.kind === "line") use(x.l.id);
    else if (x.kind === "gateway") toggleGateway(x.g.id);
    else q("[data-field=name]")?.focus();
  }
}
