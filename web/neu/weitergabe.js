// Bereich Weitergabe: oben die Ausgänge (MQTT, Modbus) mit Schalter und Zustand, darunter die
// Datenpunkte als Liste, Details rechts (Konzept neue Oberfläche §3.2, Schritt 4).

import { state, watch } from "./store.js";
import { html, text, list, ask, toast, styles, texts, previewApply, menu } from "./view.js";
import { onEscape } from "./keys.js";
import { loadTree, report } from "./data.js";

texts(
  {
    wg_outputs: "Ausgänge",
    wg_running: "läuft",
    wg_off: "aus",
    wg_connecting: "verbindet …",
    wg_error: "gestört",
    wg_mqtt_info: (broker, sent, queued) => `${broker} · ${sent} gesendet · ${queued} wartend`,
    wg_modbus_info: (listen, clients) => `hört auf ${listen} · ${clients} Leitsystem${clients === 1 ? "" : "e"}`,
    wg_settings: "Einstellungen",
    wg_save: "Speichern",
    wg_need_host: "Zuerst die Broker-Adresse eintragen",
    wg_off_q: (o) => `${o} ausschalten? Es wird dann nichts mehr an das Leitsystem weitergegeben.`,
    wg_col_no: "Nr",
    wg_col_name: "Datenpunkt",
    wg_col_quality: "Qualität",
    wg_col_out: "Ausgänge",
    wg_col_send: "Senden",
    wg_none: "Noch keine Datenpunkte. Werte wählt man bei den Zählern (Kästchen „als Datenpunkt“).",
    wg_to_meters: "Zu den Zählern",
    wg_problems: "Probleme",
    wg_pick: "Datenpunkt in der Liste wählen (↑↓) oder einen Ausgang einstellen.",
    wg_factor: "Faktor",
    wg_shift: "Verschiebung",
    wg_active: "aktiv",
    wg_inactive: "inaktiv",
    wg_import: "CSV importieren",
    wg_templates: (n) => `Vorlagen (${n})`,
    wg_tpl_del_q: (x) => `Vorlage „${x}“ löschen? Bestehende Datenpunkte bleiben.`,
    wg_delete_q: (n) => `${n} Datenpunkt${n === 1 ? "" : "e"} löschen? Ihre Werte werden nicht mehr weitergegeben.`,
    wg_out_on: (o) => `${o} ein`,
    wg_out_off: (o) => `${o} aus`,
    wg_send_set: "Senden …",
  },
  {
    wg_outputs: "Outputs",
    wg_running: "running",
    wg_off: "off",
    wg_connecting: "connecting …",
    wg_error: "faulty",
    wg_mqtt_info: (broker, sent, queued) => `${broker} · ${sent} sent · ${queued} waiting`,
    wg_modbus_info: (listen, clients) => `listening on ${listen} · ${clients} control system${clients === 1 ? "" : "s"}`,
    wg_settings: "Settings",
    wg_save: "Save",
    wg_need_host: "Enter the broker address first",
    wg_off_q: (o) => `Switch off ${o}? Nothing is passed on to the control system any more.`,
    wg_col_no: "No",
    wg_col_name: "Datapoint",
    wg_col_quality: "Quality",
    wg_col_out: "Outputs",
    wg_col_send: "Send",
    wg_none: "No datapoints yet. Values are chosen at the meters (box “as datapoint”).",
    wg_to_meters: "Go to meters",
    wg_problems: "Problems",
    wg_pick: "Pick a datapoint in the list (↑↓) or set up an output.",
    wg_factor: "Factor",
    wg_shift: "Offset",
    wg_active: "active",
    wg_inactive: "inactive",
    wg_import: "Import CSV",
    wg_templates: (n) => `Templates (${n})`,
    wg_tpl_del_q: (x) => `Delete template “${x}”? Existing datapoints stay.`,
    wg_delete_q: (n) => `Delete ${n} datapoint${n === 1 ? "" : "s"}? Their values are no longer passed on.`,
    wg_out_on: (o) => `${o} on`,
    wg_out_off: (o) => `${o} off`,
    wg_send_set: "Send …",
  }
);

const SEND = ["every", "change", "daily"];
const QUALITY = { good: "ok", stale: "warn", disturbed: "err", invalid: "err", none: "" };

