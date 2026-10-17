"use strict";

// Zählerliste schnell füllen: Liste importieren, Benennen, Namen, Live

// ------------------------------------------------------------ Zählerliste schnell füllen

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

const LIST_FIELDS = ["id", "name", "location", "note", "pa"];
const LIST_GUESS = {
  id: /^(id|ident|z(ä|ae)hler.?(nr|nummer)|serien.?(nr|nummer)|fab(rikations)?.?(nr|nummer)|meter.?(id|no)|serial|nummer|nr\.?)$/i,
  name: /^(name|bezeichnung|benennung|description)$/i,
  location: /^(ort|lage|standort|einbauort|wohnung|whg|raum|location|place)$/i,
  note: /^(notiz|bemerkung|kommentar|note|remark|comment)$/i,
  pa: /^(pa|prim(ä|ae)r.?adr(esse)?|primary.?address|m-?bus.?adr(esse)?)$/i,
};
let listData = null;

function listOpen() {
  $("list-panel").hidden = false;
  $("list-result").innerHTML = "";
  $("list-panel").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

/// Eingelesene Tabelle: Überschrift erkennen, Spalten zuordnen (geraten), Vorschau
function listLoad(text) {
  const rows = parseTable(text);
  if (!rows.length) {
    listData = null;
    $("list-map").innerHTML = "";
    return;
  }
  const looksId = (v) => /^\D{0,4}\d{5,8}$/.test(v.replace(/\s/g, ""));
  const header = !rows[0].some(looksId) && rows.length > 1;
  const cols = Math.max(...rows.map((r) => r.length));
  const map = {};
  if (header) rows[0].forEach((h, i) => LIST_FIELDS.forEach((f) => map[f] === undefined && LIST_GUESS[f].test(h.trim()) && (map[f] = i)));
  // ohne Überschrift: Spalte mit den meisten ID-artigen Werten ist die ID
  if (map.id === undefined) {
    let best = -1;
    let score = 0;
    for (let c = 0; c < cols; c++) {
      const n = rows.slice(header ? 1 : 0).filter((r) => looksId(r[c] || "")).length;
      if (n > score) [best, score] = [c, n];
    }
    if (best >= 0) map.id = best;
  }
  listData = { rows, header, cols, map };
  renderListMap();
}

function renderListMap() {
  const d = listData;
  const names = Array.from({ length: d.cols }, (_, i) => (d.header && d.rows[0][i]) || t("list_col", i + 1));
  const data = d.rows.slice(d.header ? 1 : 0);
  $("list-map").innerHTML = `<h3 class="list-h">${esc(t("list_map"))} <span class="hint">${esc(t("list_rows", data.length))}</span></h3>
    <label class="check"><input type="checkbox" id="list-header" ${d.header ? "checked" : ""}> ${esc(t("list_header"))}</label>
    <div class="form-grid list-grid">${LIST_FIELDS.map(
      (f) => `<label><span>${esc(t("list_f_" + f))}</span><select data-list-f="${f}"><option value="">${esc(t("list_unused"))}</option>${names
        .map((n, i) => `<option value="${i}" ${d.map[f] === i ? "selected" : ""}>${esc(n)}</option>`)
        .join("")}</select></label>`
    ).join("")}</div>
    <div class="table-wrap"><table class="list-preview"><thead><tr>${LIST_FIELDS.map((f) => `<th>${esc(t("list_f_" + f))}</th>`).join("")}</tr></thead>
    <tbody>${data
      .slice(0, 4)
      .map((r) => `<tr>${LIST_FIELDS.map((f) => `<td>${esc(d.map[f] !== undefined ? r[d.map[f]] || "" : "")}</td>`).join("")}</tr>`)
      .join("")}</tbody></table></div>
    <div class="row"><button type="button" id="list-check">${esc(t("list_check"))}</button></div>`;
}

function listRows() {
  const d = listData;
  return d.rows.slice(d.header ? 1 : 0).map((r) => Object.fromEntries(LIST_FIELDS.map((f) => [f, d.map[f] !== undefined ? r[d.map[f]] || "" : ""])));
}

async function listCheck(apply) {
  if (listData?.map.id === undefined) throw new Error(t("list_need_id"));
  const r = await api("meters/import-list", { method: "POST", body: { rows: listRows(), apply } });
  if (r.applied) {
    toast(t("list_done", r.added.length, r.changed.length));
    $("list-panel").hidden = true;
    $("list-paste").value = "";
    listData = null;
    $("list-map").innerHTML = "";
    await loadMeters();
    return;
  }
  const field = (f) => t("list_field_" + f);
  const n = r.added.length + r.changed.length;
  $("list-result").innerHTML = `<h3 class="list-h">${esc(t("list_summary", r.added.length, r.changed.length, r.unchanged))}</h3>
    ${r.errors.length ? `<ul class="err-list">${r.errors.map((e) => `<li>${esc(e)}</li>`).join("")}</ul><p class="hint">${esc(t("list_errors_note"))}</p>` : ""}
    ${r.warnings.length ? `<ul class="sim-warn">${r.warnings.map((e) => `<li>${esc(e)}</li>`).join("")}</ul>` : ""}
    ${!n && !r.errors.length ? `<p class="hint">${esc(t("list_nothing"))}</p>` : ""}
    ${
      n
        ? `<div class="table-wrap"><table><tbody>${r.added
            .map((a) => `<tr><td><span class="chip st-ok">${esc(t("list_new"))}</span></td><td class="mono">${esc(a.id)}</td><td>${esc(a.name)}</td><td>${esc(a.location)}</td></tr>`)
            .join("")}${r.changed
            .map((c) => `<tr><td>${esc(field(c.field))}</td><td class="mono">${esc(c.id)}</td><td>${esc(c.old || "–")} → <b>${esc(c.new)}</b></td><td></td></tr>`)
            .join("")}</tbody></table></div>`
        : ""
    }
    ${n && !r.errors.length ? `<div class="row"><button type="button" id="list-apply">${esc(t("list_apply"))}</button></div>` : ""}`;
  $("list-result").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

$("list-open").onclick = listOpen;
$("list-close").onclick = () => ($("list-panel").hidden = true);
$("list-paste").addEventListener("input", () => {
  $("list-result").innerHTML = "";
  listLoad($("list-paste").value);
});
$("list-map").addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.id === "list-header") listData.header = el.checked;
  else if (el.dataset.listF) {
    const v = el.value === "" ? undefined : Number(el.value);
    // jede Spalte nur einmal
    for (const f of LIST_FIELDS) if (listData.map[f] === v && v !== undefined) delete listData.map[f];
    listData.map[el.dataset.listF] = v;
  }
  $("list-result").innerHTML = "";
  renderListMap();
});
$("list-panel").addEventListener("click", (ev) => {
  if (ev.target.id === "list-check") guarded(ev.target, () => listCheck(false));
  if (ev.target.id === "list-apply") guarded(ev.target, () => listCheck(true));
});

