// Bereich Weitergabe: Ausgänge (MQTT, Modbus, E-Mail) als Karten mit Schalter und Zustand, Formular
// mit den Einstellungen. Reine Bausteine; geschaltet und gespeichert wird in weitergabe.js.

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

/// Zustand der E-Mail aus /api/mail: [farbe, wort, satz]
function emailState(m) {
  const c = m.config;
  if (!c.enabled) return ["", t("wg_m_off"), ""];
  if (!c.host) return ["warn", t("wg_m_unset"), ""];
  if (m.last?.error) return ["err", t("wg_m_failed"), `${slotText(m.last.time)}: ${m.last.error}`];
  return ["ok", t("wg_m_ready"), m.last ? t("wg_m_info", c.host, slotText(m.last.time)) : c.host];
}

function card(key, label, on, [cls, word, info], active) {
  // Einstellungen gleich beim Zustand: gehört zu diesem Ausgang, nicht an den Rand
  return `<div class="out${active === key ? " active" : ""}">
    <label class="out-switch"><input type="checkbox" class="switch" data-out-toggle="${key}"${on ? " checked" : ""}><b>${esc(label)}</b></label>
    <span class="chip ${cls}">${esc(word)}</span>
    <button type="button" class="quiet" data-out-settings="${key}">${esc(t("wg_settings"))}</button>
    <span class="sub cut" title="${esc(info)}">${esc(info)}</span>
  </div>`;
}

/// Karten der Ausgänge: Schalter, Name, Zustand, Einstellungen, Info. Alles Weitere (Listen für den
/// Integrator, Adressen nach Muster, Testmail) steht in den Einstellungen, damit die Leiste ruhig
/// bleibt. `ctx`: { status, active (offenes Formular), mail (/api/mail) }
export function outputsHtml(outs, { status, active, mail }) {
  // eine Zeile: Überschrift, je Ausgang eine Karte, Protokoll am Ende
  return (
    `<span class="eyebrow mark">${esc(t("wg_outputs"))}</span>` +
    outs
        .map((o) => card(o.key, o.label, !!o.on, outputState(status?.[o.key]), active))
        .join("") +
    (mail ? card("email", t("wg_email"), mail.config.enabled, emailState(mail), active) : "") +
    // Ansichten statt der Datenpunkte, zusammen am Ende
    `<span class="outlog"><button type="button" class="quiet" data-open-billing title="${esc(t("ab_hint"))}">${esc(t("ab_open"))} →</button>` +
    `<button type="button" class="quiet" data-open-log title="${esc(t("wg_log_title"))}">${esc(t("wg_log"))} →</button></span>`
  );
}

/// Verschlüsselung der E-Mail und ihr üblicher Port
const SECURITY = { starttls: 587, tls: 465, none: 25 };

/// Andere Verschlüsselung: üblichen Port eintragen, ausser es steht ein eigener drin
export function securityChanged(select) {
  const port = select.form.elements.port;
  if (Object.values(SECURITY).includes(Number(port.value))) port.value = SECURITY[select.value];
}

/// Felder eines Ausgangs: [name, beschriftung, art, extra]
function outputFields(key, cfg) {
  if (key === "email")
    return [
      ["host", t("wg_m_host"), "text", { ph: "smtp.example.org", mono: true }],
      ["port", t("wg_m_port"), "number", { mono: true }],
      ["security", t("wg_m_security"), "select", { options: Object.keys(SECURITY).map((v) => [v, t("wg_m_sec_" + v)]) }],
      ["insecure", t("wg_m_insecure"), "check"],
      ["username", t("wg_m_user"), "text"],
      ["password", t("wg_m_password"), "password", { ph: cfg.email.has_password ? t("mqtt_pass_saved") : "" }],
      ["from", t("wg_m_from"), "text", { ph: cfg.email.config.username || "mbusio@example.org", mono: true, hint: t("wg_m_from_hint") }],
      ["device", t("wg_m_device"), "text", { ph: cfg.email.device, hint: t("wg_m_device_hint") }],
      ["retry_days", t("wg_m_retry"), "number", { mono: true, hint: t("wg_m_retry_hint") }],
    ];
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

/// Für den Integrator, unter den Einstellungen: Adressen nach Muster, Liste als CSV/JSON
function integratorHtml(o, projectId, has) {
  if (!o || !projectId || !has) return "";
  const list = (f) => apiHref(`projects/${projectId}/outputs/${o.key}/list?format=${f}`);
  return `<h3>${esc(t("wg_integrator"))}</h3>
    <div class="actions">
      <button type="button" class="tonal" data-readdress="${o.key}">${esc(t("out_readdress", o.address_label))}</button>
      <a class="quiet" href="${list("csv")}" title="${esc(t("out_list", o.address_label))}">${esc(t("wg_list"))} CSV</a>
      <a class="quiet" href="${list("json")}" title="${esc(t("out_list", o.address_label))}">JSON</a>
    </div>`;
}

/// Testmail mit den gespeicherten Einstellungen; vorbelegt mit dem Absender
function testMailHtml(cfg) {
  const c = cfg.email.config;
  return `<h3>${esc(t("wg_m_test"))}</h3>
    <div class="fields"><label><span>${esc(t("wg_m_test_to"))}</span><input type="text" class="mono" data-mail-test-to value="${esc(c.from || c.username || "")}"></label></div>
    <p class="sub hint">${esc(t("wg_m_test_hint"))}</p>
    <div class="actions"><button type="button" class="tonal" data-mail-test>${esc(t("wg_m_test_send"))}</button></div>`;
}

/// Formular mit den Einstellungen eines Ausgangs; `o`, `projectId`, `has` (Datenpunkte mit
/// Adresse an diesem Ausgang) für die Teile für den Integrator
export function outputFormHtml(key, cfg, label_, o, projectId, has) {
  const c = cfg[key].config;
  return `<button type="button" class="quiet back" data-back>← ${esc(t("tab_transfer"))}</button>
    <header class="dhead"><b>${esc(label_)} · ${esc(t("wg_settings"))}</b></header>
    <form class="fields" data-out-form="${key}">${outputFields(key, cfg).map((f) => fieldHtml(f, c[f[0]])).join("")}
      <div class="actions"><button type="submit">${esc(t("wg_save"))}</button><button type="button" class="quiet" data-back>${esc(t("cancel"))}</button></div>
    </form>
    ${key === "email" ? testMailHtml(cfg) : integratorHtml(o, projectId, has)}`;
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
