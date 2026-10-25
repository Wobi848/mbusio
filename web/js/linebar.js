"use strict";

// Linien-Leiste: welche Linie, Verbindung mit einem Knopf für den nächsten Schritt, Dauerbetrieb,
// Plan der Linie, Linie wechseln

/// was der Knopf der Linien-Leiste gerade tut (nächster Schritt für die Verbindung)
let lineAction = null;

/// Linien-Leiste: welche Linie, Verbindung mit genau einem Knopf für den nächsten Schritt, Plan
function renderLineBar() {
  if (!tree) return;
  const a = tree.active;
  const line = findLine(a.line.id) || a.line;
  $("lb-line").textContent = a.line.name;
  $("lb-gateway").textContent = a.gateway.name;
  $("scan-for").textContent = t("scan_for", a.line.name);
  $("lb-lines").innerHTML =
    activeProjectNode()
      .gateways.map(
        (g) =>
          `<span class="menu-group">${esc(g.name)}</span>` +
          g.lines
            .map(
              (l) =>
                `<button type="button" data-lb-use="${l.id}" class="small quiet${l.id === a.line.id ? " on" : ""}">${esc(l.name)}<span class="mono">${esc(
                  portText(l.connection) || t("ln_no_conn")
                )}</span></button>`
            )
            .join("")
      )
      .join("") + `<button type="button" class="small quiet" data-lb-manage>${esc(t("lb_manage"))}</button>`;
  const dot = (cls) => `<span class="lb-dot ${cls}" aria-hidden="true"></span>`;
  const mono = (x) => `<span class="mono">${esc(x)}</span>`;
  let state, action;
  if (jobRunning) {
    state = `${dot("busy")}<b>${esc(t("lb_busy"))}</b>`;
    action = { text: t("cancel"), cls: "small quiet", go: cancelJob };
  } else if (connected) {
    state = `${dot("on")}<b>${esc(t("lb_connected"))}</b>${mono(`${portText(conn.port)} · ${conn.baud} Bd`)}`;
    action = { text: t("conn_change"), cls: "small quiet", go: openConnForm };
  } else if (a.line.connection) {
    state = `${dot("off")}<b>${esc(t("lb_disconnected"))}</b>${mono(`${portText(a.line.connection)} · ${a.line.baud} Bd`)}`;
    action = { text: t("connect"), cls: "small", go: () => $("connect").click() };
  } else {
    state = `${dot("warn")}<b class="warn">${esc(t("ln_no_conn"))}</b>`;
    action = { text: t("ln_set_conn"), cls: "small", go: openConnForm };
  }
  $("lb-state").innerHTML = state;
  const b = $("lb-action");
  b.textContent = action.text;
  b.className = action.cls + (b.classList.contains("go") ? " go" : "");
  b.hidden = connEditing;
  lineAction = action.go;
  // getrennt mit Anschluss: daneben „Anschluss ändern“
  $("lb-change").hidden = connEditing || connected || jobRunning || !a.line.connection;
  $("sec-conn").hidden = !connEditing;
  const p = line.plan_info;
  renderOpBlock(line);
  renderOpAll();
}

/// Block „Dauerbetrieb“ in der Linien-Leiste: grosser Schalter, Zustand, Plan darunter
/// (Konzept Bedienung §13). Ein Schalter für Zustand und Bedienung, nie zwei Knöpfe.
function renderOpBlock(line) {
  const p = line.plan_info;
  const on = !!line.operation;
  const planned = !!p?.meters;
  const fg = on && !host.background;
  const box = $("lb-op");
  box.className = "op-block" + (on ? " on" : "") + (fg ? " fg" : "");
  const plan = planned ? (p.effective !== "off" ? planText(p.effective) : t("plan_meters_own", p.meters)) : "";
  const sub = on
    ? `${esc(plan)}${p.next ? ` · ${esc(t("plan_next", slotText(p.next)))}` : ""}`
    : planned
      ? esc(t("op_plan_waiting", plan))
      : `<button type="button" class="link" data-op-plan>${esc(t("guide_plan_btn"))}</button>`;
  const state = on
    ? `<span class="op-state"><span class="lb-dot on" aria-hidden="true"></span>${esc(t("op_running"))}</span>`
    : `<span class="op-state">${esc(t("op_off_state"))}</span>`;
  box.innerHTML = `
    <label class="op-head"><input type="checkbox" class="switch big" data-op-toggle${on ? " checked" : ""}><b>${esc(t("op_chip"))}</b></label>
    ${state}${fg ? " " + ui.chip("warn", t("op_fg_short"), `${t("ps_fg")}. ${t(host.os === "windows" ? "ps_fg_win" : "ps_fg_linux")}`) : ""}
    <span class="op-sub">${sub}${planned ? ui.iconBtn(ICON_EDIT, t("op_plan_edit"), { "op-plan": "" }) : ""}</span>`;
}

