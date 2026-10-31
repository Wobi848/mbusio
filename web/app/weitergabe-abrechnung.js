// Bereich Weitergabe: Abrechnung (Konzept Export Teil B, Konzept E-Mail 4). Ansicht statt der
// Datenpunkte, wie das Protokoll: links die Einstellungen (Zeitraum, Form, Linien, Werte, Versand),
// rechts Vorschau, Herunterladen, Per E-Mail senden und die abgelegten Dateien. Jede Änderung wird
// gleich je Projekt gespeichert und die Vorschau neu gerechnet.

import { html, toast } from "./view.js";
import { onEscape } from "./keys.js";
import { report } from "./data.js";
import { state } from "./store.js";
import "./weitergabe-abrechnung-texte.js";

const PERIODS = ["prev_month", "prev_quarter", "prev_half", "prev_year", "custom"];
const MAX_COMPARE = 10;
const WAIT_DAYS = [0, 1, 2, 3, 5, 7, 14];

let box = null;
let projectId = null;
let data = null; // /api/projects/{id}/billing
let mail = null; // /api/mail (ohne Passwort): ist ein Server eingerichtet und an?
let releaseEscape = null;
let onClose = null;

/// Ansicht in `container` öffnen, für `project` (Vorgabe: aktives Projekt)
export async function openBilling(container, { project, close }) {
  box = container;
  onClose = close;
  projectId = project || state.status?.active?.project?.id;
  box.hidden = false;
  box.innerHTML = `<p class="sub pad">…</p>`;
  box.onclick = onClick;
  box.onchange = onChange;
  releaseEscape = onEscape(closeBilling);
  await load();
}

export function closeBilling() {
  if (!box) return;
  releaseEscape?.();
  box.onclick = null;
  box.onchange = null;
  box.hidden = true;
  box.innerHTML = "";
  box._html = null;
  box = null;
  onClose?.();
}

async function load() {
  try {
    [data, mail] = await Promise.all([request(`projects/${projectId}/billing`), request("mail")]);
    render();
  } catch (e) {
    report(e);
  }
}

/// Nacheinander speichern: jede Änderung baut auf der vorigen auf (zwei Felder schnell
/// hintereinander geändert, sonst überschreibt die zweite die erste mit dem alten Stand)
let saving = Promise.resolve();
function save(change) {
  saving = saving.then(async () => {
    try {
      data = await request(`projects/${projectId}/billing`, { method: "PUT", body: { ...data.settings, ...change } });
      render();
    } catch (e) {
      report(e);
      render(); // Feld wieder wie gespeichert
    }
  });
  return saving;
}

/// Knopf während der Arbeit sperren; Fehler als Meldung, Erfolg als kurze Bestätigung
async function busy(button, work) {
  button.disabled = true;
  try {
    toast(await work());
  } catch (e) {
    report(e);
  } finally {
    if (box) button.disabled = false;
  }
}

function onClick(ev) {
  const b = (sel) => ev.target.closest(sel);
  if (b("[data-b-close]")) return closeBilling();
  const now = b("[data-b-mail-now]");
  if (now)
    return busy(now, async () => {
      const r = await request(`projects/${projectId}/billing/mail`, { method: "POST" });
      return t("ab_mail_now_ok", r.to.join(", "));
    });
  const send = b("[data-b-send]");
  if (send)
    return busy(send, async () => {
      try {
        await request(`projects/${projectId}/billing/files/${encodeURIComponent(send.dataset.bSend)}/send`, { method: "POST" });
        return t("ab_m_sent_now");
      } finally {
        // auch beim Fehler: der Grund steht jetzt an der Datei
        await load();
      }
    });
}

function onChange(ev) {
  const el = ev.target;
  const s = data.settings;
  if (el.matches("[data-b-project]")) {
    projectId = Number(el.value);
    return load();
  }
  const simple = { period: "bPeriod", form: "bForm", step: "bStep", decimal: "bDecimal" };
  for (const [field, key] of Object.entries(simple)) if (key in el.dataset) return save({ [field]: el.value });
  if (el.matches("[data-b-from]")) return save({ from: el.value || null });
  if (el.matches("[data-b-to]")) return save({ to: el.value || null });
  if (el.matches("[data-b-main]")) return save({ main: el.checked });
  if (el.matches("[data-b-compare]")) return save({ compare_years: Number(el.value) });
  if (el.matches("[data-b-auto]")) return save({ auto: el.checked });
  if (el.matches("[data-b-wait]")) return save({ wait_days: Number(el.value) });
  if (el.matches("[data-b-mail-to]")) return save({ mail_to: el.value.trim() });
  if (el.matches("[data-b-subject]")) return save({ mail_subject: el.value.trim() });
  if (el.matches("[data-b-line]")) {
    const lines = chosenLines(el);
    if (lines) return save({ lines });
    el.checked = true; // mindestens eine Linie
    return;
  }
  if (el.matches("[data-b-key]")) {
    const keys = new Set(s.keys);
    if (el.checked) keys.add(el.dataset.bKey);
    else keys.delete(el.dataset.bKey);
    return save({ keys: [...keys] });
  }
}

