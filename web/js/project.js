"use strict";

// Projekte, Gateways, Linien und Ausleseplan

// ------------------------------------------------------------ Ausleseplan

const PLAN_KINDS = ["off", "monthly", "daily", "hourly", "15min"];
const ICON_CLOCK = svg('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>');

/// "daily 00:05" → "täglich 00:05"
function planText(key) {
  if (!key) return "";
  const [kind, time] = key.split(" ");
  return kind === "daily" || kind === "monthly" ? t("plan_" + kind, time) : t("plan_" + kind);
}

/// "2026-10-18 00:05:00" → "heute 00:05", "morgen 00:05" oder "20.10. 00:05"
function slotText(s) {
  if (!s) return "";
  const d = parseTime(s);
  const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const days = Math.round((day(d) - day(new Date())) / 86400000);
  const hm = s.slice(11, 16);
  if (days === 0) return `${t("day_today")} ${hm}`;
  if (days === 1) return `${t("day_tomorrow")} ${hm}`;
  return `${d.getDate()}.${d.getMonth() + 1}. ${hm}`;
}

/// Auswahl des Plans für Formulare; `inherit` = Text der Option "wie Projekt/Linie" (sonst keine)
function planField(value, inherit, hint) {
  const [kind, time] = value ? value.split(" ") : [inherit ? "" : "off", null];
  const opts = (inherit ? [["", inherit]] : []).concat(PLAN_KINDS.map((k) => [k, t("plan_opt_" + k)]));
  return `<div class="row plan-row">
      <label><span>${esc(t("plan"))}</span><select name="plan_kind">${opts
        .map(([v, l]) => `<option value="${v}" ${v === kind ? "selected" : ""}>${esc(l)}</option>`)
        .join("")}</select></label>
      <label class="plan-time"><span>${esc(t("plan_time"))}</span><input name="plan_time" class="mono time24" data-time24 value="${esc(time || "00:05")}" inputmode="numeric" maxlength="5" placeholder="HH:MM" autocomplete="off"></label>
    </div>${hint ? `<p class="hint plan-hint">${esc(hint)}</p>` : ""}`;
}

/// Uhrzeit nur bei täglich und monatlich
function wirePlanField(root) {
  const sel = root.querySelector("[name=plan_kind]");
  const wrap = root.querySelector(".plan-time");
  const update = () => (wrap.hidden = !["daily", "monthly"].includes(sel.value));
  sel.addEventListener("change", update);
  update();
}

/// Plan aus den Formularwerten; null = erben
function planFrom(values) {
  const k = values.plan_kind;
  if (!k) return null;
  if (k !== "daily" && k !== "monthly") return k;
  const time = time24(values.plan_time || "00:05");
  if (!time) throw new Error(t("plan_time_bad"));
  return `${k} ${time}`;
}

async function savePlan(path, plan) {
  const r = await api(path + "/plan", { method: "PUT", body: { plan } });
  if (r.warnings?.length) toast(t("plan_saved_warn", r.warnings.join(" ")), true);
  // geerbte Pläne der Zähler ändern sich mit
  await loadMeters().catch(() => {});
}

/// Zeile unter der Linie: Plan, nächster Termin, letztes Ergebnis, Störungen
function linePlanHtml(l) {
  const p = l.plan_info;
  if (!p || !p.meters) return "";
  const parts = [`<b>${esc(p.effective !== "off" ? planText(p.effective) : t("plan_meters_own", p.meters))}</b>`];
  const svc = l.service;
  if (l.held) parts.push(ui.chip("muted", t("plan_held"), t("plan_held_title")));
  else if (svc && !svc.connected && svc.error) parts.push(ui.chip("warn", t("plan_offline"), svc.error));
  if (p.next && !l.held) parts.push(esc(t("plan_next", slotText(p.next))));
  const r = p.result;
  if (r && r.time && !r.offline) {
    if (r.line_down) parts.push(ui.chip("warn", t("plan_line_down"), r.error || ""));
    else parts.push(ui.chip(r.ok === r.total ? "ok" : "warn", t("plan_last", r.ok, r.total), `${r.time}${r.error ? "\n" + r.error : ""}`));
  }
  if (p.disturbed) parts.push(ui.chip("warn", t("plan_disturbed", p.disturbed)));
  if (p.paused) parts.push(ui.chip("muted", t("plan_paused", p.paused), t("meter_paused_title")));
  if (p.load_warn) parts.push(ui.chip("warn", t("plan_load", Math.round(p.load * 100)), t("plan_load_title")));
  return `<div class="ln-plan">${ICON_CLOCK}${parts.join(" · ")}</div>`;
}

