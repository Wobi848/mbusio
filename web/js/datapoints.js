"use strict";

// Datenpunkte: Auswahl, Vorlagen, Bearbeiten, CSV

// ------------------------------------------------------------ Datenpunkte

let datapoints = [];
let dpTemplates = [];
const dpSelected = new Set();
let dpEditing = null;
let picker = null;
let importText = null;
const SEND_MODES = ["every", "change", "daily"];

async function loadDatapoints() {
  const r = await api("datapoints");
  datapoints = r.datapoints;
  dpTemplates = r.templates;
  for (const id of [...dpSelected]) if (!datapoints.some((d) => d.id === id)) dpSelected.delete(id);
  renderDatapoints();
}

function qualityChip(d) {
  const cls = { good: "st-ok", stale: "st-no_data", disturbed: "st-collision", invalid: "st-collision", none: "st-no_data" }[d.quality];
  const title =
    d.quality === "invalid"
      ? t(d.invalid === "status" ? "q_status_title" : "q_decreased_title")
      : d.quality === "stale"
        ? t("q_stale_title")
        : d.quality === "disturbed"
          ? t("q_disturbed_title")
          : "";
  return `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(t("q_" + d.quality))}</span>`;
}

function renderDatapoints() {
  if (tree) {
    const pid = tree.active.project.id;
    $("dp-export").href = `api/projects/${pid}/datapoints/csv?lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  }
  $("dp-table").hidden = !datapoints.length;
  $("dp-empty").hidden = datapoints.length > 0;
  $("dp-table").querySelector("tbody").innerHTML = datapoints
    .map(
      (d) => `<tr class="${dpSelected.has(d.id) ? "selected" : ""}">
      <td><input type="checkbox" data-dp-sel="${d.id}" ${dpSelected.has(d.id) ? "checked" : ""} aria-label="${esc(d.name)}"></td>
      <td class="num mono">${d.no}</td>
      <td class="wrap"><span class="name">${esc(d.name)}</span><span class="sub">${esc(d.meter)} · ${esc(d.label)}</span>${
        d.mqtt && d.mqtt_topic ? `<span class="sub mono">${esc(mqttBase())}/${esc(d.mqtt_topic)}</span>` : ""
      }</td>
      <td class="num">${d.last_value != null ? `<span class="val">${esc(formatNum(d.last_value, 6))} ${esc(d.unit)}</span>` : ""}<span class="sub">${esc(d.last_time || "")}</span></td>
      <td>${qualityChip(d)}</td>
      <td>${esc(t("send_" + d.send))}</td>
      <td><input type="checkbox" data-dp-active="${d.id}" ${d.active ? "checked" : ""} aria-label="${esc(t("th_active"))}"></td>
      <td><input type="checkbox" data-dp-mqtt="${d.id}" ${d.mqtt ? "checked" : ""} aria-label="MQTT" title="${esc(d.mqtt_topic || "")}"></td>
      <td><button class="icon-btn" data-dp-edit="${d.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button></td>
    </tr>`
    )
    .join("");
  $("dp-all").checked = datapoints.length > 0 && datapoints.every((d) => dpSelected.has(d.id));
  $("dp-bulk").hidden = dpSelected.size === 0;
  $("dp-bulk-n").textContent = t("dp_selected", dpSelected.size);
  $("dp-bulk-send").innerHTML =
    `<option value="">${esc(t("dp_send_to"))}</option>` + SEND_MODES.map((m) => `<option value="${m}">${esc(t("send_" + m))}</option>`).join("");
  const tpl = $("dp-templates");
  tpl.hidden = !dpTemplates.length;
  tpl.innerHTML =
    `<b>${esc(t("dp_templates"))}</b> ` +
    dpTemplates
      .map(
        (x) => `<span class="dp-tpl">${esc(label(x.medium))}: ${esc(t("dp_tpl_n", x.keys.length))}${x.auto ? ` · ${esc(t("dp_tpl_auto_short"))}` : ""}
        <button class="icon-btn" data-tpl-del="${x.id}" data-medium="${esc(label(x.medium))}" title="${esc(t("dp_tpl_remove"))}" aria-label="${esc(t("dp_tpl_remove"))}">${ICON_DEL}</button></span>`
      )
      .join(" ");
}

$("dp-table").addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.dataset.dpSel) {
    const id = Number(el.dataset.dpSel);
    if (el.checked) dpSelected.add(id);
    else dpSelected.delete(id);
    return renderDatapoints();
  }
  if (el.id === "dp-all") {
    dpSelected.clear();
    if (el.checked) for (const d of datapoints) dpSelected.add(d.id);
    return renderDatapoints();
  }
  if (el.dataset.dpActive) {
    guarded(el, async () => {
      await api("datapoints/bulk", { method: "POST", body: { ids: [Number(el.dataset.dpActive)], action: el.checked ? "on" : "off" } });
      await loadDatapoints();
    });
  }
  if (el.dataset.dpMqtt) {
    guarded(el, async () => {
      await api("datapoints/bulk", { method: "POST", body: { ids: [Number(el.dataset.dpMqtt)], action: el.checked ? "mqtt:on" : "mqtt:off" } });
      await loadDatapoints();
    });
  }
});

$("dp-table").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-dp-edit]");
  if (b) openDpForm(datapoints.find((d) => d.id === Number(b.dataset.dpEdit)));
});

$("dp-templates").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-tpl-del]");
  if (!b || !confirm(t("confirm_tpl_remove", b.dataset.medium))) return;
  guarded(b, async () => {
    await api("templates/" + b.dataset.tplDel, { method: "DELETE" });
    await loadDatapoints();
  });
});

async function dpBulk(action) {
  const ids = [...dpSelected];
  if (!ids.length) return;
  if (action === "delete" && !confirm(t("confirm_dp_delete", ids.length))) return;
  await api("datapoints/bulk", { method: "POST", body: { ids, action } });
  if (action === "delete") dpSelected.clear();
  await loadDatapoints();
  if (tree) loadTree().catch(() => {});
}

$("dp-bulk").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-dp-bulk]");
  if (b) guarded(b, () => dpBulk(b.dataset.dpBulk));
});
$("dp-bulk-send").onchange = () => {
  const m = $("dp-bulk-send").value;
  if (m) guarded($("dp-bulk-send"), () => dpBulk("send:" + m));
};

function openDpForm(d) {
  dpEditing = d;
  $("dpf-name").value = d.name;
  $("dpf-unit").value = d.unit;
  $("dpf-factor").value = d.factor;
  $("dpf-send").innerHTML = SEND_MODES.map((m) => `<option value="${m}" ${m === d.send ? "selected" : ""}>${esc(t("send_" + m))}</option>`).join("");
  $("dpf-active").checked = d.active;
  $("dpf-mqtt").checked = d.mqtt;
  $("dpf-topic").value = d.mqtt_topic || "";
  $("dp-form").hidden = false;
  $("dp-form").scrollIntoView({ behavior: "smooth", block: "nearest" });
  $("dpf-name").focus();
}

$("dpf-cancel").onclick = () => ($("dp-form").hidden = true);
$("dp-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("dp-form").querySelector("[type=submit]"), async () => {
    const factor = Number(String($("dpf-factor").value).replace(",", "."));
    await api("datapoints/" + dpEditing.id, {
      method: "PUT",
      body: {
        ...dpEditing,
        name: $("dpf-name").value.trim(),
        unit: $("dpf-unit").value.trim(),
        factor,
        send: $("dpf-send").value,
        active: $("dpf-active").checked,
        mqtt: $("dpf-mqtt").checked,
        mqtt_topic: $("dpf-topic").value.trim() || null,
      },
    });
    $("dp-form").hidden = true;
    await loadDatapoints();
  });
};

/// Auswahl der Werte eines Zählers (unter der Zählerliste)
async function openPicker(m) {
  picker = await api(`meters/${m.id}/values`);
  renderPicker();
  $("dp-picker").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function renderPicker() {
  const box = $("dp-picker");
  if (!picker) return (box.hidden = true);
  const m = picker.meter;
  const name = m.name || m.meter_id || m.address;
  const none = !picker.values.length;
  // noch nichts gewählt: Hauptwert vorschlagen
  const anySelected = picker.values.some((v) => v.selected);
  const medium = m.medium ? label(m.medium) : "";
  const tpl = picker.template;
  box.innerHTML = `<div class="history-head"><h3>${esc(t("dp_pick_title", name))}</h3><span class="spacer"></span>
      <button type="button" class="small quiet" data-pick-close>${esc(t("history_close"))}</button></div>
    <p class="hint">${esc(none ? t("dp_pick_none") : t("dp_pick_read", picker.read))}</p>
    ${
      none
        ? ""
        : `<div class="dp-pick">${picker.values
            .map(
              (v) => `<label class="dp-pick-row"><input type="checkbox" value="${esc(v.key)}" ${v.selected || (!anySelected && v.main) ? "checked" : ""}>
          <span>${esc(v.label)}${v.main ? ` <span class="chip st-ok">${esc(t("dp_main"))}</span>` : ""}</span>
          <span class="mono">${v.value != null ? `${esc(v.value)} ${esc(v.unit)}` : `<span class="muted">${esc(t("dp_missing"))}</span>`}</span></label>`
            )
            .join("")}</div>
      ${
        medium
          ? `<label class="check"><input type="checkbox" id="pick-tpl" ${tpl ? "checked" : ""}> ${esc(t("dp_tpl", medium))}</label>
             <label class="check sub-check"><input type="checkbox" id="pick-auto" ${tpl?.auto ? "checked" : ""}> ${esc(t("dp_tpl_auto"))}</label>`
          : ""
      }
      <label class="check"><input type="checkbox" id="pick-mqtt" ${tpl?.mqtt || mqttConfig?.enabled ? "checked" : ""}> ${esc(t("dp_pick_mqtt"))}</label>
      <div class="row"><button type="button" data-pick-save>${esc(t("save"))}</button></div>`
    }`;
  box.hidden = false;
}

$("dp-picker").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-pick-close]")) {
    picker = null;
    return renderPicker();
  }
  const b = ev.target.closest("[data-pick-save]");
  if (!b) return;
  guarded(b, async () => {
    const keys = [...$("dp-picker").querySelectorAll(".dp-pick input:checked")].map((x) => x.value);
    const tplOn = $("pick-tpl")?.checked;
    const body = { keys, mqtt: !!$("pick-mqtt")?.checked, template: tplOn ? { auto: !!$("pick-auto")?.checked } : null };
    const r = await api(`meters/${picker.meter.id}/datapoints`, { method: "PUT", body });
    toast(t("dp_saved", r.added, r.removed, r.others));
    picker = null;
    renderPicker();
    await loadDatapoints();
    if (tree) loadTree().catch(() => {});
    $("sec-datapoints").scrollIntoView({ behavior: "smooth", block: "nearest" });
  });
});

// CSV: erst Vorschau, dann übernehmen
$("dp-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("dp-export"), async () => {
    importText = await file.text();
    const r = await api(`projects/${tree.active.project.id}/datapoints/import`, { method: "POST", body: { csv: importText, apply: false } });
    renderImportPreview(r);
  });
};

function renderImportPreview(r) {
  const box = $("dp-import-preview");
  const n = r.changes.length + r.added.length;
  const field = (f) => t("dp_field_" + f);
  box.innerHTML = `<div class="history-head"><h3>${esc(t("dp_import_preview", r.changes.length, r.added.length, r.errors.length))}</h3></div>
    ${r.errors.length ? `<ul class="err-list">${r.errors.map((e) => `<li>${esc(e)}</li>`).join("")}</ul><p class="hint">${esc(t("dp_import_errors"))}</p>` : ""}
    ${!n && !r.errors.length ? `<p class="hint">${esc(t("dp_import_nothing"))}</p>` : ""}
    ${
      n
        ? `<div class="table-wrap"><table><tbody>${r.changes
            .map((c) => `<tr><td class="num mono">${c.no}</td><td>${esc(c.name)}</td><td>${esc(field(c.field))}</td><td class="mono">${esc(c.old)} → <b>${esc(c.new)}</b></td></tr>`)
            .join("")}${r.added
            .map((a) => `<tr><td><span class="chip st-ok">${esc(t("dp_new"))}</span></td><td>${esc(a.name)}</td><td colspan="2">${esc(a.meter)}</td></tr>`)
            .join("")}</tbody></table></div>`
        : ""
    }
    <div class="row">${n && !r.errors.length ? `<button type="button" data-imp-apply>${esc(t("dp_import_apply"))}</button>` : ""}
      <button type="button" class="quiet" data-imp-cancel>${esc(t("dp_import_cancel"))}</button></div>`;
  box.hidden = false;
  box.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

$("dp-import-preview").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-imp-cancel]")) {
    importText = null;
    $("dp-import-preview").hidden = true;
    return;
  }
  const b = ev.target.closest("[data-imp-apply]");
  if (!b) return;
  guarded(b, async () => {
    const r = await api(`projects/${tree.active.project.id}/datapoints/import`, { method: "POST", body: { csv: importText, apply: true } });
    if (!r.applied) return renderImportPreview(r);
    importText = null;
    $("dp-import-preview").hidden = true;
    toast(t("dp_import_done"));
    await loadDatapoints();
  });
});

$("mf-exchange").onclick = () => {
  const m = meters.find((x) => x.id === editingId);
  if (!m) return;
  const addr = prompt(t("exchange_prompt"));
  if (!addr || !addr.trim()) return;
  if (!confirm(t("exchange_confirm", m.meter_id || m.address, addr.trim()))) return;
  guarded($("mf-exchange"), async () => {
    const r = await api(`meters/${m.id}/exchange`, { method: "POST", body: { address: addr.trim() } });
    $("meter-form").hidden = true;
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  });
};
