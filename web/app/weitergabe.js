// Bereich Weitergabe: oben die Ausgänge (MQTT, Modbus) mit Schalter und Zustand, darunter die
// Datenpunkte als Liste, Details rechts (Konzept neue Oberfläche §3.2, Schritt 4).

import { state, watch } from "./store.js";
import { html, text, list, ask, toast, styles, previewApply, menu, explain, fieldKeys } from "./view.js";
import { onEscape, selectionKeys } from "./keys.js";
import { loadTree, report } from "./data.js";
import "./weitergabe-texte.js";
import { outputsHtml, outputFormHtml, formValues, securityChanged, backupTargetChanged, warnValues, reportValues, subjectPreview } from "./weitergabe-ausgaenge.js";
import { openLog, closeLog } from "./weitergabe-protokoll.js";
import { openBilling, closeBilling } from "./weitergabe-abrechnung.js";


const SEND = ["every", "change", "daily"];
const QUALITY = { good: "ok", stale: "warn", disturbed: "err", invalid: "err", none: "" };

let root = null;
let data = null; // /api/datapoints: { meters (mit datapoints), templates, outputs }
let cfg = { mqtt: null, modbus: null, email: null, backup: null }; // Einstellungen der Ausgänge
const view = { scope: storageGet("wg_scope") || "line", search: "", problems: false, active: null, output: null, selected: new Set(), preview: false, enableOnSave: false };
let releaseEscape = null;
/// `cfg`, aus dem das gezeigte Formular eines Ausgangs gebaut ist (`load` legt ein neues an)
let shownCfg = null;
const q = (sel) => root.querySelector(sel);
/// Erklärung einer Qualität (nur wo es etwas zu erklären gibt)
const qualityTitle = (p) => (["stale", "disturbed"].includes(p.quality) ? t(`q_${p.quality}_title`) : p.quality === "invalid" ? t(p.invalid === "status" ? "q_status_title" : p.invalid === "error_value" ? "q_error_value_title" : "q_decreased_title") : "");