let tree = null;
/// zusätzlich aufgeklappte Gateways (das der aktiven Linie ist immer offen)
const openGateways = new Set();

async function loadTree() {
  tree = await api("projects");
  renderTree();
}

function activeProjectNode() {
  return tree.projects.find((p) => p.id === tree.active.project.id);
}

function renderTree() {
  if (!tree) return;
  renderRememberMode();
  const a = tree.active;
  $("h-project").textContent = a.project.name;
  $("project-sub").textContent = [a.project.customer, a.project.site, a.project.plan && a.project.plan !== "off" ? `${t("plan")}: ${planText(a.project.plan)}` : ""]
    .filter(Boolean)
    .join(" · ");
  $("crumb-text").textContent = `${a.project.name} › ${a.gateway.name} › ${a.line.name}`;
  $("conn-line").textContent = t("conn_line", `${a.gateway.name} › ${a.line.name}`);
  $("project-select").innerHTML = tree.projects
    .map((p) => `<option value="${p.id}" ${p.id === a.project.id ? "selected" : ""}>${esc(p.name)}</option>`)
    .join("");
  $("project-export").href = `api/projects/${a.project.id}/export` + (token ? `?token=${encodeURIComponent(token)}` : "");
  const node = activeProjectNode();
  // Pfeile zum Verschieben; ganz oben/unten ausgegraut
  const moveBtns = (kind, id, k, n) =>
    `<button class="icon-btn" data-${kind}-move="-1" data-id="${id}" ${k === 0 ? "disabled" : ""} title="${esc(t("move_up"))}" aria-label="${esc(t("move_up"))}">${ICON_UP}</button>` +
    `<button class="icon-btn" data-${kind}-move="1" data-id="${id}" ${k === n - 1 ? "disabled" : ""} title="${esc(t("move_down"))}" aria-label="${esc(t("move_down"))}">${ICON_DOWN}</button>`;
  $("tree").innerHTML =
    node.gateways
      .map(
        (g, gk) => {
          // eingeklappt, ausser beim Gateway der aktiven Linie oder wenn aufgeklappt
          const open = g.id === a.gateway.id || openGateways.has(g.id);
          const issues = g.lines.reduce((n, l) => n + (l.issues?.count || 0), 0);
          return `<div class="gw${open ? "" : " collapsed"}">
      <div class="gw-head">
        <h3 data-gw-toggle="${g.id}" role="button" tabindex="0" aria-expanded="${open}"><svg class="chev" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>${esc(g.name)}</h3>${g.address ? `<span class="hint mono">${esc(g.address)}</span>` : ""}
        ${open ? "" : `<span class="gw-sum">${esc(t("gw_sum", g.lines.length, issues))}</span>`}
        <span class="spacer"></span>
        ${node.gateways.length > 1 ? moveBtns("gw", g.id, gk, node.gateways.length) : ""}
        <button class="icon-btn" data-gw-edit="${g.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
        <button class="icon-btn" data-gw-del="${g.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
      </div>
      ${g.lines
        .map((l, lk) => {
          const active = l.id === a.line.id;
          return `<div class="ln${active ? " active" : ""}">
          <span>${active ? `<span class="badge-active">${esc(t("ln_active"))}</span>` : `<button class="small ghost" data-ln-use="${l.id}">${esc(t("ln_use"))}</button>`}</span>
          <span class="ln-name">${esc(l.name)}</span>
          <span class="ln-conn">${l.connection ? `${esc(l.connection)} · ${l.baud} Bd` : esc(t("ln_no_conn"))}</span>
          <span class="ln-info">${esc(t("ln_info", l.meters, l.last_read))}${
            l.datapoints?.count ? ` · ${esc(t("ln_dp", l.datapoints.count))}` : ""
          }${
            l.datapoints?.invalid ? " " + ui.chip("warn", t("ln_dp_invalid", l.datapoints.invalid), t("q_decreased_title")) : ""
          }${
            l.issues
              ? l.issues.count
                ? " " + ui.chip("warn", "⚠ " + t("ln_issues", l.issues.count), l.issues.items.join("\n"))
                : " " + ui.chip("ok", "✓ " + t("ln_no_issues"))
              : ""
          }</span>
          <span class="actions">
            ${g.lines.length > 1 ? moveBtns("ln", l.id, lk, g.lines.length) : ""}
            <button class="icon-btn" data-ln-edit="${l.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
            <button class="icon-btn" data-ln-del="${l.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
          </span>
          ${linePlanHtml(l)}
        </div>`;
        })
        .join("")}
      <div class="gw-foot"><button class="small quiet" data-ln-add="${g.id}">${esc(t("ln_add"))}</button></div>
    </div>`;
        }
      )
      .join("") + `<div><button class="small ghost" data-gw-add>${esc(t("gw_add"))}</button></div>`;
}

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