let root = null;
let data = null; // /api/datapoints: { meters (mit datapoints), templates, outputs }
let cfg = { mqtt: null, modbus: null }; // Einstellungen der Ausgänge
const view = { scope: storageGet("wg_scope") || "line", search: "", problems: false, active: null, output: null, selected: new Set(), preview: false, enableOnSave: false };
let releaseEscape = null;
const q = (sel) => root.querySelector(sel);
/// Erklärung einer Qualität (nur wo es etwas zu erklären gibt)
const qualityTitle = (p) => (["stale", "disturbed"].includes(p.quality) ? t(`q_${p.quality}_title`) : p.quality === "invalid" ? t(p.invalid === "status" ? "q_status_title" : p.invalid === "error_value" ? "q_error_value_title" : "q_decreased_title") : "");

export function mount(pane) {
  styles("weitergabe");
  root = pane;
  root.innerHTML = `
    <div class="outputs" data-outputs></div>
    <div class="toolbar">
      <input type="search" data-search placeholder="${esc(t("n_search"))}" aria-label="${esc(t("n_search"))}">
      <div class="seg" role="group" data-scope></div>
      <button type="button" class="seg-like" data-problems></button>
      <span class="grow"></span>
      <div class="menu-host"><button type="button" class="quiet" data-tpl-btn hidden></button><div class="menu" data-tpl-menu hidden></div></div>
      <a class="quiet" data-export>CSV</a>
      <label class="quiet filebtn">${esc(t("wg_import"))}<input type="file" accept=".csv,text/csv" data-import hidden></label>
    </div>
    <div class="split">
      <div class="listwrap">
        <div class="row head" aria-hidden="true"><span></span><span>${esc(t("wg_col_no"))}</span><span>${esc(t("wg_col_name"))}</span><span>${esc(t("history_series"))}</span><span>${esc(
          t("wg_col_quality")
        )}</span><span>${esc(t("wg_col_out"))}</span><span>${esc(t("wg_col_send"))}</span></div>
        <div class="rows" role="listbox" tabindex="0" aria-multiselectable="true" data-list></div>
        <div class="empty" data-empty hidden></div>
      </div>
      <aside class="detail" data-detail></aside>
    </div>
    <div class="selbar" data-selbar hidden></div>`;
  wire();
  watch(["status"], onLine);
  watch(["status", "tree"], render);
}

// ------------------------------------------------------------ Daten

let lineId = null;
async function onLine() {
  const id = state.status?.active?.line?.id;
  if (!id || id === lineId) return;
  lineId = id;
  await load();
}

async function load() {
  try {
    const [d, mq, mb] = await Promise.all([request(`datapoints?scope=${view.scope}`), request("mqtt"), request("modbus")]);
    data = d;
    cfg = { mqtt: mq, modbus: mb };
    const ids = new Set(points().map((p) => p.id));
    for (const id of [...view.selected]) if (!ids.has(id)) view.selected.delete(id);
  } catch (e) {
    report(e);
  }
  render();
}

const points = () => (data ? data.meters.flatMap((m) => m.datapoints) : []);
const byId = (id) => points().find((p) => p.id === id);
const projectId = () => state.status?.active?.project?.id;

function shown() {
  const s = view.search.trim().toLowerCase();
  return points().filter(
    (p) => (!view.problems || (p.quality !== "good" && p.quality !== "none")) && (!s || [p.name, p.meter, p.label, p.no].some((x) => String(x ?? "").toLowerCase().includes(s)))
  );
}

// ------------------------------------------------------------ Ausgänge

/// Zustand eines Ausgangs aus /api/status: [farbe, wort, satz]
function outputState(key) {
  const s = state.status?.[key];
  if (!s || s.state === "off") return ["", t("wg_off"), ""];
  if (key === "mqtt") {
    if (s.state === "connected") return ["ok", t("wg_running"), t("wg_mqtt_info", s.broker, s.sent, s.queued)];
    if (s.state === "connecting") return ["", t("wg_connecting"), s.broker || ""];
    return ["err", t("wg_error"), s.error || ""];
  }
  if (s.state === "listening") return ["ok", t("wg_running"), t("wg_modbus_info", s.listen, s.clients)];
  return ["err", t("wg_error"), s.error || ""];
}

