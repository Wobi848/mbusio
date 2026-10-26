// Bereich Anlage: Abrechnung exportieren (Konzept Export, Teil B). Feld im Detailbereich: Zeitraum,
// Form, Werte, Dezimaltrennzeichen; jede Änderung wird gleich je Projekt gespeichert und die
// Vorschau neu gerechnet. Herunterladen nimmt die gespeicherten Einstellungen.

import { html } from "./view.js";
import { report } from "./data.js";

const PERIODS = ["prev_month", "prev_year", "custom"];

let box = null;
let projectId = null;
let data = null; // Antwort von /api/projects/{id}/billing

/// Feld in `container` öffnen; `close` führt zurück zu den Details des Projekts
export async function openBilling(container, id, close) {
  box = container;
  projectId = id;
  box.onclick = (ev) => ev.target.closest("[data-billing-back], [data-billing-close]") && close();
  box.onchange = onChange;
  box.classList.add("open");
  box._html = null;
  box.innerHTML = `<p class="sub">…</p>`;
  try {
    data = await request(`projects/${id}/billing`);
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
  if (el.matches("[data-b-key]")) {
    const keys = new Set(s.keys);
    if (el.checked) keys.add(el.dataset.bKey);
    else keys.delete(el.dataset.bKey);
    return save({ keys: [...keys] });
  }
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
  const files = data.files.length
    ? `<div class="vals">${data.files
        .map(
          (f) =>
            `<a class="vrow" data-b-file href="${apiHref(`projects/${projectId}/billing/files/${encodeURIComponent(f.name)}`)}" download><span>${esc(f.name)}</span><span class="sub">${esc(slotText(f.time))}</span><span class="sub">${esc(Math.max(1, Math.round(f.size / 1024)))} KB</span></a>`
        )
        .join("")}</div>`
    : `<p class="sub">${esc(t("an_b_files_none"))}</p>`;
  return `<h3>${esc(t("an_b_auto"))}</h3>
    <div class="vals">
      <label class="vrow"><span>${esc(t("an_b_auto_label"))}</span><span class="sub">${esc(hint)}</span><input type="checkbox" data-b-auto${s.auto && !custom ? " checked" : ""}${custom ? " disabled" : ""}></label>
    </div>
    ${s.auto || data.files.length ? files : ""}`;
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