/// Gewählte Linien nach einem Häkchen; alle gewählt = [] (auch künftige Linien), keine = null
function chosenLines(el) {
  const ids = data.lines.map((l) => l.id);
  const on = new Set(data.settings.lines.length ? data.settings.lines.filter((id) => ids.includes(id)) : ids);
  if (el.checked) on.add(Number(el.dataset.bLine));
  else on.delete(Number(el.dataset.bLine));
  if (!on.size) return null;
  return on.size === ids.length ? [] : [...on];
}

const opt = (value, label, current) => `<option value="${esc(value)}"${String(value) === String(current) ? " selected" : ""}>${esc(label)}</option>`;
const day = (iso) => (lang === "de" ? iso.split("-").reverse().join(".") : iso);

/// Leiste, Einstellungen und Vorschau einzeln; eine Antwort des Servers ersetzt das Feld nicht
/// samt dem Getippten
function render() {
  if (!box || !data) return;
  const s = data.settings;
  const projects = state.tree?.projects || [];
  if (!box.querySelector("[data-b-bar]"))
    box.innerHTML = `<div class="toolbar" data-b-bar></div><div class="bview"><div class="bset detail"></div><div class="bout"></div></div>`;
  html(
    box.querySelector("[data-b-bar]"),
    `<button type="button" class="quiet" data-b-close>← ${esc(t("tab_transfer"))}</button>
      <b>${esc(t("ab_title"))}</b>
      <label class="inline"><span class="sub">${esc(t("ab_project"))}</span><select data-b-project>${projects.map((p) => opt(p.id, p.name, projectId)).join("")}</select></label>
      <span class="grow"></span>
      ${data.preview ? `<a class="button" data-b-download href="${apiHref(`projects/${projectId}/billing/csv`)}" download>${esc(t("ab_download"))}</a>` : ""}
      ${mailNowButton(s)}`
  );
  keepTyping(box.querySelector(".bset"), () => html(box.querySelector(".bset"), settingsHtml(s) + sendHtml(s)));
  html(box.querySelector(".bout"), preview(data.preview) + filesHtml(s));
}

/// Neu bauen, ohne das Feld zu verlieren, in dem gerade getippt wird: Text, Fokus und Cursor
/// gehen auf das gleiche Feld im neuen Inhalt über
function keepTyping(part, rebuild) {
  const el = document.activeElement;
  const typing = part.contains(el) && el.matches("input:not([type=checkbox]), textarea");
  const attr = typing && [...el.attributes].find((a) => a.name.startsWith("data-b-"));
  const kept = attr && { sel: `[${attr.name}${attr.value ? `="${CSS.escape(attr.value)}"` : ""}]`, value: el.value, start: el.selectionStart, end: el.selectionEnd };
  rebuild();
  const again = kept && part.querySelector(kept.sel);
  if (!again || again === el) return;
  again.value = kept.value;
  again.focus();
  // Datumsfelder kennen keinen Cursor (selectionStart ist dort null)
  if (kept.start !== null) again.setSelectionRange(kept.start, kept.end);
}

/// Senden geht nur mit Empfängern und eingerichtetem, eingeschaltetem Mailserver; sonst gesperrt
/// mit dem Grund als Tooltip
function mailNowButton(s) {
  const why = !s.mail_to ? t("ab_mail_now_no_to") : !mail.config.host ? t("ab_m_no_server") : !mail.config.enabled ? t("ab_m_mail_off") : "";
  return `<button type="button" class="tonal" data-b-mail-now${why ? ` disabled title="${esc(why)}"` : ` title="${esc(t("ab_mail_now_title"))}"`}>${esc(t("ab_mail_now"))}</button>`;
}