function renderOutputs() {
  const outs = data?.outputs || [];
  html(
    q("[data-outputs]"),
    `<span class="eyebrow">${esc(t("wg_outputs"))}</span>` +
      outs
        .map((o) => {
          const [cls, word, info] = outputState(o.key);
          const on = !!o.on;
          const has = points().some((p) => p.outputs[o.key]?.address);
          return `<div class="out${view.output === o.key ? " active" : ""}">
          <label class="out-switch"><input type="checkbox" class="switch" data-out-toggle="${o.key}"${on ? " checked" : ""}><b>${esc(o.label)}</b></label>
          <span class="chip ${cls}">${esc(word)}</span><span class="sub cut" title="${esc(info)}">${esc(info)}</span>
          <span class="grow"></span>
          ${has && projectId() ? `<button type="button" class="quiet" data-readdress="${o.key}">${esc(t("out_readdress", o.address_label))}</button>
            <a class="quiet" href="${apiHref(`projects/${projectId()}/outputs/${o.key}/list?format=csv`)}" title="${esc(t("out_list", o.address_label))}">CSV</a>
            <a class="quiet" href="${apiHref(`projects/${projectId()}/outputs/${o.key}/list?format=json`)}" title="${esc(t("out_list", o.address_label))}">JSON</a>` : ""}
          <button type="button" class="tonal" data-out-settings="${o.key}">${esc(t("wg_settings"))}</button>
        </div>`;
        })
        .join("")
  );
}

async function toggleOutput(input) {
  const key = input.dataset.outToggle;
  const on = input.checked;
  input.checked = !on;
  const label = data.outputs.find((o) => o.key === key)?.label || key;
  if (on && key === "mqtt" && !cfg.mqtt.config.host) {
    toast(t("wg_need_host"));
    // man hat „ein“ gedrückt: nach dem Speichern ist der Ausgang ein
    return openOutput(key, true);
  }
  if (!on && !(await ask(input, t("wg_off_q", label), t("n_switch_off")))) return;
  try {
    if (key === "mqtt") await request("mqtt", { method: "PUT", body: { config: { ...cfg.mqtt.config, enabled: on }, password: null } });
    else await request("modbus", { method: "PUT", body: { ...cfg.modbus.config, enabled: on } });
    await load();
  } catch (e) {
    report(e);
  }
}

function openOutput(key, enable = false) {
  view.output = key;
  view.enableOnSave = enable;
  view.active = null;
  view.preview = false;
  render();
}

/// Felder eines Ausgangs: [name, beschriftung, art, extra]
function outputFields(key) {
  if (key === "mqtt") {
    const base = (cfg.mqtt.config.base || "mbusio").replace(/\/+$/, "");
    return [
      ["host", t("mqtt_host"), "text", { ph: "192.168.1.10", mono: true }],
      ["port", t("mqtt_port"), "number", { mono: true }],
      ["tls", "TLS", "check"],
      ["insecure", t("mqtt_insecure"), "check"],
      ["username", t("mqtt_user"), "text"],
      ["password", t("mqtt_password"), "password", { ph: cfg.mqtt.has_password ? t("mqtt_pass_saved") : "" }],
      ["client_id", t("mqtt_client"), "text", { ph: "mbusio-…", mono: true }],
      ["base", t("mqtt_base"), "text", { ph: "mbusio", mono: true }],
      ["topic", t("mqtt_topic"), "text", { ph: "{projekt}/{gateway}-{linie}/{zaehler}/{wert}", mono: true, hint: t("mqtt_topic_hint") }],
      ["time_sync", t("mqtt_time", base + "/time"), "check"],
      ["ca", t("mqtt_ca"), "textarea", { ph: "-----BEGIN CERTIFICATE-----", mono: true }],
      ["ha", t("mqtt_ha"), "check"],
      ["ha_prefix", t("mqtt_ha_prefix"), "text", { ph: "homeassistant", mono: true, hint: t("mqtt_ha_hint") }],
    ];
  }
  return [
    ["bind", t("modbus_bind"), "text", { ph: "0.0.0.0", mono: true, hint: t("modbus_bind_hint") }],
    ["port", t("modbus_port"), "number", { mono: true, hint: t("modbus_port_hint") }],
    ["project", t("modbus_project"), "select", { options: [[0, t("modbus_project_active")], ...cfg.modbus.projects.map((p) => [p.id, p.name])] }],
    ["swap_words", t("modbus_words"), "select", { options: [[false, t("modbus_words_abcd")], [true, t("modbus_words_cdab")]] }],
  ];
}

