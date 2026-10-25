"use strict";

// Bausteine der Oberfläche: einmal gebaut, überall gleich verwendet (siehe docs/konzept-bedienung.md)

const ui = {
  /// Status-Chip: kind "ok" (grün), "warn" (orange), "muted" (grau), "err" (rot), "info" (Akzent)
  chip(kind, text, title = "") {
    const cls = { ok: "st-ok", warn: "st-collision", muted: "st-no_data", err: "st-err", info: "st-info" }[kind] || "st-no_data";
    return `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(text)}</span>`;
  },

  /// Kleiner Knopf mit Symbol; `data` = { name: wert } für data-Attribute
  iconBtn(icon, title, data = {}, cls = "") {
    const attrs = Object.entries(data)
      .map(([k, v]) => `data-${k}="${esc(v)}"`)
      .join(" ");
    return `<button type="button" class="icon-btn ${cls}" ${attrs} title="${esc(title)}" aria-label="${esc(title)}">${icon}</button>`;
  },

  // ---------------------------------------------------------- Zähler

  /// Medium-Symbol (abgerundetes Quadrat, Farbe je Medium); small = 22 px statt 34 px
  mediumIcon(medium, small = false) {
    const kind = MEDIUM_KIND[medium] || "other";
    const size = small ? 12 : 18;
    return `<span class="mid m-${kind}${small ? " s" : ""}" aria-hidden="true"><svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${
      small ? 2.2 : 2
    }" stroke-linecap="round" stroke-linejoin="round">${MEDIUM_ICON[kind]}</svg></span>`;
  },

  /// Ein Zähler, überall gleich: Symbol, Name (fehlt: „ohne Namen“), darunter ID · PA · Hersteller.
  /// m = { name, ident, pa, man, medium, info }; opts: { chip (html rechts), alert (roter Punkt) }
  meter(m, { chip = "", alert = false } = {}) {
    const info = [m.ident ? `ID ${m.ident}` : "", m.pa != null && m.pa !== "" ? `PA ${m.pa}` : "", m.man, m.info].filter(Boolean).join(" · ");
    const name = m.name ? `<span class="mtr-name">${esc(m.name)}</span>` : `<span class="mtr-name none">${esc(t("meter_unnamed"))}</span>`;
    return `<div class="mtr">${ui.mediumIcon(m.medium).replace('class="mid', `class="mid${alert ? " alert" : ""}`)}<span class="mtr-text">${name}<span class="mtr-info">${esc(
      info
    )}</span></span>${chip}</div>`;
  },

  /// Kurzform als Pille: kleines Symbol und Name (oder ID), für Texte und Kopfzeilen
  meterShort(m) {
    return `<span class="mtr-pill">${ui.mediumIcon(m.medium, true)}${m.name ? esc(m.name) : `<span class="mono">${esc(m.ident || "")}</span>`}</span>`;
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

  // ---------------------------------------------------------- Eingabe, Formular
  // Felder: { name, label, type: text|number|password|select|check|textarea|time24|html,
  //           options: [[wert, text]], num (Auswahl liefert Zahl, "" = null), list (Vorschläge),
  //           wide, cls, placeholder, hint, required, max (Zeichen), data ({ name: wert }),
//           value (Vorgabe, wenn in den Werten nichts steht),
  //           html (bei type html: fertiger Baustein)

  /// Ein Eingabefeld ohne Beschriftung (im Formular und in bearbeitbaren Tabellen)
  /// Datei als JSON lesen; nicht lesbar: Fehler mit der Meldung `bad` (z.B. für guarded)
  async readJson(file, bad) {
    try {
      return JSON.parse(await file.text());
    } catch {
      throw new Error(bad);
    }
  },

  input(f, v) {
    const data = Object.entries(f.data || {})
      .map(([k, x]) => ` data-${k}="${esc(x)}"`)
      .join("");
    const common = `name="${f.name}" class="${f.cls || ""}" aria-label="${esc(f.label)}"${data} ${f.placeholder ? `placeholder="${esc(f.placeholder)}"` : ""} ${
      f.required ? "required" : ""
    } ${f.list ? `list="${f.list}"` : ""} ${f.max ? `maxlength="${f.max}"` : ""}`;
    if (f.type === "check") return `<input type="checkbox" ${common} ${v ? "checked" : ""}>`;
    if (f.type === "select")
      return `<select ${common}>${f.options.map(([o, txt]) => `<option value="${esc(o)}" ${String(o) === String(v ?? "") ? "selected" : ""}>${esc(txt)}</option>`).join("")}</select>`;
    if (f.type === "textarea") return `<textarea ${common} rows="${f.rows || 4}">${esc(v ?? "")}</textarea>`;
    if (f.type === "time24") return `<input ${common} value="${esc(v ?? "")}" inputmode="numeric" maxlength="5" autocomplete="off" data-time24>`;
    return `<input ${common} type="${f.type === "password" ? "password" : "text"}" value="${esc(v ?? "")}" ${f.type === "number" ? 'inputmode="decimal"' : ""} autocomplete="off">`;
  },

  form(fields, values = {}) {
    const field = (f) => {
      if (f.type === "html") return `<div class="span-all">${f.html}</div>`;
      const input = ui.input(f, values[f.name] ?? f.value);
      if (f.type === "check") return `<label class="check ${f.wide ? "span-2" : ""}" data-field="${f.name}">${input} ${esc(f.label)}</label>`;
      return `<label class="${f.wide ? "span-2" : ""}" data-field="${f.name}"><span>${esc(f.label)}</span> ${input}</label>`;
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
      // ausgeblendete Felder (z.B. Uhrzeit bei „stündlich“) zählen nicht
      if (!el || el.closest("[hidden]")) continue;
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

/// Tabelle aus CSV oder aus Excel kopiert (Tabulator); Trennzeichen wird erkannt
function parseTable(text) {
  text = text.replace(/^\uFEFF/, "");
  const first = text.split(/\r?\n/)[0] || "";
  const delim = ["\t", ";", ","].map((d) => [d, first.split(d).length]).sort((a, b) => b[1] - a[1])[0][0];
  const rows = [];
  let row = [];
  let cur = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          cur += '"';
          i++;
        } else quoted = false;
      } else cur += c;
    } else if (c === '"' && cur === "") quoted = true;
    else if (c === delim) {
      row.push(cur);
      cur = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cur);
      rows.push(row);
      row = [];
      cur = "";
    } else cur += c;
  }
  if (cur || row.length) {
    row.push(cur);
    rows.push(row);
  }
  return rows.map((r) => r.map((x) => x.trim())).filter((r) => r.some(Boolean));
}

/// Zähler der Liste für den Zähler-Baustein: ID (über PA gemerkt: die ID aus der Auslesung), PA,
/// Hersteller, eigene Baudrate
function meterBlock(m) {
  const viaPa = /^\d+$/.test(m.address);
  return {
    name: m.name,
    ident: viaPa ? m.meter_id : m.meter_id || m.address,
    pa: viaPa ? m.address : m.primary_address,
    man: m.manufacturer,
    medium: m.medium,
    info: m.baud ? `${m.baud} Bd` : "",
  };
}

/// Symbole (Strich-SVG, Farbe vom Text)
const svg = (d, size = 15, stroke = 2) =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON_EDIT = svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>');
const ICON_DEL = svg('<path d="M6 6l12 12M18 6L6 18"/>');
const ICON_UP = svg('<path d="M6 15l6-6 6 6"/>');
const ICON_DOWN = svg('<path d="M6 9l6 6 6-6"/>');
const ICON_CHART = svg('<path d="M4 19h16M5 15l4-5 4 3 6-7"/>');
const ICON_ALERT = svg('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16.5v.01"/>');
const ICON_CLOCK = svg('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>');
const ICON_CHECK = svg('<path d="M5 12l5 5L20 7"/>', 11, 3.2);
const ICON_NEXT = svg('<path d="M5 12h14M13 6l6 6-6 6"/>', 14);

/// Medium-Schlüssel vom Server → Art des Symbols
const MEDIUM_KIND = {
  medium_heat_outlet: "heat",
  medium_heat_inlet: "heat",
  medium_heat_cooling: "heat",
  medium_steam: "heat",
  medium_hca: "heat",
  medium_cooling_inlet: "cold",
  medium_water: "water",
  medium_cold_water: "water",
  medium_dual_water: "water",
  medium_warm_water: "warm",
  medium_hot_water: "warm",
  medium_electricity: "power",
  medium_gas: "gas",
};
const MEDIUM_ICON = {
  heat: '<path d="M12 3c1 3 5 5 5 10a5 5 0 0 1-10 0c0-3 2-4 2-7 1.5 1 3 2.5 3 4.5"/>',
  cold: '<path d="M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9"/>',
  water: '<path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"/>',
  warm: '<path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"/><path d="M9 15c1-1 2 1 3 0s2 1 3 0"/>',
  power: '<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>',
  gas: '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
  other: '<rect x="5" y="5" width="14" height="14" rx="3"/><path d="M9 12h6"/>',
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
    if (!el.matches?.("[data-time24]")) return;
    const v = time24(el.value);
    el.classList.toggle("invalid", !v);
    if (v) el.value = v;
  },
  true
);

// Menüs (<details class="menu">): nach einer Auswahl oder einem Klick daneben schliessen
document.addEventListener("click", (ev) => {
  for (const m of document.querySelectorAll("details.menu[open]")) {
    if (!m.contains(ev.target) || ev.target.closest(".menu-items button, .menu-items a")) m.open = false;
  }
});