function settingsHtml(s) {
  const decimal = s.decimal || (lang === "de" ? "," : ".");
  return `<h3>${esc(t("ab_settings"))}</h3>
    <div class="fields">
      <label><span>${esc(t("ab_period"))}</span><select data-b-period>${PERIODS.map((x) => opt(x, t("period_" + x), s.period)).join("")}</select></label>
      ${
        s.period === "custom"
          ? `<label><span>${esc(t("ab_from"))}</span><input type="date" data-b-from value="${esc(s.from || "")}"></label>
             <label><span>${esc(t("ab_to"))}</span><input type="date" data-b-to value="${esc(s.to || "")}"></label>`
          : ""
      }
      <label><span>${esc(t("ab_form"))}</span><select data-b-form>${opt("billing", t("ab_billing"), s.form)}${opt("series", t("ab_series"), s.form)}</select></label>
      ${
        s.form === "series"
          ? `<label><span>${esc(t("ab_step"))}</span><select data-b-step>${opt("month", t("ab_month"), s.step)}${opt("day", t("ab_day"), s.step)}</select></label>`
          : `<label><span>${esc(t("ab_compare"))}</span><select data-b-compare>${Array.from({ length: MAX_COMPARE + 1 }, (_, n) => opt(n, n ? t("ab_compare_n", n) : t("ab_compare_none"), s.compare_years)).join("")}</select></label>`
      }
      <label><span>${esc(t("ab_decimal"))}</span><select data-b-decimal>${opt(",", t("ab_comma"), decimal)}${opt(".", t("ab_point"), decimal)}</select></label>
    </div>
    ${linesHtml(s)}
    <h3>${esc(t("ab_values"))}</h3>
    <div class="vals">
      <label class="vrow main"><span>${esc(t("ab_main"))}</span><span class="sub">${esc(t("ab_main_hint"))}</span><input type="checkbox" data-b-main${s.main ? " checked" : ""}></label>
    </div>
    <details class="more"${s.keys.length ? " open" : ""}><summary>${esc(t("ab_more_values", data.kinds.length))}</summary>
    <div class="vals">
      ${data.kinds
        .map(
          (k) =>
            `<label class="vrow"><span>${esc(k.label)} <span class="sub">${esc(k.unit)}</span></span><span class="sub">${esc(t("ab_meters", k.meters, k.main))}</span><input type="checkbox" data-b-key="${esc(k.key)}"${
              s.keys.includes(k.key) ? " checked" : ""
            }></label>`
        )
        .join("")}
    </div></details>`;
}

/// Linien zur Auswahl, nur wenn das Projekt mehrere hat
function linesHtml(s) {
  if (data.lines.length < 2) return "";
  const on = (id) => !s.lines.length || s.lines.includes(id);
  const n = data.lines.filter((l) => on(l.id)).length;
  const what = n === data.lines.length ? t("ab_lines_all", n) : t("ab_lines_some", n, data.lines.length);
  return `<details class="more"${n < data.lines.length ? " open" : ""}><summary>${esc(t("ab_lines", what))}</summary>
    <div class="vals">${data.lines
      .map(
        (l) =>
          `<label class="vrow"><span>${esc(l.label)}</span><span class="sub">${esc(t("ab_line_meters", l.meters))}</span><input type="checkbox" data-b-line="${l.id}"${on(l.id) ? " checked" : ""}></label>`
      )
      .join("")}</div></details>`;
}

/// Versand: automatisch ablegen, Wartezeit, Empfänger, Betreff mit Vorschau; immer sichtbar
function sendHtml(s) {
  const custom = s.period === "custom";
  const hint = custom ? t("ab_auto_custom") : t("ab_auto_" + s.period);
  return `<h3>${esc(t("ab_send"))}</h3>
    <div class="vals">
      <label class="vrow"><span>${esc(t("ab_auto_label"))}</span><span class="sub">${esc(hint)}</span><input type="checkbox" data-b-auto${s.auto && !custom ? " checked" : ""}${custom ? " disabled" : ""}></label>
    </div>
    <div class="fields">
      ${
        s.auto && !custom
          ? `<label><span>${esc(t("ab_wait"))}</span><select data-b-wait>${WAIT_DAYS.map((d) => opt(d, t("ab_wait_days", d), s.wait_days)).join("")}</select></label>
             <p class="sub hint">${esc(t("ab_wait_hint"))}</p>`
          : ""
      }
      <label><span>${esc(t("ab_m_to"))}</span><input type="text" data-b-mail-to value="${esc(s.mail_to)}" placeholder="${esc(t("ab_m_to_ph"))}"></label>
      <p class="sub hint">${esc(t("ab_m_to_hint"))}</p>
      <label><span>${esc(t("ab_subject"))}</span><input type="text" data-b-subject value="${esc(s.mail_subject)}" placeholder="${esc(data.subject_default)}"></label>
      <p class="sub hint">${data.subject ? `<b>${esc(t("ab_subject_preview", data.subject))}</b><br>` : ""}${esc(t("ab_subject_hint"))}</p>
    </div>
    ${s.mail_to ? mailHint() : ""}`;
}