function fieldHtml([name, label_, kind, x = {}], value) {
  const cls = x.mono ? ' class="mono"' : "";
  const ph = x.ph ? ` placeholder="${esc(x.ph)}"` : "";
  const input =
    kind === "check"
      ? `<input type="checkbox" name="${name}"${value ? " checked" : ""}>`
      : kind === "select"
        ? `<select name="${name}">${x.options.map(([v, l]) => `<option value="${esc(v)}"${String(v) === String(value) ? " selected" : ""}>${esc(l)}</option>`).join("")}</select>`
        : kind === "textarea"
          ? `<textarea name="${name}" rows="3"${cls}${ph}>${esc(value ?? "")}</textarea>`
          : `<input type="${kind}" name="${name}" value="${kind === "password" ? "" : esc(value ?? "")}"${cls}${ph}>`;
  return `<label${kind === "check" ? ' class="check"' : ""}><span>${esc(label_)}</span>${input}</label>${x.hint ? `<p class="sub hint">${esc(x.hint)}</p>` : ""}`;
}

function outputForm(key) {
  const c = key === "mqtt" ? cfg.mqtt.config : cfg.modbus.config;
  const label_ = data.outputs.find((o) => o.key === key)?.label || key;
  return `<button type="button" class="quiet back" data-back>← ${esc(t("tab_transfer"))}</button>
    <header class="dhead"><b>${esc(label_)} · ${esc(t("wg_settings"))}</b></header>
    <form class="fields" data-out-form="${key}">${outputFields(key).map((f) => fieldHtml(f, c[f[0]])).join("")}
      <div class="actions"><button type="submit">${esc(t("wg_save"))}</button><button type="button" class="quiet" data-back>${esc(t("cancel"))}</button></div>
    </form>`;
}

async function saveOutput(form) {
  const key = form.dataset.outForm;
  const values = view.enableOnSave ? { enabled: true } : {};
  for (const [name, , kind] of outputFields(key)) {
    const el = form.elements[name];
    values[name] = kind === "check" ? el.checked : kind === "number" ? Number(el.value) || 0 : el.value;
  }
  try {
    if (key === "mqtt") {
      const { password, ...rest } = values;
      await request("mqtt", { method: "PUT", body: { config: { ...cfg.mqtt.config, ...rest }, password: password || null } });
    } else {
      await request("modbus", { method: "PUT", body: { ...cfg.modbus.config, ...values, project: Number(values.project), swap_words: values.swap_words === "true" } });
    }
    toast(t(key === "mqtt" ? "mqtt_saved" : "modbus_saved"));
    view.output = null;
    await load();
  } catch (e) {
    report(e);
  }
}

// ------------------------------------------------------------ Datenpunkte

function render() {
  if (!root || !data) return;
  renderOutputs();
  html(q("[data-scope]"), ["line", "project"].map((s) => `<button type="button" data-scope-set="${s}" aria-pressed="${view.scope === s}">${esc(t("tr_scope_" + s))}</button>`).join(""));
  const bad = points().filter((p) => p.quality !== "good" && p.quality !== "none").length;
  const pb = q("[data-problems]");
  html(pb, `${esc(t("wg_problems"))} <span class="n${bad ? " err" : ""}">${bad}</span>`);
  pb.setAttribute("aria-pressed", String(view.problems));
  if (projectId()) q("[data-export]").href = apiHref(`projects/${projectId()}/datapoints/csv`);
  const tb = q("[data-tpl-btn]");
  tb.hidden = !data.templates.length;
  text(tb, t("wg_templates", data.templates.length));

  const rows = shown();
  if (view.active != null && !byId(view.active)) view.active = null;
  list(q("[data-list]"), rows, (p) => p.id, row);
  const empty = q("[data-empty]");
  empty.hidden = rows.length > 0;
  html(empty, points().length ? esc(t("meters_none_match")) : `${esc(t("wg_none"))} <a href="#/zaehler">${esc(t("wg_to_meters"))} →</a>`);
  renderDetail();
  renderSelbar();
}

