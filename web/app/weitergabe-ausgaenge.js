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

/// Ziel der automatischen Sicherung, auch für die Karte und den Einschalt-Check
const backupTarget = (c) => (c.target === "ftp" ? c.host : c.folder);

/// Zustand der automatischen Sicherung aus /api/backup/auto: [farbe, wort, satz]
function backupState(b) {
  const c = b.config;
  if (!c.enabled) return ["", t("wg_m_off"), ""];
  if (!backupTarget(c)) return ["warn", t("wg_m_unset"), ""];
  if (b.state.error) return ["err", t("wg_m_failed"), `${slotText(b.state.time)}: ${b.state.error}`];
  return ["ok", t("wg_m_ready"), b.state.time ? t("wg_bk_info", backupTarget(c), slotText(b.state.time), sizeText(b.state.bytes)) : backupTarget(c)];
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
export function outputsHtml(outs, { status, active, mail, backup }) {
  // eine Zeile: Überschrift, je Ausgang eine Karte, Protokoll am Ende
  return (
    `<span class="eyebrow mark">${esc(t("wg_outputs"))}</span>` +
    outs
        .map((o) => card(o.key, o.label, !!o.on, outputState(status?.[o.key]), active))
        .join("") +
    // ohne Server ist die E-Mail nicht ein, auch wenn sie eingeschaltet gespeichert ist (Vorgabe)
    (mail ? card("email", t("wg_email"), mail.config.enabled && !!mail.config.host, emailState(mail), active) : "") +
    (backup ? card("backup", t("wg_bk_title"), backup.config.enabled && !!backupTarget(backup.config), backupState(backup), active) : "") +
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
  if (key === "backup") {
    const b = cfg.backup;
    const folder = { when: "folder", hide: b.config.target !== "folder" };
    const ftp = { when: "ftp", hide: b.config.target !== "ftp" };
    return [
      ["target", t("wg_bk_target"), "select", { options: [["folder", t("wg_bk_target_folder")], ["ftp", t("wg_bk_target_ftp")]] }],
      ["folder", t("wg_bk_folder"), "text", { ph: "/mnt/sicherung", mono: true, hint: t("wg_bk_folder_hint"), ...folder }],
      ["host", t("wg_bk_host"), "text", { ph: "ftp.example.org", mono: true, ...ftp }],
      ["port", t("wg_bk_port"), "number", { mono: true, ...ftp }],
      ["user", t("wg_bk_user"), "text", ftp],
      ["password", t("wg_bk_password"), "password", { ph: b.has_password ? t("mqtt_pass_saved") : "", ...ftp }],
      ["dir", t("wg_bk_dir"), "text", { ph: "/sicherung", mono: true, ...ftp }],
      ["tls", t("wg_bk_tls"), "check", ftp],
      ["insecure", t("wg_m_insecure"), "check", ftp],
      ["time", t("wg_bk_time"), "text", { ph: "02:30", mono: true }],
      ["keep", t("wg_bk_keep"), "number", { mono: true, hint: t("wg_bk_keep_hint") }],
      ["secret", t("wg_bk_secret"), "password", { ph: b.has_secret ? t("mqtt_pass_saved") : "", hint: t("wg_bk_secret_hint") }],
    ];
  }
  return [
    ["bind", t("modbus_bind"), "text", { ph: "0.0.0.0", mono: true, hint: t("modbus_bind_hint") }],
    ["port", t("modbus_port"), "number", { mono: true, hint: t("modbus_port_hint") }],
    ["project", t("modbus_project"), "select", { options: [[0, t("modbus_project_active")], ...cfg.modbus.projects.map((p) => [p.id, p.name])] }],
    ["swap_words", t("modbus_words"), "select", { options: [[false, t("modbus_words_abcd")], [true, t("modbus_words_cdab")]] }],
  ];
}

/// Ziel-Auswahl der Sicherung geändert: nur die dazu passenden Felder zeigen
export function backupTargetChanged(select) {
  for (const el of select.form.querySelectorAll("[data-when]")) el.classList.toggle("hide", el.dataset.when !== select.value);
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
  const preview = x.preview ? `<b data-subject-preview>${esc(x.preview)}</b><br>` : "";
  const field = `<label${kind === "check" ? ' class="check"' : ""}><span>${esc(label_)}</span>${input}</label>${x.hint ? `<p class="sub hint">${preview}${esc(x.hint)}</p>` : ""}`;
  // nur für Felder, die vom Ziel abhängen (Sicherung: Ordner oder FTP)
  return x.when ? `<div class="fieldwhen${x.hide ? " hide" : ""}" data-when="${x.when}">${field}</div>` : field;
}

/// Jetzt sichern, unter den Einstellungen der automatischen Sicherung
function backupNowHtml() {
  return `<h3>${esc(t("wg_bk_now"))}</h3>
    <p class="sub hint">${esc(t("wg_bk_now_hint"))}</p>
    <div class="actions"><button type="button" class="tonal" data-backup-now>${esc(t("wg_bk_now"))}</button></div>`;
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
    <div class="fields"><label><span>${esc(t("wg_m_test_to"))}</span><input type="text" class="mono" data-mail-test-to value="${esc(c.warn.to || c.from || c.username || "")}"></label></div>
    <p class="sub hint">${esc(t("wg_m_test_hint"))}</p>
    <div class="actions"><button type="button" class="tonal" data-mail-test>${esc(t("wg_m_test_send"))}</button></div>`;
}

/// Betreff wie ihn der Server füllt (`mail::fill`): bekannte Platzhalter ersetzt, unbekannte
/// bleiben stehen, damit man den Tippfehler sieht; leer = Vorgabe
export function subjectPreview(cfg, form, value) {
  const now = new Date();
  const two = (n) => String(n).padStart(2, "0");
  const known = { geraet: cfg.email.device, anzahl: "2", datum: `${two(now.getDate())}.${two(now.getMonth() + 1)}.${now.getFullYear()}` };
  const names = form === "warn" ? ["geraet", "anzahl"] : ["geraet", "datum"];
  const template = value.trim() || cfg.email[`${form}_subject_default`];
  return "→ " + template.replace(/\{(\w+)\}/g, (m, k) => (names.includes(k) ? known[k] : m));
}

/// Arten von Problemen und wählbare Abstände in Minuten (wie `warn::KINDS`, `warn::INTERVALS`)
export const WARN_KINDS = ["disturbed", "fault", "battery", "swap", "line", "mqtt", "disk", "billing", "backup"];
const WARN_INTERVALS = [0, 15, 60, 240, 1440];

function warnFields(cfg) {
  return [
    ["to", t("wg_w_to"), "text", { ph: "hv@example.org", mono: true, hint: t("wg_w_to_hint") }],
    ["subject", t("wg_w_subject"), "text", { ph: cfg.email.warn_subject_default, hint: t("wg_w_subject_hint"), preview: subjectPreview(cfg, "warn", cfg.email.config.warn.subject) }],
    ["interval_minutes", t("wg_w_interval"), "select", { options: WARN_INTERVALS.map((m) => [m, t("wg_w_every_" + m)]), hint: t("wg_w_interval_hint") }],
    ["line_minutes", t("wg_w_line"), "number", { mono: true }],
    ["mqtt_minutes", t("wg_w_mqtt"), "number", { mono: true, hint: t("wg_w_wait_hint") }],
  ];
}
const WARN_CHECKS = ["resolved", "remind"];

/// Häkchen mit Text dahinter, platzsparend in zwei Spalten
const opt = (name, label_, on) => `<label class="opt"><input type="checkbox" name="${name}"${on ? " checked" : ""}> ${esc(label_)}</label>`;

/// Warnungen (Konzept E-Mail 5): eigener Abschnitt mit eigenem Speichern; gelten auch für die
/// Meldungen in der Statusleiste
function warnHtml(cfg) {
  const w = cfg.email.config.warn;
  return `<details class="more" data-warn-section><summary>${esc(t("wg_w_title"))}</summary>
    <p class="sub hint">${esc(t("wg_w_intro"))}</p>
    <form class="fields" data-warn-form>${warnFields(cfg).map((f) => fieldHtml(f, w[f[0]])).join("")}
      <div class="opts">${WARN_CHECKS.map((k) => opt(k, t("wg_w_" + k), w[k])).join("")}</div>
      <p class="sub">${esc(t("wg_w_kinds"))}</p>
      <div class="opts">${WARN_KINDS.map((k) => opt(`kind_${k}`, t("warn_kind_" + k), !w.off.includes(k))).join("")}</div>
      <div class="actions"><button type="submit">${esc(t("wg_save"))}</button></div>
    </form></details>`;
}

/// Bericht (Konzept E-Mail 6), wie `report::FREQUENCIES` und `report::PARTS`
const REPORT_FREQUENCIES = ["off", "daily", "weekly"];
const REPORT_PARTS = ["reading", "outputs", "problems", "billing", "disk"];

function reportFields(cfg) {
  return [
    ["to", t("wg_r_to"), "text", { ph: "hv@example.org", mono: true, hint: t("wg_r_to_hint") }],
    ["subject", t("wg_w_subject"), "text", { ph: cfg.email.report_subject_default, hint: t("wg_r_subject_hint"), preview: subjectPreview(cfg, "report", cfg.email.config.report.subject) }],
    ["frequency", t("wg_r_frequency"), "select", { options: REPORT_FREQUENCIES.map((f) => [f, t("wg_r_freq_" + f)]) }],
    ["time", t("wg_r_time"), "text", { ph: "HH:MM", mono: true }],
    ["weekday", t("wg_r_weekday"), "select", { options: [0, 1, 2, 3, 4, 5, 6].map((d) => [d, t("wg_r_day_" + d)]), hint: t("wg_r_weekday_hint") }],
  ];
}

/// Bericht: eigener Abschnitt mit eigenem Speichern, unter den Warnungen
function reportHtml(cfg) {
  const r = cfg.email.config.report;
  return `<details class="more" data-report-section><summary>${esc(t("wg_r_title"))}</summary>
    <p class="sub hint">${esc(t("wg_r_intro"))}</p>
    <form class="fields" data-report-form>${reportFields(cfg).map((f) => fieldHtml(f, r[f[0]])).join("")}
      <p class="sub">${esc(t("wg_r_parts"))}</p>
      <div class="opts">${REPORT_PARTS.map((k) => opt(`part_${k}`, t("wg_r_part_" + k), !r.off.includes(k))).join("")}</div>
      <div class="actions"><button type="submit">${esc(t("wg_save"))}</button><button type="button" class="tonal" data-report-now>${esc(t("wg_r_now"))}</button></div>
      <p class="sub hint">${esc(t("wg_r_now_hint"))}</p>
    </form></details>`;
}

/// Bericht-Einstellungen aus dem Formular
export function reportValues(form) {
  const e = form.elements;
  return {
    to: e.to.value.trim(),
    subject: e.subject.value.trim(),
    frequency: e.frequency.value,
    time: e.time.value.trim(),
    weekday: Number(e.weekday.value),
    off: REPORT_PARTS.filter((k) => !e[`part_${k}`].checked),
  };
}

/// Warn-Einstellungen aus dem Formular
export function warnValues(cfg, form) {
  const values = {};
  for (const [name, , kind] of warnFields(cfg)) values[name] = kind === "text" ? form.elements[name].value.trim() : Number(form.elements[name].value) || 0;
  for (const name of WARN_CHECKS) values[name] = form.elements[name].checked;
  values.off = WARN_KINDS.filter((k) => !form.elements[`kind_${k}`].checked);
  return values;
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
    ${key === "email" ? testMailHtml(cfg) + warnHtml(cfg) + reportHtml(cfg) : key === "backup" ? backupNowHtml() : integratorHtml(o, projectId, has)}`;
}

/// Werte aus dem Formular, nach Art des Felds (Häkchen, Zahl, Text)
export function formValues(key, cfg, form) {
  const values = {};
  for (const [name, , kind] of outputFields(key, cfg)) {
    const el = form.elements[name];
    values[name] = kind === "check" ? el.checked : kind === "number" ? Number(el.value) || 0 : el.value;
  }
  // Modbus: die Auswahlfelder liefern Text
  if (key === "modbus") Object.assign(values, { project: Number(values.project), swap_words: values.swap_words === "true" });
  return values;
}