// Schnellerfassung
$("quick-toggle").onclick = () => {
  quickEdit = !quickEdit;
  $("names-panel").hidden = !quickEdit;
  if (quickEdit) renderNames();
  renderMeters();
  if (quickEdit) $("meters-table").querySelector("input.quick")?.focus();
};

async function quickSave(input) {
  const m = meters.find((x) => x.id === Number(input.dataset.id));
  const f = input.dataset.q;
  const v = input.value.trim();
  if (!m || m[f] === v) return;
  const old = m[f];
  // gleich übernehmen, damit ein Neuzeichnen den neuen Wert zeigt
  m[f] = v;
  try {
    const saved = await api("meters/" + m.id, { method: "PUT", body: { ...m, [f]: v } });
    Object.assign(m, saved);
    input.classList.add("saved");
    setTimeout(() => input.classList.remove("saved"), 900);
    // neue Orte gleich als Vorschlag
    if (f === "location") renderLocList();
  } catch (e) {
    m[f] = old;
    input.value = old;
    toast(e.message, true);
  }
}

function renderLocList() {
  $("loc-list").innerHTML = [...new Set(meters.map((m) => m.location).filter(Boolean))]
    .sort()
    .map((l) => `<option value="${esc(l)}">`)
    .join("");
}

/// Ort vom Zähler darüber übernehmen
function quickSame(id) {
  const k = meters.findIndex((m) => m.id === id);
  if (k < 1) return;
  const input = $("meters-table").querySelector(`input[data-q=location][data-id="${id}"]`);
  input.value = meters[k - 1].location;
  quickSave(input);
}