function row(p) {
  const sel = view.selected.has(p.id);
  const outs = (data.outputs || [])
    .map((o) => {
      const x = p.outputs[o.key];
      return x?.enabled ? `<span class="chip${o.on ? " ok" : ""}" title="${esc(x.address || "")}">${esc(o.label)}</span>` : "";
    })
    .join(" ");
  return `<div class="row${p.id === view.active ? " active" : ""}${p.active ? "" : " off"}" role="option" id="dp-${p.id}" aria-selected="${sel}">
    <span class="box" data-box aria-hidden="true">${sel ? "✓" : ""}</span>
    <span class="mono sub">${p.no}</span>
    <span class="name">${esc(p.name)}${p.name.includes(p.label) ? "" : `<span class="sub"> · ${esc(p.label)}</span>`}</span>
    <span class="mono">${p.last_value != null ? `${esc(p.last_value)} ${esc(p.unit)}` : "–"}</span>
    <span><span class="chip ${QUALITY[p.quality] || ""}" title="${esc(qualityTitle(p))}">${esc(t("q_" + p.quality))}</span></span>
    <span>${outs}</span>
    <span class="sub">${p.active ? esc(t("send_" + p.send)) : esc(t("wg_inactive"))}</span>
  </div>`;
}

function renderDetail() {
  const box = q("[data-detail]");
  if (view.preview) return;
  const typing = box.contains(document.activeElement) && document.activeElement.matches("input:not([type=checkbox]), select, textarea");
  const key = view.output ? "out:" + view.output : "dp:" + view.active;
  if (typing && box.dataset.shown === key) return;
  box.dataset.shown = key;
  box.classList.toggle("open", !!(view.output || view.active));
  if (view.output) return html(box, outputForm(view.output));
  const p = byId(view.active);
  if (!p) return html(box, `<p class="sub">${esc(t("wg_pick"))}</p>`);
  html(
    box,
    `<button type="button" class="quiet back" data-back>← ${esc(t("tab_transfer"))}</button>
    <header class="dhead"><div><b>${esc(p.name)}</b><div class="sub">${esc(`Nr ${p.no} · ${p.meter} · ${p.label}`)}</div></div></header>
    <p class="chip ${QUALITY[p.quality] || ""} wide">${esc(t("q_" + p.quality))}${qualityTitle(p) ? ` – ${esc(qualityTitle(p))}` : ""}</p>
    <div class="fields">
      <label><span>${esc(t("meter_name"))}</span><input data-dp-field="name" value="${esc(p.name)}" data-saved="${esc(p.name)}"></label>
      <label><span>${esc(t("wg_factor"))}</span><input data-dp-field="factor" class="mono" value="${p.factor}" data-saved="${p.factor}"></label>
      <label><span>${esc(t("wg_shift"))}</span><input data-dp-field="shift" class="mono" value="${p.shift}" data-saved="${p.shift}"></label>
      <label><span>${esc(t("wg_col_send"))}</span><select data-dp-field="send">${SEND.map((s) => `<option value="${s}"${p.send === s ? " selected" : ""}>${esc(t("send_" + s))}</option>`).join("")}</select></label>
      <label class="check"><span>${esc(t("wg_active"))}</span><input type="checkbox" data-dp-field="active"${p.active ? " checked" : ""}></label>
      ${(data.outputs || [])
        .map((o) => {
          const x = p.outputs[o.key] || {};
          return `<label class="check"><span>${esc(o.label)}</span><input type="checkbox" data-dp-out="${o.key}"${x.enabled ? " checked" : ""}></label>
            <label><span>${esc(o.address_label)}</span><input data-dp-addr="${o.key}" class="mono" value="${esc(x.address || "")}" data-saved="${esc(x.address || "")}"></label>`;
        })
        .join("")}
    </div>
    <p class="sub">${p.last_time ? esc(`${t("n_last_reading")}: ${slotText(p.last_time)}`) : ""}</p>`
  );
}