const findGateway = (id) => activeProjectNode().gateways.find((g) => g.id === id);
const findLine = (id) => activeProjectNode().gateways.flatMap((g) => g.lines).find((l) => l.id === id);

/// Formular für Projekt, Gateway oder Linie unter dem Baum
/// Felder der Formulare für Projekt, Gateway und Linie (Baustein Formular)
function treeFields(kind, data) {
  if (kind === "project")
    return [
      { name: "name", label: t("f_name"), required: true },
      { name: "customer", label: t("f_customer") },
      { name: "site", label: t("f_site") },
      { name: "technician", label: t("f_technician") },
      { name: "order_no", label: t("f_order") },
      { name: "remarks", label: t("f_remarks"), wide: true },
      { type: "html", html: planField(data.plan || "off", null, t("plan_hint_project")) },
      {
        name: "thin",
        label: t("thin"),
        type: "select",
        num: true,
        options: [0, 30, 90, 365].map((d) => [d, d ? t("thin_days", d) : t("thin_off")]),
        hint: t("thin_hint"),
      },
    ];
  if (kind === "gateway")
    return [
      { name: "name", label: t("f_name"), required: true },
      { name: "address", label: t("f_address"), cls: "mono" },
      { name: "note", label: t("f_note"), wide: true },
    ];
  return [
    { name: "name", label: t("f_name"), required: true },
    { name: "connection", label: t("f_connection"), cls: "mono", placeholder: t("f_connection_ph"), list: "port-list", wide: true },
    { name: "baud", label: t("baud"), type: "select", num: true, options: BAUDS.map((b) => [b, b]) },
    { type: "html", html: planField(data.plan ?? null, t("plan_inherit_project", planText(tree.active.project.plan || "off")), t("plan_hint_line")) },
  ];
}

