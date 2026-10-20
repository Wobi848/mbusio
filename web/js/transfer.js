"use strict";

// Reiter Weitergabe: wohin (Ausgänge), was (Datenpunkte je Zähler), Vorlagen und CSV.
// Die einzige Stelle, an der festgelegt wird, was weitergegeben wird (Konzept Bedienung 3.2).

const SEND_MODES = ["every", "change", "daily"];

/// Zustand des Reiters
const transfer = {
  data: null, // { meters, templates, outputs }
  scope: storageGet("transfer_scope") || "line",
  picking: new Map(), // Zähler-ID → Werte zur Auswahl (offen)
  focus: null, // Zähler, dessen Werte nach dem Laden aufgehen sollen
  selected: new Set(), // markierte Datenpunkte
  outputOpen: null, // Ausgang mit offenen Einstellungen
};
let mqttConfig = null;
let mqttHasPassword = false;

const mqttBase = () => (mqttConfig?.base || "mbusio").replace(/\/+$/, "");
const allDatapoints = () => (transfer.data ? transfer.data.meters.flatMap((m) => m.datapoints) : []);

async function loadTransfer() {
  const [d, mq] = await Promise.all([api(`datapoints?scope=${transfer.scope}`), api("mqtt")]);
  transfer.data = d;
  mqttConfig = mq.config;
  mqttHasPassword = mq.has_password;
  mqttStatus = mq.status;
  const ids = new Set(allDatapoints().map((x) => x.id));
  for (const id of [...transfer.selected]) if (!ids.has(id)) transfer.selected.delete(id);
  renderTransfer();
  if (transfer.focus) {
    const id = transfer.focus;
    transfer.focus = null;
    await togglePick(id, true);
    document.querySelector(`[data-tr-meter="${id}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }
}

/// Aus Zählerliste oder Auslesung: Reiter öffnen und die Werte dieses Zählers zeigen
/// (Zähler der aktiven Linie: Ansicht „aktive Linie“ genügt)
function openTransfer(meterId) {
  transfer.focus = meterId;
  showTab("transfer");
}

function renderTransfer() {
  if (!transfer.data) return;
  renderOutputs();
  renderDatapointTable();
  renderTemplates();
  if (tree) {
    $("tr-export").href = apiHref(`projects/${tree.active.project.id}/datapoints/csv`);
  }
  $("tr-scope").value = transfer.scope;
  renderGuide();
}

// ------------------------------------------------------------ Ausgänge

/// Zustand eines Ausgangs als Chip und Satz (heute nur MQTT)
function outputState(key) {
  const s = mqttStatus;
  if (key !== "mqtt" || !s) return { chip: ui.chip("muted", t("mqtt_off")), text: "" };
  const kind = { connected: "ok", error: "warn", connecting: "muted" }[s.state] || "muted";
  const text =
    s.state === "connected"
      ? t("mqtt_state_connected", s.broker, s.since, s.sent, s.queued)
      : s.state === "connecting"
        ? t("mqtt_state_connecting", s.broker)
        : s.state === "error"
          ? t("mqtt_state_error", s.broker, s.error, s.queued)
          : t("mqtt_state_off");
  return { chip: ui.chip(kind, t("mqtt_" + (s.state || "off"))), text };
}

/// Nur den Zustand der Ausgänge nachführen (alle paar Sekunden): ein offenes Formular bleibt,
/// wie es ist, sonst gingen Eingaben verloren
function renderOutputState() {
  document.querySelectorAll("[data-out-state]").forEach((el) => {
    const st = outputState(el.dataset.outState);
    el.innerHTML = `${st.chip}<span class="hint">${esc(st.text)}</span>`;
  });
}

/// Unter einem Ausgang: Adressen nach Muster setzen (Vorschau), Liste für den Integrator (CSV, JSON)
function outputTools(o) {
  if (!tree || !allDatapoints().some((d) => d.outputs[o.key]?.address)) return "";
  const list = (format) => apiHref(`projects/${tree.active.project.id}/outputs/${o.key}/list?format=${format}`);
  return `<div class="row tight out-tools">
      <button type="button" class="small tonal" data-out-readdress="${o.key}">${esc(t("out_readdress", o.address_label))}</button>
      <span class="hint">${esc(t("out_list", o.address_label))}</span>
      <a class="button small ghost" href="${list("csv")}" data-out-list>CSV</a>
      <a class="button small ghost" href="${list("json")}" data-out-list>JSON</a>
    </div>
    <div class="out-preview" data-out-preview="${o.key}" hidden></div>`;
}

function renderOutputs() {
  $("tr-outputs").innerHTML = transfer.data.outputs
    .map((o) => {
      const st = outputState(o.key);
      const open = transfer.outputOpen === o.key;
      return `<div class="out-row">
        <div class="out-head"><b>${esc(o.label)}</b><span class="out-state" data-out-state="${o.key}">${st.chip}<span class="hint">${esc(st.text)}</span></span><span class="spacer"></span>
          <button type="button" class="small ${open ? "quiet" : "tonal"}" data-out-settings="${o.key}">${esc(t(open ? "history_close" : "out_settings"))}</button></div>
        ${open ? `<form class="out-form" data-out-form="${o.key}">${outputForm(o.key)}</form>` : ""}
        ${outputTools(o)}
      </div>`;
    })
    .join("");
  if (transfer.outputOpen === "mqtt") updateMqttForm();
}

/// Felder der MQTT-Einstellungen (Baustein Formular)
function mqttFields() {
  const base = mqttBase();
  return [
    { name: "enabled", label: t("mqtt_enabled"), type: "check", wide: true },
    { name: "host", label: t("mqtt_host"), cls: "mono", placeholder: "192.168.1.10", wide: true },
    { name: "port", label: t("mqtt_port"), type: "number", cls: "mono" },
    { name: "tls", label: "TLS", type: "check" },
    { name: "insecure", label: t("mqtt_insecure"), type: "check", wide: true },
    { name: "username", label: t("mqtt_user") },
    { name: "password", label: t("mqtt_password"), type: "password", placeholder: mqttHasPassword ? t("mqtt_pass_saved") : "" },
    { name: "client_id", label: t("mqtt_client"), cls: "mono", placeholder: "mbusio-…" },
    { name: "base", label: t("mqtt_base"), cls: "mono", placeholder: "mbusio" },
    { name: "topic", label: t("mqtt_topic"), cls: "mono", wide: true, placeholder: "{projekt}/{gateway}-{linie}/{zaehler}/{wert}", hint: t("mqtt_topic_hint") },
    { name: "time_sync", label: t("mqtt_time", base + "/time"), type: "check", wide: true },
    { name: "ca", label: t("mqtt_ca"), type: "textarea", cls: "mono", placeholder: "-----BEGIN CERTIFICATE-----", wide: true },
  ];
}

function outputForm(key) {
  if (key !== "mqtt") return "";
  return `${ui.form(mqttFields(), { ...mqttConfig, password: "" })}
    <p class="hint mono" id="mqtt-example"></p>
    <div class="row"><button type="submit">${esc(t("save"))}</button></div>`;
}

/// TLS-Felder nur mit TLS; Beispiel-Topic aktuell halten
function updateMqttForm() {
  const f = document.querySelector('[data-out-form="mqtt"]');
  if (!f) return;
  const tls = f.querySelector("[name=tls]").checked;
  for (const n of ["insecure", "ca"]) f.querySelector(`[data-field="${n}"]`).hidden = !tls;
  const base = (f.querySelector("[name=base]").value.trim() || "mbusio").replace(/\/+$/, "");
  const sample = allDatapoints().find((d) => d.outputs.mqtt?.address)?.outputs.mqtt.address || "projekt/gateway-1-linie-1/whg-1/energy";
  $("mqtt-example").textContent = t("mqtt_example", `${base}/${sample}`);
}

$("tr-outputs").addEventListener("click", (ev) => {
  const re = ev.target.closest("[data-out-readdress]");
  if (re) {
    const key = re.dataset.outReaddress;
    const path = `projects/${tree.active.project.id}/outputs/${key}/readdress`;
    return guarded(re, () =>
      ui.previewApply(document.querySelector(`[data-out-preview="${key}"]`), {
        title: re.textContent,
        run: (apply) => api(path, { method: "POST", body: { apply } }),
        done: async () => {
          toast(t("out_readdressed"));
          await loadTransfer();
        },
      })
    );
  }
  const b = ev.target.closest("[data-out-settings]");
  if (!b) return;
  transfer.outputOpen = transfer.outputOpen === b.dataset.outSettings ? null : b.dataset.outSettings;
  renderOutputs();
});

$("tr-outputs").addEventListener("change", (ev) => {
  const f = ev.target.closest('[data-out-form="mqtt"]');
  if (!f) return;
  if (ev.target.name === "tls") {
    // üblicher Port mitwechseln, solange man keinen eigenen hat
    const port = f.querySelector("[name=port]");
    if (["", "1883", "8883"].includes(port.value.trim())) port.value = ev.target.checked ? "8883" : "1883";
  }
  updateMqttForm();
});

$("tr-outputs").addEventListener("submit", (ev) => {
  ev.preventDefault();
  const f = ev.target;
  guarded(f.querySelector("[type=submit]"), async () => {
    const v = ui.formValues(f, mqttFields());
    await api("mqtt", {
      method: "PUT",
      body: {
        config: { ...v, port: v.port || (v.tls ? 8883 : 1883), base: v.base || "mbusio", password: "" },
        // leer = gespeichertes behalten; Benutzer gelöscht = Passwort auch
        password: v.password ? v.password : v.username ? null : "",
      },
    });
    toast(t("mqtt_saved"));
    transfer.outputOpen = null;
    await loadTransfer();
    setTimeout(() => refreshStatus().catch(() => {}), 1500);
  });
});

// ------------------------------------------------------------ Datenpunkte je Zähler

function qualityChip(d) {
  const kind = { good: "ok", stale: "muted", disturbed: "warn", invalid: "warn", none: "muted" }[d.quality];
  const title =
    d.quality === "invalid"
      ? t({ status: "q_status_title", error_value: "q_error_value_title" }[d.invalid] || "q_decreased_title")
      : d.quality === "stale"
        ? t("q_stale_title")
        : d.quality === "disturbed"
          ? t("q_disturbed_title")
          : "";
  return ui.chip(kind, t("q_" + d.quality), title);
}

/// Spalten der Datenpunkt-Tabelle (eine Spalte je Ausgang)
function dpColumns() {
  return [
    {
      headHtml: `<input type="checkbox" id="tr-all" title="${esc(t("dp_select_all"))}">`,
      cell: (d) => `<input type="checkbox" data-dp-sel="${d.id}" ${transfer.selected.has(d.id) ? "checked" : ""} aria-label="${esc(d.name)}">`,
    },
    { head: t("th_no"), cls: "num mono", cell: (d) => d.no },
    { head: t("th_dp"), cls: "wrap", cell: (d) => `<span class="name">${esc(d.name)}</span><span class="sub">${esc(d.label)}</span>` },
    {
      head: t("th_value"),
      cls: "num",
      cell: (d) =>
        (d.last_value != null ? `<span class="val">${esc(formatNum(d.last_value, 6))} ${esc(d.unit)}</span>` : "") +
        `<span class="sub">${esc(d.last_time || "")}</span>`,
    },
    { head: t("th_quality"), cell: qualityChip },
    { head: t("th_send"), cell: (d) => esc(t("send_" + d.send)) },
    { head: t("th_active"), cell: (d) => `<input type="checkbox" data-dp-active="${d.id}" ${d.active ? "checked" : ""} aria-label="${esc(t("th_active"))}">` },
    ...transfer.data.outputs.map((o) => ({
      head: o.label,
      cls: "wrap out-cell",
      cell: (d) => {
        const x = d.outputs[o.key] || {};
        return `<input type="checkbox" data-dp-out="${o.key}" data-id="${d.id}" ${x.enabled ? "checked" : ""} aria-label="${esc(o.label)}">${
          x.enabled && x.address
            ? `<span class="sub mono out-addr" title="${esc((o.key === "mqtt" ? mqttBase() + "/" : "") + x.address)}"><bdi dir="ltr">${esc(x.address)}</bdi></span>`
            : ""
        }`;
      },
    })),
    { head: "", cell: (d) => ui.iconBtn(ICON_EDIT, t("edit"), { "dp-edit": d.id }) },
  ];
}

/// Zeile des Zählers über seinen Datenpunkten, mit Knopf zum Wählen der Werte
function meterRow(m, colspan) {
  const open = transfer.picking.has(m.id);
  const ident = m.meter_id || m.address;
  // ohne Namen liefert der Server die ID als Namen: wie in der Zählerliste „ohne Namen“ und die ID
  const block = {
    name: m.name !== ident ? m.name : "",
    ident,
    medium: m.medium,
    info: [m.location, transfer.scope === "project" ? m.line : null].filter(Boolean).join(" · "),
  };
  return `<tr class="tr-meter" data-tr-meter="${m.id}"><td colspan="${colspan}">
    <div class="tr-meter-head">${ui.meter(block)}
      ${ui.chip(m.datapoints.length ? "ok" : "muted", t("tr_dp_count", m.datapoints.length))}
      <span class="spacer"></span>
      <button type="button" class="small ${open ? "quiet" : "ghost"}" data-tr-pick="${m.id}">${esc(t(open ? "history_close" : "tr_pick"))}</button></div>
    ${open ? pickerHtml(m) : ""}
  </td></tr>`;
}

/// Werte eines Zählers zum Anhaken (das ist die Auswahl, was weitergegeben wird)
function pickerHtml(m) {
  const p = transfer.picking.get(m.id);
  if (!p) return `<p class="loading"><span class="spin"></span></p>`;
  if (!p.values.length)
    return `<div class="tr-pick"><p class="hint">${esc(t("dp_pick_none"))}</p>
      <button type="button" class="small ghost needs-conn" data-tr-read="${m.id}">${esc(t("read"))}</button></div>`;
  const any = p.values.some((v) => v.selected);
  const medium = p.meter.medium ? label(p.meter.medium) : "";
  const on = transfer.data.outputs.filter((o) => o.on);
  return `<div class="tr-pick">
    <p class="hint">${esc(t("dp_pick_read", p.read))}</p>
    <div class="dp-pick">${p.values
      .map(
        (v) => `<label class="dp-pick-row"><input type="checkbox" value="${esc(v.key)}" ${v.selected || (!any && v.main) ? "checked" : ""}>
          <span>${esc(v.label)}${v.main ? " " + ui.chip("ok", t("dp_main")) : ""}</span>
          <span class="mono">${v.value != null ? `${esc(v.value)} ${esc(v.unit)}` : `<span class="muted">${esc(t("dp_missing"))}</span>`}</span></label>`
      )
      .join("")}</div>
    ${
      medium
        ? `<label class="check"><input type="checkbox" data-pick-tpl ${p.template ? "checked" : ""}> ${esc(t("dp_tpl", medium))}</label>
           <label class="check sub-check"><input type="checkbox" data-pick-auto ${p.template?.auto ? "checked" : ""}> ${esc(t("dp_tpl_auto"))}</label>`
        : ""
    }
    ${on
      .map(
        (o) =>
          `<label class="check"><input type="checkbox" data-pick-out="${o.key}" ${p.template?.outputs?.includes(o.key) || !p.template ? "checked" : ""}> ${esc(
            t("tr_pick_out", o.label)
          )}</label>`
      )
      .join("")}
    <div class="row"><button type="button" data-pick-save="${m.id}">${esc(t("save"))}</button></div>
  </div>`;
}

function renderDatapointTable() {
  const cols = dpColumns();
  const meters = transfer.data.meters;
  const box = $("tr-table");
  if (!meters.length) {
    box.innerHTML = `<p class="empty">${esc(t("tr_no_meters"))}</p>`;
  } else {
    box.innerHTML = `<div class="table-wrap"><table id="dp-table">${ui.head(cols)}<tbody>${meters
      .map(
        (m) =>
          meterRow(m, cols.length) +
          m.datapoints.map((d) => `<tr class="${transfer.selected.has(d.id) ? "selected" : ""}">${ui.cells(cols, d)}</tr>`).join("")
      )
      .join("")}</tbody></table></div>`;
    const all = allDatapoints();
    $("tr-all").checked = all.length > 0 && all.every((d) => transfer.selected.has(d.id));
  }
  $("dp-bulk").hidden = transfer.selected.size === 0;
  $("dp-bulk-n").textContent = t("dp_selected", transfer.selected.size);
  $("dp-bulk-send").innerHTML =
    `<option value="">${esc(t("dp_send_to"))}</option>` + SEND_MODES.map((m) => `<option value="${m}">${esc(t("send_" + m))}</option>`).join("");
  $("dp-bulk-out").innerHTML =
    `<option value="">${esc(t("tr_bulk_out"))}</option>` +
    transfer.data.outputs.flatMap((o) => [`<option value="out:${o.key}:on">${esc(o.label)}: ${esc(t("on"))}</option>`, `<option value="out:${o.key}:off">${esc(o.label)}: ${esc(t("off"))}</option>`]).join("");
  updateButtons();
}

function renderTemplates() {
  const tpl = $("dp-templates");
  const list = transfer.data.templates;
  tpl.hidden = !list.length;
  tpl.innerHTML =
    `<b>${esc(t("dp_templates"))}</b> ` +
    list
      .map(
        (x) => `<span class="dp-tpl">${esc(label(x.medium))}: ${esc(t("dp_tpl_n", x.keys.length))}${x.auto ? ` · ${esc(t("dp_tpl_auto_short"))}` : ""}${
          x.outputs?.length ? ` · ${esc(x.outputs.map(outputLabelOf).join(", "))}` : ""
        }
        ${ui.iconBtn(ICON_DEL, t("dp_tpl_remove"), { "tpl-del": x.id, medium: label(x.medium) })}</span>`
      )
      .join(" ");
}

const outputLabelOf = (key) => transfer.data.outputs.find((o) => o.key === key)?.label || key;

async function togglePick(meterId, open = !transfer.picking.has(meterId)) {
  if (!open) {
    transfer.picking.delete(meterId);
    return renderDatapointTable();
  }
  transfer.picking.set(meterId, null);
  renderDatapointTable();
  transfer.picking.set(meterId, await api(`meters/${meterId}/values`));
  renderDatapointTable();
}

$("tr-table").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  const d = b.dataset;
  if (d.trPick) return guarded(b, () => togglePick(Number(d.trPick)));
  if (d.trRead)
    return guarded(b, async () => {
      await api(`meters/${d.trRead}/read`, { method: "POST" });
      transfer.picking.set(Number(d.trRead), await api(`meters/${d.trRead}/values`));
      renderDatapointTable();
    });
  if (d.dpEdit) return openDpForm(allDatapoints().find((x) => x.id === Number(d.dpEdit)));
  if (d.pickSave) {
    const row = b.closest(".tr-pick");
    const keys = [...row.querySelectorAll(".dp-pick input:checked")].map((x) => x.value);
    const outputs = [...row.querySelectorAll("[data-pick-out]:checked")].map((x) => x.dataset.pickOut);
    const tpl = row.querySelector("[data-pick-tpl]")?.checked;
    return guarded(b, async () => {
      const r = await api(`meters/${d.pickSave}/datapoints`, {
        method: "PUT",
        body: { keys, outputs, template: tpl ? { auto: !!row.querySelector("[data-pick-auto]")?.checked } : null },
      });
      toast(t("dp_saved", r.added, r.removed, r.others));
      transfer.picking.delete(Number(d.pickSave));
      await loadTransfer();
      if (tree) loadTree().catch(() => {});
    });
  }
});

$("tr-table").addEventListener("change", (ev) => {
  const el = ev.target;
  const bulk = (ids, action) =>
    guarded(el, async () => {
      await api("datapoints/bulk", { method: "POST", body: { ids, action } });
      await loadTransfer();
    });
  if (el.dataset.dpSel) {
    const id = Number(el.dataset.dpSel);
    if (el.checked) transfer.selected.add(id);
    else transfer.selected.delete(id);
    return renderDatapointTable();
  }
  if (el.id === "tr-all") {
    transfer.selected.clear();
    if (el.checked) for (const d of allDatapoints()) transfer.selected.add(d.id);
    return renderDatapointTable();
  }
  if (el.dataset.dpActive) return bulk([Number(el.dataset.dpActive)], el.checked ? "on" : "off");
  if (el.dataset.dpOut) return bulk([Number(el.dataset.id)], `out:${el.dataset.dpOut}:${el.checked ? "on" : "off"}`);
});

$("dp-templates").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-tpl-del]");
  if (!b || !confirm(t("confirm_tpl_remove", b.dataset.medium))) return;
  guarded(b, async () => {
    await api("templates/" + b.dataset.tplDel, { method: "DELETE" });
    await loadTransfer();
  });
});

// Massenbearbeitung der markierten Datenpunkte
async function dpBulk(action) {
  const ids = [...transfer.selected];
  if (!ids.length) return;
  if (action === "delete" && !confirm(t("confirm_dp_delete", ids.length))) return;
  await api("datapoints/bulk", { method: "POST", body: { ids, action } });
  if (action === "delete") transfer.selected.clear();
  await loadTransfer();
  if (tree) loadTree().catch(() => {});
}

$("dp-bulk").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-dp-bulk]");
  if (b) guarded(b, () => dpBulk(b.dataset.dpBulk));
});
$("dp-bulk").addEventListener("change", (ev) => {
  const el = ev.target;
  const v = el.value;
  if (!v) return;
  guarded(el, () => dpBulk(el.id === "dp-bulk-send" ? "send:" + v : v));
});

$("tr-scope").onchange = () => {
  transfer.scope = $("tr-scope").value;
  storageSet("transfer_scope", transfer.scope);
  loadTransfer().catch((e) => toast(e.message, true));
};

// ------------------------------------------------------------ Datenpunkt bearbeiten

function dpFields() {
  return [
    { name: "name", label: t("f_name"), required: true, wide: true },
    { name: "unit", label: t("f_unit") },
    { name: "factor", label: t("f_factor"), type: "number", cls: "mono", required: true },
    { name: "send", label: t("f_send"), type: "select", options: SEND_MODES.map((m) => [m, t("send_" + m)]), wide: true },
    { name: "active", label: t("f_active"), type: "check" },
    ...transfer.data.outputs.flatMap((o) => [
      { name: `out_${o.key}`, label: t("tr_pick_out", o.label), type: "check" },
      { name: `addr_${o.key}`, label: o.address_label, cls: "mono", wide: true, placeholder: t("f_topic_ph") },
    ]),
  ];
}

function openDpForm(d) {
  const values = { ...d };
  for (const o of transfer.data.outputs) {
    values[`out_${o.key}`] = !!d.outputs[o.key]?.enabled;
    values[`addr_${o.key}`] = d.outputs[o.key]?.address || "";
  }
  ui.show(
    $("tr-edit"),
    ui.panel(
      `${t("dp_form_title")}: ${d.no} · ${d.name}`,
      `<form id="dp-form">${ui.form(dpFields(), values)}<p class="hint">${esc(t("f_factor_hint"))}</p>
        <div class="row"><button type="submit">${esc(t("save"))}</button></div></form>`
    )
  );
  $("dp-form").onsubmit = (ev) => {
    ev.preventDefault();
    guarded(ev.submitter, async () => {
      const v = ui.formValues($("dp-form"), dpFields());
      const outputs = Object.fromEntries(transfer.data.outputs.map((o) => [o.key, { enabled: v[`out_${o.key}`], address: v[`addr_${o.key}`] || null }]));
      await api("datapoints/" + d.id, {
        method: "PUT",
        body: { ...d, name: v.name, unit: v.unit, factor: v.factor, send: v.send, active: v.active, outputs },
      });
      ui.show($("tr-edit"), "");
      await loadTransfer();
    });
  };
}

// ------------------------------------------------------------ CSV

$("tr-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("tr-export"), async () => {
    const rows = parseTable(await file.text());
    const path = `projects/${tree.active.project.id}/datapoints/import`;
    await ui.previewApply($("tr-preview"), {
      title: t("dp_csv_import"),
      run: (apply) => api(path, { method: "POST", body: { rows, apply } }),
      done: async () => {
        toast(t("dp_import_done"));
        await loadTransfer();
      },
    });
  });
};