function renderSelbar() {
  const bar = q("[data-selbar]");
  const n = view.selected.size;
  bar.hidden = !n;
  if (!n) {
    releaseEscape?.();
    releaseEscape = null;
    return;
  }
  releaseEscape ??= onEscape(() => {
    view.selected.clear();
    render();
  });
  const outs = data.outputs || [];
  html(
    bar,
    `<b>${esc(t("n_selected", n))}</b>
    <button type="button" class="tonal" data-bulk="on">${esc(t("dp_on"))}</button>
    <button type="button" class="tonal" data-bulk="off">${esc(t("dp_off"))}</button>
    <select data-bulk-send><option value="">${esc(t("wg_send_set"))}</option>${SEND.map((s) => `<option value="send:${s}">${esc(t("send_" + s))}</option>`).join("")}</select>
    ${outs.map((o) => `<button type="button" class="quiet" data-bulk="out:${o.key}:on">${esc(t("wg_out_on", o.label))}</button><button type="button" class="quiet" data-bulk="out:${o.key}:off">${esc(t("wg_out_off", o.label))}</button>`).join("")}
    <button type="button" class="quiet danger-text" data-bulk-del>${esc(t("n_delete_n", n))}</button>
    <span class="grow"></span><span class="sub">${esc(t("n_esc_clear"))}</span>`
  );
}

// ------------------------------------------------------------ Aktionen

/// Datenpunkt ändern: ganzer Datenpunkt mit der Änderung (so erwartet es die API)
async function saveDp(p, change) {
  try {
    await request(`datapoints/${p.id}`, { method: "PUT", body: { ...p, ...change } });
    await load();
    return true;
  } catch (e) {
    report(e);
    return false;
  }
}

async function bulk(action) {
  try {
    await request("datapoints/bulk", { method: "POST", body: { ids: [...view.selected], action } });
    await load();
    loadTree().catch(() => {});
  } catch (e) {
    report(e);
  }
}

async function readdress(key) {
  view.preview = true;
  view.output = null;
  view.active = null;
  const box = q("[data-detail]");
  box.classList.add("open");
  const o = data.outputs.find((x) => x.key === key);
  try {
    await previewApply(box, {
      title: t("out_readdress", o.address_label),
      run: (apply) => request(`projects/${projectId()}/outputs/${key}/readdress`, { method: "POST", body: { apply } }),
      done: async (cs) => {
        view.preview = false;
        if (cs) toast(t("out_readdressed"));
        await load();
      },
    });
  } catch (e) {
    view.preview = false;
    report(e);
  }
}

async function importCsv(file) {
  view.preview = true;
  view.output = null;
  view.active = null;
  const box = q("[data-detail]");
  box.classList.add("open");
  try {
    const rows = parseTable(await file.text());
    await previewApply(box, {
      title: t("dp_csv_import"),
      run: (apply) => request(`projects/${projectId()}/datapoints/import`, { method: "POST", body: { rows, apply } }),
      done: async (cs) => {
        view.preview = false;
        if (cs) toast(t("dp_import_done"));
        await load();
      },
    });
  } catch (e) {
    view.preview = false;
    report(e);
  }
}

function select(id, focus = false) {
  view.active = id;
  view.output = null;
  view.preview = false;
  render();
  const el = root.querySelector(`#dp-${id}`);
  el?.scrollIntoView({ block: "nearest" });
  if (el) q("[data-list]").setAttribute("aria-activedescendant", el.id);
  if (focus) q("[data-list]").focus();
}

function toggle(id) {
  if (view.selected.has(id)) view.selected.delete(id);
  else view.selected.add(id);
  render();
}

// ------------------------------------------------------------ Bedienung

