// Kopfzeile: Projekt › Linie, Anschluss mit der passenden Aktion, Dauerbetrieb der Linie,
// „N im Dauerbetrieb“ über alle Projekte. Event-Handler hängen beim Laden des Moduls an.

import { state, set, watch, activeLine, operatingLines } from "./store.js";
import { html, text, menu, closeMenu, opPossible, opText } from "./view.js";
import { reload, useLine, report, switchOperation } from "./data.js";

/// Anzeigen an den Zustand hängen (beim Start)
export function startKopf() {
  watch(["tree", "status"], renderProjects);
  watch(["tree", "status"], renderLine);
  watch(["status", "offline"], renderConnection);
  watch(["tree", "status"], renderOperation);
  watch(["tree", "status"], renderOpAll);
}

function renderProjects() {
  const sel = $("project");
  const active = state.status?.active?.project?.id;
  html(
    sel,
    (state.tree?.projects || []).map((p) => `<option value="${p.id}"${p.id === active ? " selected" : ""}>${esc(p.name)}</option>`).join("")
  );
  if (active) sel.value = String(active);
}

$("project").addEventListener("change", (ev) => {
  const p = state.tree.projects.find((x) => x.id === Number(ev.target.value));
  const line = p?.gateways.flatMap((g) => g.lines)[0];
  if (line) useLine(line.id).catch(report);
});

function renderLine() {
  const a = state.status?.active;
  if (!a) return;
  html($("line"), `<b>${esc(a.line.name)}</b><span class="sub">${esc(a.gateway.name)}</span>`);
  const project = state.tree?.projects.find((p) => p.id === a.project.id);
  html(
    $("line-menu"),
    (project?.gateways || [])
      .map(
        (g) =>
          `<div class="group">${esc(g.name)}</div>` +
          g.lines
            .map(
              (l) =>
                `<button type="button" role="menuitem" data-line="${l.id}"${l.id === a.line.id ? ' aria-current="true"' : ""}>` +
                `<span>${esc(l.name)}${l.operation ? ` <span class="dot on" title="${esc(t("op_chip"))}"></span>` : ""}</span>` +
                `<span class="sub mono">${esc(portText(l.connection) || t("ln_no_conn"))}</span></button>`
            )
            .join("")
      )
      .join("")
  );
}

menu($("line"), $("line-menu"));
$("line-menu").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-line]");
  if (!b) return;
  closeMenu();
  const id = Number(b.dataset.line);
  if (id !== state.status.active.line.id) useLine(id).catch(report);
});

/// Anschluss: Zustand als Wort und Farbe, daneben genau die passende Aktion
function renderConnection() {
  const s = state.status;
  if (!s) return;
  const line = s.active.line;
  const btn = $("conn-action");
  if (state.offline) {
    html($("conn-state"), `<span class="dot err"></span><b>${esc(t("server_offline"))}</b>`);
  } else if (s.connected) {
    $("conn-state").title = "";
    html($("conn-state"), `<span class="dot on"></span><b>${esc(t("lb_connected"))}</b> <span class="mono">${esc(portText(s.connection.port))} · ${s.connection.baud}</span>`);
    text(btn, t("n_change"));
    btn.title = t("conn_change");
    btn.className = "quiet";
  } else if (line.connection) {
    // der Grund (z.B. „USB-Wandler abgezogen?“) steht ganz in der Statusleiste, hier nur der Zustand
    html($("conn-state"), `<span class="dot off"></span><b>${esc(t("lb_disconnected"))}</b> <span class="mono">${esc(portText(line.connection))}</span>`);
    $("conn-state").title = s.line?.error || "";
    text(btn, t("connect"));
    btn.title = "";
    btn.className = "";
  } else {
    html($("conn-state"), `<span class="dot warn"></span><b class="warn">${esc(t("ln_no_conn"))}</b>`);
    text(btn, t("ln_set_conn"));
    btn.title = "";
    btn.className = "";
  }
}

/// Verbinden mit dem gespeicherten Anschluss geht direkt; sonst das kleine Formular
$("conn-action").addEventListener("click", async (ev) => {
  const s = state.status;
  if (!s.connected && s.active.line.connection) {
    ev.stopImmediatePropagation(); // nicht auch noch das Formular öffnen (menu() hängt danach)
    try {
      await request("connect", { method: "POST", body: { port: s.active.line.connection, baud: s.active.line.baud } });
    } catch (e) {
      report(e);
    }
    return reload();
  }
});
menu($("conn-action"), $("conn-form"), { onOpen: renderConnForm });