/// Plan der Linie im Projektbaum bearbeiten (Formular der Linie, beim Plan)
function openLinePlan(id) {
  showTab("project");
  $("tree").querySelector(`[data-ln-edit="${id}"]`)?.click();
  const sel = $("tree-form").querySelector("[name=plan_kind]");
  if (sel) {
    reveal(sel, "center");
    sel.focus({ preventScroll: true });
  }
}

$("lb-op").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-op-plan]")) openLinePlan(tree.active.line.id);
});
$("tree").addEventListener("change", (ev) => {
  const sw = ev.target.closest("[data-ln-op]");
  if (sw) toggleOperation(Number(sw.dataset.lnOp), sw);
});
$("lb-op").addEventListener("change", (ev) => {
  const sw = ev.target.closest("[data-op-toggle]");
  if (sw) toggleOperation(tree.active.line.id, sw);
});

/// Schalter Dauerbetrieb (Linien-Leiste oder Projektbaum). Fehlt Anschluss oder Plan, führt er
/// dorthin statt zu melden; Ausschalten fragt nach. Der Schalter zeigt danach den echten Zustand.
async function toggleOperation(id, sw) {
  const l = tree.projects.flatMap((p) => p.gateways.flatMap((g) => g.lines)).find((x) => x.id === id);
  const on = sw.checked;
  sw.checked = !on;
  if (on && !l.connection) return id === tree.active.line.id ? openConnForm() : toast(t("op_need_conn"), true);
  if (on && !l.plan_info?.meters) return openLinePlan(id);
  if (!on && !confirm(t("op_confirm_off", l.name))) return;
  sw.disabled = true;
  try {
    await api(`lines/${id}/operation`, { method: "PUT", body: { on } });
    await loadTree();
    toast(t(on ? "op_switched_on" : "op_switched_off", l.name));
  } catch (e) {
    toast(e.message, true);
  } finally {
    sw.disabled = false;
  }
}


/// Verbindungsformular unter der Linien-Leiste öffnen bzw. schliessen
function openConnForm(open = true) {
  connEditing = open;
  renderLineBar();
  if (open) setTimeout(() => $("port").focus(), 50);
}

$("lb-action").onclick = () => lineAction?.();
$("lb-change").onclick = () => openConnForm();
$("conn-close").onclick = () => openConnForm(false);
$("lb-lines").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.lbManage !== undefined) return showTab("project");
  const id = Number(b.dataset.lbUse);
  if (id && id !== tree.active.line.id) guarded(b, () => activateLine(id));
});

/// Seite neu laden mit einer Meldung danach (Wechsel von Projekt/Linie betrifft fast alles)
function reloadWith(msg, isError = false) {
  try {
    sessionStorage.setItem("flash", JSON.stringify({ msg, isError }));
  } catch {
    /* ohne Meldung */
  }
  location.reload();
}

async function activateLine(id, name = null) {
  const r = await api(`lines/${id}/activate`, { method: "POST" });
  const line = tree.projects.flatMap((p) => p.gateways).flatMap((g) => g.lines).find((l) => l.id === id);
  name ??= line ? line.name : "";
  reloadWith(r.error ? t("activate_failed", name, r.error) : t("activated", name), !!r.error);
}