/// Der Mailserver gehört zum Ausgang E-Mail: hier nur sagen, wenn er fehlt oder aus ist
function mailHint() {
  const c = mail.config;
  const why = !c.host ? t("ab_m_no_server") : !c.enabled ? t("ab_m_mail_off") : "";
  return why ? `<p class="chip warn wide">${esc(why)} <a href="#/transfer/email">${esc(t("ab_m_setup"))} →</a></p>` : "";
}

/// Abgelegte Dateien mit Zustand der E-Mail; warum die neueste nicht ging, steht sichtbar darunter
function filesHtml(s) {
  if (!s.auto && !data.files.length) return "";
  const list = data.files.length
    ? `<div class="vals">${data.files
        .map(
          (f) =>
            `<div class="vrow bfile"><a data-b-file href="${apiHref(`projects/${projectId}/billing/files/${encodeURIComponent(f.name)}`)}" download title="${esc(
              t("ab_file_title", Math.max(1, Math.round(f.size / 1024)))
            )}">${esc(f.name)}</a><span class="sub">${esc(slotText(f.time))}</span>${mailState(f, !!s.mail_to)}</div>`
        )
        .join("")}</div>`
    : `<p class="sub">${esc(t("ab_files_none"))}</p>`;
  const why = data.files[0]?.mail?.error;
  return `<h3>${esc(t("ab_files"))}</h3>${list}${why ? `<p class="chip warn wide">${esc(t("ab_m_why", why))}</p>` : ""}`;
}

/// Zustand der E-Mail an einer Datei: gesendet, nicht gesendet (Grund im Tooltip), Senden
function mailState(f, canSend) {
  const m = f.mail;
  const send = canSend ? `<button type="button" class="quiet" data-b-send="${esc(f.name)}">${esc(t(m?.sent ? "ab_m_send_again" : "ab_m_send"))}</button>` : "";
  if (m?.sent) return `<span class="chip ok" title="${esc(t("ab_m_sent_title", slotText(m.sent)))}">${esc(t("ab_m_sent"))}</span>${send}`;
  if (m?.error) return `<span class="chip warn" title="${esc(m.error)}">${esc(t("ab_m_not_sent"))}</span>${send}`;
  return `<span></span>${send}`;
}

/// Spalten der Vorschau: was man prüfen will (ohne Quellen); die Datei hat alle. Abrechnung: Name,
/// Wert, Einheit, Anfang, Ende, Verbrauch, Vorjahre, Hinweis
function previewCols(header) {
  if (data.settings.form === "series") return [0, 3, 8, 9, 10, 11];
  return [2, 7, 8, 9, 11, ...Array.from({ length: header.length - 13 }, (_, i) => 13 + i)];
}

function preview(p) {
  if (!p) return `<p class="chip warn wide">${esc(data.error || "")}</p>`;
  const head = `<h3>${esc(t("ab_preview"))} <span class="sub">${esc(`${day(p.from)} – ${day(p.to)} · ${t("ab_rows", p.total)}`)}</span></h3>`;
  // je Art des Hinweises: „1 × Tausch erkannt · 2 × antwortet nicht“ (Konzept Export 3.6)
  const kinds = Object.entries(p.summary || {})
    .map(([k, n]) => `${n} × ${t("ab_s_" + k)}`)
    .join(" · ");
  const notes = p.notes ? `<p class="chip warn wide">${esc(t("ab_notes", p.notes))}${kinds ? `: ${esc(kinds)}` : ""}</p>` : "";
  if (!p.rows.length) return head + `<p class="sub">${esc(t("ab_empty"))}</p>`;
  const cols = previewCols(p.header);
  // Zähler ohne Namen: ID oder PA zeigen, sonst weiss man nicht, welche Zeile wozu gehört
  const cell = (r, i) => (i === 2 && data.settings.form !== "series" && !r[2] ? [r[4] && `ID ${r[4]}`, r[5] && `PA ${r[5]}`].filter(Boolean).join(" · ") : r[i] ?? "");
  const pick = (r) => cols.map((i) => cell(r, i));
  return `${head}${notes}<div class="btable"><table><thead><tr>${cols.map((i) => `<th>${esc(p.header[i] ?? "")}</th>`).join("")}</tr></thead><tbody>${p.rows
    .map((r) => `<tr>${pick(r).map((c) => `<td>${esc(c)}</td>`).join("")}</tr>`)
    .join("")}</tbody></table></div>${p.total > p.rows.length ? `<p class="sub">${esc(t("ab_more", p.total - p.rows.length))}</p>` : ""}`;
}