async function renderConnForm() {
  const s = state.status;
  const current = s.connection?.port || s.active.line.connection || "";
  const baud = s.connection?.baud || s.active.line.baud || 2400;
  const box = $("conn-form");
  // die Suche nach COM-Ports kann unter Windows Sekunden dauern: zeigen, dass gesucht wird
  if (!state.ports) box.innerHTML = `<p class="sub"><span class="dot busy"></span> ${esc(t("n_ports_searching"))}</p>`;
  const ports = state.ports || (await request("ports").catch(() => []));
  set({ ports });
  const known = ports.some((p) => p.port === current);
  box.innerHTML = `
    <label class="row"><span>${esc(t("port"))}</span><select name="port">${ports
      .map((p) => `<option value="${esc(p.port)}"${p.port === current ? " selected" : ""}>${esc(p.label ? `${p.dev || portText(p.port)} · ${p.label}` : p.port)}</option>`)
      .join("")}</select></label>
    <label class="row"><span>${esc(t("port_manual"))}</span><input name="manual" class="mono" value="${known ? "" : esc(current)}" placeholder="COM3, /dev/ttyUSB0, tcp://…"></label>
    <label class="row"><span>${esc(t("baud"))}</span><select name="baud">${BAUDS
      .map((b) => `<option${b === baud ? " selected" : ""}>${b}</option>`)
      .join("")}</select></label>
    <div class="row end">${s.connected ? `<button type="button" class="quiet" data-disconnect>${esc(t("disconnect"))}</button>` : ""}<button type="submit" data-connect>${esc(t("connect"))}</button></div>`;
  box.querySelector("[name=manual]").addEventListener("keydown", (ev) => ev.key === "Enter" && box.querySelector("[data-connect]").click());
}

$("conn-form").addEventListener("click", async (ev) => {
  const box = $("conn-form");
  const go = ev.target.closest("[data-connect], [data-disconnect]");
  if (!go) return;
  try {
    if (go.dataset.disconnect !== undefined) await request("disconnect", { method: "POST" });
    else {
      const port = box.querySelector("[name=manual]").value.trim() || box.querySelector("[name=port]").value;
      await request("connect", { method: "POST", body: { port, baud: Number(box.querySelector("[name=baud]").value) } });
    }
    closeMenu(true);
  } catch (e) {
    report(e);
  }
  reload();
});

/// Dauerbetrieb der Linie: Schalter zeigt Zustand und schaltet; ohne Plan gesperrt mit Grund daneben
function renderOperation() {
  const l = activeLine();
  if (!l) return;
  const p = l.plan_info;
  const sw = $("op-switch");
  sw.checked = !!l.operation;
  sw.disabled = !opPossible(l);
  const fg = l.operation && state.status && !state.status.background;
  $("op").classList.toggle("on", !!l.operation);
  // nächster Termin und „nicht als Dienst“ als Detail; sichtbar steht das an „N im Dauerbetrieb“
  $("op").title = [l.operation && p.next ? t("plan_next", slotText(p.next)) : "", fg ? t("ps_fg") : ""].filter(Boolean).join(" · ");
  text($("op-text"), opText(l));
}

$("op-switch").addEventListener("change", (ev) => switchOperation(ev.target, activeLine()));

/// Kopfzeile rechts: was dieses Gerät im Hintergrund tut, über alle Projekte
function renderOpAll() {
  const ops = operatingLines();
  const b = $("op-all");
  b.hidden = !ops.length;
  if (!ops.length) return;
  const warn = !state.status?.background || ops.some(({ l }) => l.plan_info?.result?.line_down || l.plan_info?.result?.offline);
  html(b, `<span class="dot ${warn ? "warn" : "on"}"></span>${esc(t("n_op_count", ops.length))}`);
  html(
    $("op-all-menu"),
    (state.status?.background ? "" : `<div class="group warn">${esc(t("ps_fg"))}</div>`) +
      ops
        .map(({ p, g, l }) => {
          const r = l.plan_info?.result;
          const bad = r?.line_down || r?.offline;
          return `<button type="button" role="menuitem" data-line="${l.id}">` +
            `<span>${esc(`${p.name} › ${g.name} › ${l.name}`)}</span>` +
            `<span class="sub">${bad ? `<span class="chip warn">${esc(t(r.offline ? "plan_offline" : "plan_line_down"))}</span>` : esc(l.plan_info?.next ? t("plan_next", slotText(l.plan_info.next)) : "")}</span></button>`;
        })
        .join("")
  );
}

menu($("op-all"), $("op-all-menu"));
$("op-all-menu").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-line]");
  if (!b) return;
  closeMenu();
  useLine(Number(b.dataset.line)).catch(report);
});
