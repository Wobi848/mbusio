// Bereich Anlage: Abrechnung exportieren (Konzept Export, Teil B). Feld im Detailbereich: Zeitraum,
// Form, Werte, Dezimaltrennzeichen; jede Änderung wird gleich je Projekt gespeichert und die
// Vorschau neu gerechnet. Herunterladen nimmt die gespeicherten Einstellungen. Automatisch
// abgelegte Dateien gehen an die Empfänger des Projekts (Teil C, Bausteine in anlage-mail.js).

import { html, toast } from "./view.js";
import { report } from "./data.js";
import { mailStateHtml } from "./anlage-mail.js";

const PERIODS = ["prev_month", "prev_year", "custom"];

let box = null;
let projectId = null;
let data = null; // Antwort von /api/projects/{id}/billing
let mail = null; // Antwort von /api/mail (ohne Passwort): ist ein Server eingerichtet?

/// Feld in `container` öffnen; `close` führt zurück zu den Details des Projekts
export async function openBilling(container, id, close) {
  box = container;
  projectId = id;
  box.onclick = (ev) => {
    if (ev.target.closest("[data-billing-back], [data-billing-close]")) return close();
    onClick(ev);
  };
  box.onchange = onChange;
  box.classList.add("open");
  box._html = null;
  box.innerHTML = `<p class="sub">…</p>`;
  try {
    [data, mail] = await Promise.all([request(`projects/${id}/billing`), request("mail")]);
    render();
  } catch (e) {
    report(e);
  }
}

export function closeBilling() {
  if (!box) return;
  box.onclick = null;
  box.onchange = null;
  box = null;
}

async function save(change) {
  try {
    data = await request(`projects/${projectId}/billing`, { method: "PUT", body: { ...data.settings, ...change } });
    render();
  } catch (e) {
    report(e);
  }
}