/// Formular für Projekt, Gateway oder Linie unter dem Baum
function openTreeForm(kind, data, onSave) {
  const f = $("tree-form");
  const fields = treeFields(kind, data);
  f.innerHTML = `<h3>${esc(t("form_" + kind))}</h3>${ui.form(fields, { ...data, baud: data.baud || 2400, thin: data.thin_days || 0 })}
    <datalist id="port-list">${[...$("port").options].map((o) => `<option value="${esc(o.value)}">`).join("")}</datalist>
    <div class="row"><button type="submit">${esc(t("save"))}</button><button type="button" class="quiet" data-form-cancel>${esc(t("cancel"))}</button></div>`;
  f.hidden = false;
  if (f.querySelector("[name=plan_kind]")) wirePlanField(f);
  f.onsubmit = (ev) => {
    ev.preventDefault();
    guarded(f.querySelector("[type=submit]"), async () => {
      const values = { ...ui.formValues(f, fields), ...planInputs(f) };
      await onSave({ ...data, ...values });
      f.hidden = true;
    });
  };
  f.querySelector("[data-form-cancel]").onclick = () => (f.hidden = true);
  f.querySelector("input").focus();
  f.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

/// Eingaben des Planfelds (Art und Uhrzeit), für planFrom
const planInputs = (root) => ({
  plan_kind: root.querySelector("[name=plan_kind]")?.value,
  plan_time: root.querySelector("[name=plan_time]")?.value,
});

$("tree").addEventListener("click", (ev) => {
  const toggle = ev.target.closest("[data-gw-toggle]");
  if (toggle) {
    const id = Number(toggle.dataset.gwToggle);
    if (id === tree.active.gateway.id) return;
    if (openGateways.has(id)) openGateways.delete(id);
    else openGateways.add(id);
    renderTree();
    return;
  }
  const b = ev.target.closest("button");
  if (!b) return;
  const d = b.dataset;
  if (d.gwMove || d.lnMove) {
    return guarded(b, async () => {
      await api(`${d.gwMove ? "gateways" : "lines"}/${d.id}/move`, { method: "POST", body: { dir: Number(d.gwMove || d.lnMove) } });
      await loadTree();
    });
  }
  const id = Number(d.lnUse || d.lnEdit || d.lnDel || d.lnAdd || d.gwEdit || d.gwDel || 0);
  if (d.lnUse) return guarded(b, () => activateLine(id));
  if (d.lnEdit) {
    const l = findLine(id);
    return openTreeForm("line", l, async (v) => {
      const plan = planFrom(v);
      await api(`lines/${id}`, { method: "PUT", body: { name: v.name, connection: v.connection, baud: v.baud, timeout_ms: l.timeout_ms } });
      if (plan !== (l.plan ?? null)) await savePlan(`lines/${id}`, plan);
      // aktive Linie mit neuem Anschluss: gleich neu verbinden
      if (id === tree.active.line.id && (v.connection !== l.connection || v.baud !== l.baud)) return activateLine(id, v.name);
      await loadTree();
    });
  }
  if (d.lnAdd) {
    const g = findGateway(id);
    const prefill = g.address ? `tcp://${g.address}:` : "";
    return openTreeForm("line", { name: t("ln_new", g.lines.length + 1), connection: prefill, baud: 2400 }, async (v) => {
      const plan = planFrom(v);
      const created = await api(`gateways/${id}/lines`, { method: "POST", body: v });
      if (plan !== null) await savePlan(`lines/${created.id}`, plan);
      await loadTree();
    });
  }
  if (d.gwEdit) {
    return openTreeForm("gateway", findGateway(id), async (v) => {
      await api(`gateways/${id}`, { method: "PUT", body: v });
      await loadTree();
    });
  }
  if (d.gwAdd !== undefined) {
    const n = activeProjectNode().gateways.length + 1;
    return openTreeForm("gateway", { name: t("gw_new", n), address: "", note: "" }, async (v) => {
      await api(`projects/${tree.active.project.id}/gateways`, { method: "POST", body: v });
      await loadTree();
    });
  }
  if (d.lnDel || d.gwDel) {
    const name = d.lnDel ? findLine(id).name : findGateway(id).name;
    if (!confirm(t(d.lnDel ? "confirm_ln_delete" : "confirm_gw_delete", name))) return;
    return guarded(b, async () => {
      const before = tree.active.line.id;
      await api(`${d.lnDel ? "lines" : "gateways"}/${id}`, { method: "DELETE" });
      const after = (await api("projects")).active.line.id;
      if (after !== before) reloadWith(t("activated", ""));
      else await loadTree();
    });
  }
});

$("project-select").onchange = () =>
  guarded($("project-select"), async () => {
    const p = tree.projects.find((x) => x.id === Number($("project-select").value));
    const first = p?.gateways.flatMap((g) => g.lines)[0];
    if (first) await activateLine(first.id);
  });

$("project-new").onclick = () =>
  guarded($("project-new"), async () => {
    const name = prompt(t("project_new_prompt"));
    if (!name || !name.trim()) return;
    await api("projects", { method: "POST", body: { name: name.trim() } });
    reloadWith(t("activated", name.trim()));
  });

$("project-edit").onclick = () =>
  openTreeForm("project", tree.active.project, async (v) => {
    const plan = planFrom(v);
    await api(`projects/${tree.active.project.id}`, { method: "PUT", body: v });
    if (plan !== (tree.active.project.plan || "off")) await savePlan(`projects/${tree.active.project.id}`, plan);
    const thin = Number(v.thin) || null;
    if (thin !== (tree.active.project.thin_days ?? null))
      await api(`projects/${tree.active.project.id}/thin`, { method: "PUT", body: { days: thin } });
    await loadMeters().catch(() => {});
    await loadTree();
  });

$("project-delete").onclick = () => {
  const p = tree.active.project;
  if (!confirm(t("confirm_project_delete", p.name))) return;
  guarded($("project-delete"), async () => {
    await api(`projects/${p.id}`, { method: "DELETE" });
    reloadWith(t("activated", ""));
  });
};

$("project-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("project-new"), async () => {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("import_bad"));
    }
    const r = await api("projects/import", { method: "POST", body: data });
    tree = await api("projects");
    const p = tree.projects.find((x) => x.id === r.project);
    const first = p?.gateways.flatMap((g) => g.lines)[0];
    toast(t("project_imported", p?.name || ""));
    if (first) await activateLine(first.id);
  });
};

// Meldung nach dem Neuladen (Linie gewechselt, Projekt angelegt …)
try {
  const flash = JSON.parse(sessionStorage.getItem("flash") || "null");
  sessionStorage.removeItem("flash");
  if (flash?.msg) setTimeout(() => toast(flash.msg, flash.isError), 300);
} catch {
  /* privater Modus */
}