export function mount(pane) {
  styles("weitergabe");
  root = pane;
  root.innerHTML = `
    <div class="outputs" data-outputs></div>
    <section class="logview" data-logview hidden></section>
    <section class="billview" data-billview hidden></section>
    <div class="toolbar">
      <input type="search" data-search placeholder="${esc(t("n_search"))}" aria-label="${esc(t("n_search"))}">
      <div class="seg" role="group" data-scope></div>
      <button type="button" class="seg-like" data-problems></button>
      <span class="grow"></span>
      <div class="menu-host"><button type="button" class="quiet" data-tpl-btn hidden></button><div class="menu" data-tpl-menu hidden></div></div>
      <span class="sub">${esc(t("wg_points_csv"))}</span>
      <a class="quiet" data-export>${esc(t("wg_export"))}</a>
      <label class="quiet filebtn">${esc(t("wg_import"))}<input type="file" accept=".csv,text/csv" data-import hidden></label>
      ${explain(t("wg_explain"))}
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
    const [d, mq, mb, em, bk] = await Promise.all([request(`datapoints?scope=${view.scope}`), request("mqtt"), request("modbus"), request("mail"), request("backup/auto")]);
    data = d;
    cfg = { mqtt: mq, modbus: mb, email: em, backup: bk };
    const ids = new Set(points().map((p) => p.id));
    for (const id of [...view.selected]) if (!ids.has(id)) view.selected.delete(id);
  } catch (e) {
    report(e);
  }
  render();
  fromAddress();
}

/// Feste Adressen: #/transfer/email öffnet die Einstellungen der E-Mail,
/// #/transfer/billing?project=<id> die Abrechnung (z.B. aus der Anlage)
function fromAddress() {
  const m = data && location.hash.match(/^#\/transfer\/(email|backup|billing)(?:\?project=(\d+))?/);
  if (!m) return;
  history.replaceState(null, "", "#/transfer");
  if (m[1] === "email" || m[1] === "backup") return openOutput(m[1]);
  showBilling(m[2] ? Number(m[2]) : null);
}
addEventListener("hashchange", fromAddress);

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

async function toggleOutput(input) {
  const key = input.dataset.outToggle;
  const on = input.checked;
  input.checked = !on;
  const label = key === "email" ? t("wg_email") : key === "backup" ? t("wg_bk_title") : data.outputs.find((o) => o.key === key)?.label || key;
  const backupUnset = key === "backup" && !(cfg.backup.config.target === "ftp" ? cfg.backup.config.host : cfg.backup.config.folder);
  if (on && ((key === "mqtt" && !cfg.mqtt.config.host) || (key === "email" && !cfg.email.config.host) || backupUnset)) {
    toast(t(key === "email" ? "wg_m_need_host" : key === "backup" ? "wg_bk_need_target" : "wg_need_host"));
    // man hat „ein“ gedrückt: nach dem Speichern ist der Ausgang ein
    return openOutput(key, true);
  }
  if (!on && !(await ask(input, t("wg_off_q", label), t("n_switch_off")))) return;
  try {
    await putOutput(key, { enabled: on });
    await load();
  } catch (e) {
    report(e);
  }
}

/// Einstellungen eines Ausgangs zum Server, ergänzt um die gespeicherten; leeres Passwort = gespeichertes behalten
function putOutput(key, changes) {
  const { password, secret, ...rest } = changes;
  if (key === "mqtt") return request("mqtt", { method: "PUT", body: { config: { ...cfg.mqtt.config, ...rest }, password: password || null } });
  if (key === "email") return request("mail", { method: "PUT", body: { config: { ...cfg.email.config, ...rest }, password: password || undefined } });
  if (key === "backup") return request("backup/auto", { method: "PUT", body: { config: { ...cfg.backup.config, ...rest }, password: password || null, secret: secret || null } });
  return request("modbus", { method: "PUT", body: { ...cfg.modbus.config, ...rest } });
}

/// Abrechnung und Protokoll schliessen: es ist immer nur eine Ansicht offen
function closeViews() {
  closeBilling();
  closeLog();
}

function openOutput(key, enable = false) {
  // Einstellungen eines Ausgangs ersetzen Abrechnung und Protokoll
  closeViews();
  view.output = key;
  view.enableOnSave = enable;
  view.active = null;
  view.preview = false;
  render();
}

async function saveOutput(form) {
  const key = form.dataset.outForm;
  const values = { ...formValues(key, cfg, form), ...(view.enableOnSave ? { enabled: true } : {}) };
  try {
    await putOutput(key, values);
    toast(t(key === "mqtt" ? "mqtt_saved" : key === "email" ? "wg_m_saved" : key === "backup" ? "wg_bk_saved" : "modbus_saved"));
    view.output = null;
    await load();
  } catch (e) {
    report(e);
  }
}

/// Abschnitt Warnungen oder Bericht speichern; das Formular des Servers bleibt offen
async function saveMailPart(part, values, saved) {
  try {
    await putOutput("email", { [part]: values });
    toast(t(saved));
    cfg.email = await request("mail");
  } catch (e) {
    report(e);
  }
}

/// Bericht jetzt mit den gespeicherten Einstellungen; Ergebnis wie bei der Testmail
async function reportNow(button) {
  button.disabled = true;
  try {
    const r = await request("mail/report", { method: "POST" });
    toast(t("wg_r_now_ok", r.to));
  } catch (e) {
    report(e);
  } finally {
    button.disabled = false;
    cfg.email = await request("mail").catch(() => cfg.email);
    renderCards();
  }
}

/// Testmail an die Adresse im Feld, mit den gespeicherten Einstellungen; die Karte zeigt danach
/// auch das Ergebnis
async function testMail(button) {
  const to = q("[data-mail-test-to]").value.trim();
  button.disabled = true;
  try {
    await request("mail/test", { method: "POST", body: { to } });
    toast(t("wg_m_test_ok", to));
  } catch (e) {
    report(e);
  } finally {
    button.disabled = false;
    cfg.email = await request("mail").catch(() => cfg.email);
    render();
  }
}

/// Jetzt sichern mit den gespeicherten Einstellungen; die Karte zeigt danach auch das Ergebnis
async function backupNow(button) {
  button.disabled = true;
  try {
    const r = await request("backup/auto/test", { method: "POST" });
    toast(t("wg_bk_now_ok", sizeText(r.bytes)));
  } catch (e) {
    report(e);
  } finally {
    button.disabled = false;
    cfg.backup = await request("backup/auto").catch(() => cfg.backup);
    render();
  }
}

// ------------------------------------------------------------ Datenpunkte

/// Karten der Ausgänge (Zustand, zuletzt gesendet)
function renderCards() {
  html(
    q("[data-outputs]"),
    outputsHtml(data.outputs || [], { status: state.status, active: view.output, mail: cfg.email, backup: cfg.backup })
  );
}

function render() {
  if (!root || !data) return;
  renderCards();
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
  html(empty, points().length ? esc(t("meters_none_match")) : `${esc(t("wg_none"))} <a href="#/meters">${esc(t("wg_to_meters"))} →</a>`);
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
    <span class="name"><span class="cut">${esc(p.name)}${p.name.includes(p.label) ? "" : `<span class="sub"> · ${esc(p.label)}</span>`}</span></span>
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
  // Einstellungen eines Ausgangs hängen nur an `cfg`: nicht bei jeder Statusabfrage neu bauen,
  // sonst klappen offene Abschnitte (Warnungen, Bericht) alle paar Sekunden zu
  if (view.output && box.dataset.shown === key && shownCfg === cfg) return;
  box.dataset.shown = key;
  shownCfg = cfg;
  box.classList.toggle("open", !!(view.output || view.active));
  if (view.output) {
    const o = data.outputs.find((x) => x.key === view.output);
    const has = points().some((p) => p.outputs[view.output]?.address);
    const label = view.output === "email" ? t("wg_email") : view.output === "backup" ? t("wg_bk_title") : o?.label || view.output;
    return html(box, outputFormHtml(view.output, cfg, label, o, projectId(), has));
  }
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
    <p class="sub">${p.last_time ? esc(`${t("n_last_reading")}: ${slotText(p.last_time)}`) : ""}</p>
    <div class="actions"><button type="button" class="quiet" data-dp-log>${esc(t("wg_dp_log"))} →</button></div>`
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

/// Abrechnung statt der Datenpunkte zeigen (Vorgabe: aktives Projekt)
/// Abrechnung oder Protokoll statt der Datenpunkte: kein Ausgang ist dann gewählt
function leaveOutput() {
  view.output = null;
  renderCards();
}

function showBilling(project = null) {
  closeViews();
  leaveOutput();
  root.classList.add("showbill");
  openBilling(q("[data-billview]"), { project, close: () => root.classList.remove("showbill") });
}

/// Protokoll statt der Datenpunkte zeigen; `p` filtert auf einen Datenpunkt
function showLog(p = null) {
  closeViews();
  leaveOutput();
  root.classList.add("showlog");
  openLog(q("[data-logview]"), { dp: p?.id ?? null, dpName: p?.name || "", close: () => root.classList.remove("showlog") });
}

function wire() {
  root.addEventListener("input", (ev) => {
    const form = ev.target.closest("[data-warn-form], [data-report-form]");
    if (form && ev.target.name === "subject") form.querySelector("[data-subject-preview]").textContent = subjectPreview(cfg, form.matches("[data-warn-form]") ? "warn" : "report", ev.target.value);
  });
  q("[data-search]").addEventListener("input", (ev) => {
    view.search = ev.target.value;
    render();
  });
  root.addEventListener("click", async (ev) => {
    const b = (sel) => ev.target.closest(sel);
    if (b("[data-open-log]")) return showLog();
    if (b("[data-open-billing]")) return showBilling();
    if (b("[data-dp-log]")) return showLog(byId(view.active));
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
    if (b("[data-mail-test]")) return testMail(b("[data-mail-test]"));
    if (b("[data-report-now]")) return reportNow(b("[data-report-now]"));
    if (b("[data-backup-now]")) return backupNow(b("[data-backup-now]"));
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
    if (el.matches("[data-out-form=email] [name=security]")) return securityChanged(el);
    if (el.matches("[data-out-form=backup] [name=target]")) return backupTargetChanged(el);
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
    if (ev.target.matches("[data-warn-form]")) return saveMailPart("warn", warnValues(cfg, ev.target), "wg_w_saved");
    if (ev.target.matches("[data-report-form]")) return saveMailPart("report", reportValues(ev.target), "wg_r_saved");
    saveOutput(ev.target);
  });
  root.addEventListener("keydown", (ev) => fieldKeys(ev, "[data-dp-field], [data-dp-addr]"));
  q("[data-list]").addEventListener("keydown", (ev) =>
    selectionKeys(ev, { rows: shown(), view, select: (id) => select(id, true), toggle, render })
  );
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