function onChange(ev) {
  const el = ev.target;
  const s = data.settings;
  if (el.matches("[data-b-period]")) return save({ period: el.value });
  if (el.matches("[data-b-from]")) return save({ from: el.value || null });
  if (el.matches("[data-b-to]")) return save({ to: el.value || null });
  if (el.matches("[data-b-form]")) return save({ form: el.value });
  if (el.matches("[data-b-step]")) return save({ step: el.value });
  if (el.matches("[data-b-main]")) return save({ main: el.checked });
  if (el.matches("[data-b-decimal]")) return save({ decimal: el.value });
  if (el.matches("[data-b-auto]")) return save({ auto: el.checked });
  if (el.matches("[data-b-mail-to]")) return save({ mail_to: el.value.trim() });
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

/// Knopf während der Arbeit sperren; Fehler als Meldung, Erfolg als kurze Bestätigung
async function busy(button, work, done) {
  button.disabled = true;
  try {
    await work();
    toast(done);
  } catch (e) {
    report(e);
  } finally {
    button.disabled = false;
  }
}

function onClick(ev) {
  const send = ev.target.closest("[data-b-send]");
  if (send)
    return busy(
      send,
      async () => {
        try {
          data = await request(`projects/${projectId}/billing/files/${encodeURIComponent(send.dataset.bSend)}/send`, { method: "POST" });
        } finally {
          // auch beim Fehler: der Grund steht jetzt an der Datei
          data = await request(`projects/${projectId}/billing`);
          render();
        }
      },
      t("an_m_sent_now")
    );
}

/// Der Mailserver gehört zur Weitergabe (Ausgang E-Mail): hier nur sagen, wenn er fehlt oder aus ist
function mailHint() {
  const c = mail.config;
  const why = !c.host ? t("an_m_no_server") : !c.enabled ? t("an_m_mail_off") : "";
  return why ? `<p class="chip warn wide">${esc(why)} <a href="#/weitergabe/email">${esc(t("an_m_setup"))} →</a></p>` : "";
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

/// Linien zur Auswahl, nur wenn das Projekt mehrere hat
function linesHtml(s) {
  if (data.lines.length < 2) return "";
  const on = (id) => !s.lines.length || s.lines.includes(id);
  const n = data.lines.filter((l) => on(l.id)).length;
  const what = n === data.lines.length ? t("an_b_lines_all", n) : t("an_b_lines_some", n, data.lines.length);
  return `<details class="more"${n < data.lines.length ? " open" : ""}><summary>${esc(t("an_b_lines", what))}</summary>
    <div class="vals">${data.lines
      .map(
        (l) =>
          `<label class="vrow"><span>${esc(l.label)}</span><span class="sub">${esc(t("an_b_line_meters", l.meters))}</span><input type="checkbox" data-b-line="${l.id}"${on(l.id) ? " checked" : ""}></label>`
      )
      .join("")}</div></details>`;
}

const opt = (value, label, current) => `<option value="${esc(value)}"${value === current ? " selected" : ""}>${esc(label)}</option>`;
const day = (iso) => (lang === "de" ? iso.split("-").reverse().join(".") : iso);

function render() {
  if (!box) return;
  const s = data.settings;
  const p = data.preview;
  const decimal = s.decimal || (lang === "de" ? "," : ".");
  html(
    box,
    `<button type="button" class="quiet back" data-billing-back>← ${esc(t("an_billing_back"))}</button>
    <header class="dhead"><div><b>${esc(t("an_billing"))}</b><div class="sub">${esc(t("an_billing_hint"))}</div></div></header>
    <div class="fields">
      <label><span>${esc(t("an_b_period"))}</span><select data-b-period>${PERIODS.map((x) => opt(x, t("an_b_" + x), s.period)).join("")}</select></label>
      ${
        s.period === "custom"
          ? `<label><span>${esc(t("an_b_from"))}</span><input type="date" data-b-from value="${esc(s.from || "")}"></label>
             <label><span>${esc(t("an_b_to"))}</span><input type="date" data-b-to value="${esc(s.to || "")}"></label>`
          : ""
      }
      <label><span>${esc(t("an_b_form"))}</span><select data-b-form>${opt("billing", t("an_b_billing"), s.form)}${opt("series", t("an_b_series"), s.form)}</select></label>
      ${s.form === "series" ? `<label><span>${esc(t("an_b_step"))}</span><select data-b-step>${opt("month", t("an_b_month"), s.step)}${opt("day", t("an_b_day"), s.step)}</select></label>` : ""}
      <label><span>${esc(t("an_b_decimal"))}</span><select data-b-decimal>${opt(",", t("an_b_comma"), decimal)}${opt(".", t("an_b_point"), decimal)}</select></label>
    </div>
    ${linesHtml(s)}
    <h3>${esc(t("an_b_values"))}</h3>
    <div class="vals">
      <label class="vrow main"><span>${esc(t("an_b_main"))}</span><span class="sub">${esc(t("an_b_main_hint"))}</span><input type="checkbox" data-b-main${s.main ? " checked" : ""}></label>
    </div>
    <details class="more"${s.keys.length ? " open" : ""}><summary>${esc(t("an_b_more_values", data.kinds.length))}</summary>
    <div class="vals">
      ${data.kinds
        .map(
          (k) =>
            `<label class="vrow"><span>${esc(k.label)} <span class="sub">${esc(k.unit)}</span></span><span class="sub">${esc(t("an_b_meters", k.meters, k.main))}</span><input type="checkbox" data-b-key="${esc(k.key)}"${
              s.keys.includes(k.key) ? " checked" : ""
            }></label>`
        )
        .join("")}
    </div></details>
    ${preview(p)}
    <div class="actions">
      ${p ? `<a class="button" data-b-download href="${apiHref(`projects/${projectId}/billing/csv`)}" download>${esc(t("an_b_download"))}</a>` : ""}
      <button type="button" class="quiet" data-billing-close>${esc(t("close"))}</button>
    </div>
    ${auto(s)}`
  );
}

/// Automatisch ablegen (Konzept Export 3.5): Schalter und abgelegte Dateien
function auto(s) {
  const custom = s.period === "custom";
  const hint = custom ? t("an_b_auto_custom") : t("an_b_auto_" + s.period);
  const active = s.auto && !custom;
  const files = data.files.length
    ? `<div class="vals">${data.files
        .map(
          (f) =>
            `<div class="vrow bfile"><a data-b-file href="${apiHref(`projects/${projectId}/billing/files/${encodeURIComponent(f.name)}`)}" download title="${esc(t("an_b_file_title", Math.max(1, Math.round(f.size / 1024))))}">${esc(f.name)}</a><span class="sub">${esc(slotText(f.time))}</span>${mailStateHtml(f, !!s.mail_to)}</div>`
        )
        .join("")}</div>`
    : `<p class="sub">${esc(t("an_b_files_none"))}</p>`;
  // warum die neueste Datei nicht ging: sichtbar, nicht nur im Tooltip
  const why = data.files[0]?.mail?.error;
  const problem = why ? `<p class="chip warn wide">${esc(t("an_m_why", why))}</p>` : "";
  return `<h3>${esc(t("an_b_auto"))}</h3>
    <div class="vals">
      <label class="vrow"><span>${esc(t("an_b_auto_label"))}</span><span class="sub">${esc(hint)}</span><input type="checkbox" data-b-auto${active ? " checked" : ""}${custom ? " disabled" : ""}></label>
    </div>
    ${
      active
        ? `<div class="fields bmail"><label><span>${esc(t("an_m_to"))}</span><input type="text" data-b-mail-to value="${esc(s.mail_to)}" placeholder="${esc(t("an_m_to_ph"))}"></label><p class="sub hint">${esc(t("an_m_to_hint"))}</p></div>`
        : ""
    }
    ${s.auto || data.files.length ? files + problem : ""}
    ${active && s.mail_to ? mailHint() : ""}`;
}

/// Spalten der Vorschau (der Detailbereich ist schmal): was man prüfen will; die Datei hat alle
const PREVIEW_COLS = { billing: [2, 7, 8, 9, 11, 13, 14], series: [0, 3, 8, 9, 10, 11] };

function preview(p) {
  if (!p) return `<p class="chip warn wide">${esc(data.error || "")}</p>`;
  const head = `<h3>${esc(t("an_b_preview"))} <span class="sub">${esc(`${day(p.from)} – ${day(p.to)} · ${t("an_b_rows", p.total)}`)}</span></h3>`;
  // je Art des Hinweises: „1 × Tausch erkannt · 2 × antwortet nicht“ (Konzept Export 3.6)
  const kinds = Object.entries(p.summary || {})
    .map(([k, n]) => `${n} × ${t("an_bs_" + k)}`)
    .join(" · ");
  const notes = p.notes ? `<p class="chip warn wide">${esc(t("an_b_notes", p.notes))}${kinds ? `: ${esc(kinds)}` : ""}</p>` : "";
  if (!p.rows.length) return head + `<p class="sub">${esc(t("an_b_empty"))}</p>`;
  const cols = PREVIEW_COLS[data.settings.form];
  const pick = (r) => cols.map((i) => r[i] ?? "");
  return `${head}${notes}<div class="btable"><table><thead><tr>${pick(p.header).map((h) => `<th>${esc(h)}</th>`).join("")}</tr></thead><tbody>${p.rows
    .map((r) => `<tr>${pick(r).map((c) => `<td>${esc(c)}</td>`).join("")}</tr>`)
    .join("")}</tbody></table></div>${p.total > p.rows.length ? `<p class="sub">${esc(t("an_b_more", p.total - p.rows.length))}</p>` : ""}`;
}