$("meters-table").addEventListener("change", (ev) => {
  if (ev.target.dataset.q) quickSave(ev.target);
});
$("meters-table").addEventListener("keydown", (ev) => {
  const el = ev.target;
  if (!el.dataset.q || ev.key !== "Enter") return;
  ev.preventDefault();
  // Enter: gleiches Feld beim nächsten Zähler (Shift: vorheriger)
  const all = [...$("meters-table").querySelectorAll(`input[data-q=${el.dataset.q}]`)];
  const next = all[all.indexOf(el) + (ev.shiftKey ? -1 : 1)];
  quickSave(el);
  if (next) {
    next.focus();
    next.select();
  }
});

// Namen nach Muster
function renderNames(preview = null) {
  const box = $("names-panel");
  const pattern = $("names-pattern")?.value ?? "{ort} {medium}";
  const only = $("names-only")?.checked ?? false;
  box.innerHTML = `<div class="history-head"><h3>${esc(t("names_title"))}</h3></div>
    <p class="hint">${esc(t("names_hint"))}</p>
    <div class="row names-row"><label class="grow"><span>${esc(t("names_pattern"))}</span> <input id="names-pattern" class="mono" value="${esc(pattern)}"></label>
      <label class="check"><input type="checkbox" id="names-only" ${only ? "checked" : ""}> ${esc(t("names_only_empty"))}</label></div>
    ${
      preview
        ? preview.length
          ? `<p class="hint">${esc(t("names_preview", preview.length))}</p><div class="table-wrap"><table><tbody>${preview
              .slice(0, 50)
              .map((n) => `<tr><td class="mono">${esc(n.ident || "")}</td><td>${esc(n.old || "–")} → <b>${esc(n.new)}</b></td></tr>`)
              .join("")}</tbody></table></div>
            <div class="row"><button type="button" id="names-apply">${esc(t("names_apply"))}</button></div>`
          : `<p class="hint">${esc(t("names_none"))}</p>`
        : `<div class="row"><button type="button" class="small ghost" id="names-preview">${esc(t("quick_names"))}</button></div>`
    }`;
}

async function namesRun(apply) {
  const body = { pattern: $("names-pattern").value, only_empty: $("names-only").checked, apply };
  const r = await api("meters/names", { method: "POST", body });
  if (apply) {
    toast(t("names_done", r.names.length));
    await loadMeters();
    renderNames();
  } else renderNames(r.names);
}

$("names-panel").addEventListener("click", (ev) => {
  if (ev.target.id === "names-preview") guarded(ev.target, () => namesRun(false));
  if (ev.target.id === "names-apply") guarded(ev.target, () => namesRun(true));
});
$("names-panel").addEventListener("input", (ev) => {
  // Muster geändert: Vorschau neu
  if (ev.target.id === "names-pattern" || ev.target.id === "names-only") renderNames();
});

// Live: einen Zähler immer wieder lesen
function stopLive() {
  clearInterval(liveTimer);
  liveTimer = null;
  liveId = null;
}

function toggleLive(m) {
  if (liveId === m.id) {
    stopLive();
    return renderMeters();
  }
  stopLive();
  liveId = m.id;
  toast(t("live_on", m.name || m.meter_id || m.address));
  let busy = false;
  const tick = async () => {
    if (busy || liveId !== m.id || jobRunning) return;
    busy = true;
    try {
      await api(`meters/${m.id}/read`, { method: "POST" });
    } catch {
      /* Fehler steht beim Zähler */
    }
    busy = false;
    if (liveId !== m.id) return;
    meters = await api("meters").catch(() => meters);
    renderMeters();
  };
  liveTimer = setInterval(tick, 4000);
  tick();
  renderMeters();
}