function wire() {
  q("[data-search]").addEventListener("input", (ev) => {
    view.search = ev.target.value;
    render();
  });
  root.addEventListener("click", async (ev) => {
    const b = (sel) => ev.target.closest(sel);
    if (b("[data-scope-set]")) {
      view.scope = b("[data-scope-set]").dataset.scopeSet;
      storageSet("wg_scope", view.scope);
      return load();
    }
    if (b("[data-problems]")) {
      view.problems = !view.problems;
      return render();
    }
    if (b("[data-out-settings]")) return openOutput(b("[data-out-settings]").dataset.outSettings);

    if (b("[data-readdress]")) return readdress(b("[data-readdress]").dataset.readdress);
    if (b("[data-back]")) {
      view.output = null;
      view.active = null;
      return render();
    }
    if (b("[data-bulk]")) return bulk(b("[data-bulk]").dataset.bulk);
    if (b("[data-bulk-del]")) {
      if (await ask(b("[data-bulk-del]"), t("wg_delete_q", view.selected.size), t("delete"))) {
        await bulk("delete");
        view.selected.clear();
        render();
      }
      return;
    }
    if (b("[data-tpl-del]")) {
      const tpl = data.templates.find((x) => String(x.id) === b("[data-tpl-del]").dataset.tplDel);
      if (await ask(b("[data-tpl-del]"), t("wg_tpl_del_q", label(tpl?.medium || "")), t("delete"))) {
        await request(`templates/${tpl.id}`, { method: "DELETE" }).catch(report);
        await load();
        fillTemplates();
      }
      return;
    }
    const r = b("[data-list] [role=option]");
    if (!r) return;
    const id = Number(r.dataset.key);
    if (b("[data-box]") || ev.ctrlKey || ev.metaKey) return toggle(id);
    select(id);
  });
  root.addEventListener("change", async (ev) => {
    const el = ev.target;
    if (el.matches("[data-out-toggle]")) return toggleOutput(el);
    if (el.matches("[data-import]") && el.files[0]) {
      const f = el.files[0];
      el.value = "";
      return importCsv(f);
    }
    if (el.matches("[data-bulk-send]") && el.value) return bulk(el.value);
    const p = byId(view.active);
    if (!p) return;
    if (el.matches("[data-dp-field]")) {
      const f = el.dataset.dpField;
      const v = el.type === "checkbox" ? el.checked : f === "factor" || f === "shift" ? Number(el.value.replace(",", ".")) : el.value.trim();
      if (f === "factor" || f === "shift") if (!Number.isFinite(v)) return toast(t("cs_errors_note"), { error: true });
      if (await saveDp(p, { [f]: v })) flash(el);
    }
    if (el.matches("[data-dp-out]")) {
      const k = el.dataset.dpOut;
      await saveDp(p, { outputs: { ...p.outputs, [k]: { ...(p.outputs[k] || {}), enabled: el.checked } } });
    }
    if (el.matches("[data-dp-addr]")) {
      const k = el.dataset.dpAddr;
      if (await saveDp(p, { outputs: { ...p.outputs, [k]: { ...(p.outputs[k] || {}), address: el.value.trim() || null } } })) flash(el);
    }
  });
  menu(q("[data-tpl-btn]"), q("[data-tpl-menu]"), { onOpen: fillTemplates });
  root.addEventListener("submit", (ev) => {
    ev.preventDefault();
    saveOutput(ev.target);
  });
  root.addEventListener("keydown", (ev) => {
    const el = ev.target;
    if (el.matches("[data-dp-field], [data-dp-addr]") && ev.key === "Escape" && el.dataset.saved !== undefined) {
      el.value = el.dataset.saved;
      el.blur();
      ev.stopPropagation();
    }
    if (el.matches("[data-dp-field], [data-dp-addr]") && ev.key === "Enter") el.blur();
  });
  q("[data-list]").addEventListener("keydown", (ev) => {
    const rows = shown();
    const i = rows.findIndex((p) => p.id === view.active);
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      const next = rows[Math.max(0, Math.min(rows.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))];
      if (next) select(next.id, true);
    } else if (ev.key === " " && view.active != null) {
      ev.preventDefault();
      toggle(view.active);
    } else if (ev.key.toLowerCase() === "a" && (ev.ctrlKey || ev.metaKey)) {
      ev.preventDefault();
      for (const p of rows) view.selected.add(p.id);
      render();
    }
  });
}

function flash(el) {
  el.dataset.saved = el.value;
  el.classList.add("saved");
  setTimeout(() => el.classList.remove("saved"), 900);
}

function fillTemplates() {
  html(
    q("[data-tpl-menu]"),
    data.templates
      .map((x) => `<div class="tpl"><span>${esc(label(x.medium))}<span class="sub"> · ${esc(t("dp_tpl_n", x.keys.length))}${x.auto ? ` · ${esc(t("dp_tpl_auto_short"))}` : ""}</span></span><button type="button" class="quiet danger-text" data-tpl-del="${x.id}">${esc(t("delete"))}</button></div>`)
      .join("") || `<p class="sub">–</p>`
  );
}
