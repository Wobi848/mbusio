// Bereich Weitergabe: Ausgänge (MQTT, Modbus) als Zeilen mit Schalter und Zustand, Formular mit
// den Einstellungen. Reine Bausteine; geschaltet und gespeichert wird in weitergabe.js.

import { outputWord } from "./view.js";

/// Zustand eines Ausgangs aus /api/status: [farbe, wort, satz]
function outputState(s) {
  const [cls, word] = outputWord(s?.state);
  const info =
    !s || s.state === "off"
      ? ""
      : s.state === "connected"
        ? t("wg_mqtt_info", s.broker, s.sent, s.queued)
        : s.state === "listening"
          ? [t("wg_modbus_info", s.listen, s.clients), ...Object.entries(s.peers || {}).map(([ip, p]) => t("wg_modbus_peer", ip, slotText(p.last)))].join(" · ")
          : s.state === "connecting"
            ? s.broker || ""
            : s.error || "";
  return [cls, word, info];
}

/// Zeilen der Ausgänge: Schalter, Zustand, Einstellungen, Info, Listen für den Integrator.
/// `ctx`: { status, active (offenes Formular), hasAddress(key), projectId }
export function outputsHtml(outs, { status, active, hasAddress, projectId }) {
  // eine Zeile: Überschrift, je Ausgang eine Karte, Protokoll am Ende
  return (
    `<span class="eyebrow">${esc(t("wg_outputs"))}</span>` +
    outs
        .map((o) => {
          const [cls, word, info] = outputState(status?.[o.key]);
          const on = !!o.on;
          const has = hasAddress(o.key);
          // Einstellungen gleich beim Zustand: gehört zu diesem Ausgang, nicht an den Rand
          return `<div class="out${active === o.key ? " active" : ""}">
          <label class="out-switch"><input type="checkbox" class="switch" data-out-toggle="${o.key}"${on ? " checked" : ""}><b>${esc(o.label)}</b></label>
          <span class="chip ${cls}">${esc(word)}</span>
          <button type="button" class="quiet" data-out-settings="${o.key}">${esc(t("wg_settings"))}</button>
          <span class="sub cut" title="${esc(info)}">${esc(info)}</span>
          ${has && projectId ? `<span class="out-more"><button type="button" class="quiet" data-readdress="${o.key}">${esc(t("out_readdress", o.address_label))}</button>
            <a class="quiet" href="${apiHref(`projects/${projectId}/outputs/${o.key}/list?format=csv`)}" title="${esc(t("out_list", o.address_label))}">CSV</a>
            <a class="quiet" href="${apiHref(`projects/${projectId}/outputs/${o.key}/list?format=json`)}" title="${esc(t("out_list", o.address_label))}">JSON</a></span>` : ""}
        </div>`;
        })
        .join("") +
    `<button type="button" class="quiet outlog" data-open-log title="${esc(t("wg_log_title"))}">${esc(t("wg_log"))} →</button>`
  );
}

/// Felder eines Ausgangs: [name, beschriftung, art, extra]
function outputFields(key, cfg) {
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

/// Formular mit den Einstellungen eines Ausgangs
export function outputFormHtml(key, cfg, label_) {
  const c = key === "mqtt" ? cfg.mqtt.config : cfg.modbus.config;
  return `<button type="button" class="quiet back" data-back>← ${esc(t("tab_transfer"))}</button>
    <header class="dhead"><b>${esc(label_)} · ${esc(t("wg_settings"))}</b></header>
    <form class="fields" data-out-form="${key}">${outputFields(key, cfg).map((f) => fieldHtml(f, c[f[0]])).join("")}
      <div class="actions"><button type="submit">${esc(t("wg_save"))}</button><button type="button" class="quiet" data-back>${esc(t("cancel"))}</button></div>
    </form>`;
}

/// Werte aus dem Formular, nach Art des Felds (Häkchen, Zahl, Text)
export function formValues(key, cfg, form) {
  const values = {};
  for (const [name, , kind] of outputFields(key, cfg)) {
    const el = form.elements[name];
    values[name] = kind === "check" ? el.checked : kind === "number" ? Number(el.value) || 0 : el.value;
  }
  return values;
}
