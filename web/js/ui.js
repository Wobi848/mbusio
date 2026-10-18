"use strict";

// Bausteine der Oberfläche: einmal gebaut, überall gleich verwendet (siehe docs/konzept-bedienung.md)

const ui = {
  /// Status-Chip: kind "ok" (grün), "warn" (orange), "muted" (grau), "err" (rot)
  chip(kind, text, title = "") {
    const cls = { ok: "st-ok", warn: "st-collision", muted: "st-no_data", err: "st-err" }[kind] || "st-no_data";
    return `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(text)}</span>`;
  },

  /// Kleiner Knopf mit Symbol; `data` = { name: wert } für data-Attribute
  iconBtn(icon, title, data = {}, cls = "") {
    const attrs = Object.entries(data)
      .map(([k, v]) => `data-${k}="${esc(v)}"`)
      .join(" ");
    return `<button type="button" class="icon-btn ${cls}" ${attrs} title="${esc(title)}" aria-label="${esc(title)}">${icon}</button>`;
  },

  // ---------------------------------------------------------- Tabelle
  // Spalten: { head: Text, cls: Klasse der Zelle, cell: (zeile) => html }

  head(columns) {
    return `<thead><tr>${columns.map((c) => `<th class="${c.cls || ""}">${c.headHtml ?? esc(c.head ?? "")}</th>`).join("")}</tr></thead>`;
  },

  cells(columns, row) {
    return columns.map((c) => `<td class="${c.cls || ""}">${c.cell(row)}</td>`).join("");
  },

  /// Ganze Tabelle; leer: Hinweis statt Tabelle
  table({ columns, rows, empty = "", cls = "", rowAttrs = () => "" }) {
    if (!rows.length) return empty ? `<p class="empty">${esc(empty)}</p>` : "";
    return `<div class="table-wrap"><table class="${cls}">${ui.head(columns)}<tbody>${rows
      .map((r) => `<tr ${rowAttrs(r)}>${ui.cells(columns, r)}</tr>`)
      .join("")}</tbody></table></div>`;
  },

  // ---------------------------------------------------------- Aufklapp-Bereich

  /// Kasten mit Titel, Inhalt und „Schliessen“ (Knopf mit data-panel-close)
  panel(title, body, { close = true, actions = "" } = {}) {
    return `<div class="panel"><div class="panel-head"><h3>${esc(title)}</h3><span class="spacer"></span>${actions}${
      close ? `<button type="button" class="small quiet" data-panel-close>${esc(t("history_close"))}</button>` : ""
    }</div>${body}</div>`;
  },

  /// Kasten ein- bzw. ausblenden; Schliessen-Knopf leert ihn
  show(box, html) {
    box.innerHTML = html;
    box.hidden = !html;
    box.onclick ??= (ev) => {
      if (ev.target.closest("[data-panel-close]")) ui.show(box, "");
    };
    if (html) box.scrollIntoView({ behavior: "smooth", block: "nearest" });
  },

  // ---------------------------------------------------------- Vorschau → Übernehmen

  /// Änderungssatz vom Server: was neu ist, was sich ändert, Hinweise, Fehler
  changeSet(cs) {
    const n = cs.added.length + cs.changed.length;
    const list = (items, cls) => (items.length ? `<ul class="${cls}">${items.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>` : "");
    const rows = [
      ...cs.added.map((a) => `<tr><td>${ui.chip("ok", t("cs_new"))}</td><td class="mono">${esc(a.item)}</td><td colspan="2">${esc(a.detail)}</td></tr>`),
      ...cs.changed.map(
        (c) => `<tr><td>${esc(c.field)}</td><td class="mono">${esc(c.item)}</td><td>${esc(c.old || "–")} → <b>${esc(c.new || "–")}</b></td><td></td></tr>`
      ),
    ];
    return `<p class="cs-summary">${esc(t("cs_summary", cs.added.length, cs.changed.length, cs.unchanged))}</p>
      ${list(cs.errors, "err-list")}${cs.errors.length ? `<p class="hint">${esc(t("cs_errors_note"))}</p>` : ""}
      ${list(cs.warnings, "warn-list")}
      ${!n && !cs.errors.length ? `<p class="hint">${esc(t("cs_nothing"))}</p>` : ""}
      ${rows.length ? `<div class="table-wrap"><table class="cs-table"><tbody>${rows.join("")}</tbody></table></div>` : ""}`;
  },

  /// Ablauf „prüfen, zeigen, übernehmen“ in einem Kasten. `run(apply)` liefert den
  /// Änderungssatz; nach dem Übernehmen wird `done(cs)` aufgerufen.
  async previewApply(box, { title, run, done }) {
    const render = (cs) => {
      const can = cs.added.length + cs.changed.length > 0 && !cs.errors.length;
      ui.show(
        box,
        ui.panel(
          title,
          ui.changeSet(cs) +
            `<div class="row">${can ? `<button type="button" data-cs-apply>${esc(t("cs_apply"))}</button>` : ""}
             <button type="button" class="quiet" data-panel-close>${esc(t("cs_discard"))}</button></div>`,
          { close: false }
        )
      );
      box.querySelector("[data-cs-apply]")?.addEventListener("click", (ev) =>
        guarded(ev.currentTarget, async () => {
          const r = await run(true);
          if (!r.applied) return render(r);
          ui.show(box, "");
          await done?.(r);
        })
      );
    };
    render(await run(false));
  },

  // ---------------------------------------------------------- Formular
  // Felder: { name, label, type: text|number|password|select|check|textarea|time24|html,
  //           options: [[wert, text]], num (Auswahl liefert Zahl, "" = null), list (Vorschläge),
  //           wide, cls, placeholder, hint, required, html (bei type html: fertiger Baustein) }

  form(fields, values = {}) {
    const field = (f) => {
      if (f.type === "html") return `<div class="span-all">${f.html}</div>`;
      const v = values[f.name];
      const label = esc(f.label);
      const common = `name="${f.name}" class="${f.cls || ""}" ${f.placeholder ? `placeholder="${esc(f.placeholder)}"` : ""} ${f.required ? "required" : ""} ${
        f.list ? `list="${f.list}"` : ""
      }`;
      let input;
      if (f.type === "check")
        return `<label class="check ${f.wide ? "span-2" : ""}" data-field="${f.name}"><input type="checkbox" name="${f.name}" ${v ? "checked" : ""}> ${label}</label>`;
      if (f.type === "select")
        input = `<select ${common}>${f.options.map(([o, txt]) => `<option value="${esc(o)}" ${String(o) === String(v ?? "") ? "selected" : ""}>${esc(txt)}</option>`).join("")}</select>`;
      else if (f.type === "textarea") input = `<textarea ${common} rows="${f.rows || 4}">${esc(v ?? "")}</textarea>`;
      else if (f.type === "time24")
        input = `<input ${common} value="${esc(v ?? "")}" inputmode="numeric" maxlength="5" autocomplete="off" data-time24>`;
      else
        input = `<input ${common} type="${f.type === "password" ? "password" : "text"}" value="${esc(v ?? "")}" ${f.type === "number" ? 'inputmode="decimal"' : ""} autocomplete="off">`;
      return `<label class="${f.wide ? "span-2" : ""}" data-field="${f.name}"><span>${label}</span> ${input}</label>`;
    };
    return `<div class="form-grid">${fields.map(field).join("")}</div>${fields
      .filter((f) => f.hint)
      .map((f) => `<p class="hint">${esc(f.hint)}</p>`)
      .join("")}`;
  },

  /// Werte eines Formulars lesen und prüfen; wirft eine verständliche Meldung
  formValues(root, fields) {
    const out = {};
    for (const f of fields) {
      const el = root.querySelector(`[name="${f.name}"]`);
      if (!el) continue;
      if (f.type === "check") out[f.name] = el.checked;
      else if (f.type === "number") {
        const n = Number(String(el.value).trim().replace(",", "."));
        if (el.value.trim() === "" && !f.required) out[f.name] = null;
        else if (!Number.isFinite(n)) throw new Error(`${f.label}: ${t("form_number")}`);
        else out[f.name] = n;
      } else if (f.type === "time24") {
        const v = time24(el.value);
        if (!v) throw new Error(t("plan_time_bad"));
        out[f.name] = v;
      } else if (f.num) out[f.name] = el.value === "" ? null : Number(el.value);
      else out[f.name] = el.value.trim();
      if (f.required && (out[f.name] === "" || out[f.name] == null)) throw new Error(`${f.label}: ${t("form_required")}`);
    }
    return out;
  },
};

/// Uhrzeit immer 24 h als HH:MM; Kurzformen: "5" → 05:00, "530" → 05:30, "17:5" → 17:05.
/// null, wenn es keine gültige Uhrzeit ist.
function time24(s) {
  const v = String(s || "").trim();
  let h;
  let m;
  const parts = v.split(/[:.h ]/).filter(Boolean);
  if (parts.length === 2) [h, m] = parts;
  else if (/^\d{1,2}$/.test(v)) [h, m] = [v, "0"];
  else if (/^\d{3,4}$/.test(v)) [h, m] = [v.slice(0, -2), v.slice(-2)];
  else return null;
  if (!/^\d{1,2}$/.test(h) || !/^\d{1,2}$/.test(m) || Number(h) > 23 || Number(m) > 59) return null;
  return `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;
}

// Uhrzeitfelder überall: beim Verlassen in die Normalform bringen, Ungültiges markieren
document.addEventListener(
  "blur",
  (ev) => {
    const el = ev.target;
    if (!el.matches?.("[data-time24], [name=plan_time]")) return;
    const v = time24(el.value);
    el.classList.toggle("invalid", !v);
    if (v) el.value = v;
  },
  true
);
