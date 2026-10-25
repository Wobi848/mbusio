"use strict";

// Zählerliste schnell füllen: Liste importieren, Benennen, Namen erzeugen

// ------------------------------------------------------------ Zählerliste schnell füllen

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
  listData = listLogic.guess(rows);
  renderListMap();
}

function renderListMap() {
  const d = listData;
  const names = Array.from({ length: d.cols }, (_, i) => (d.header && d.rows[0][i]) || t("list_col", i + 1));
  const data = d.rows.slice(d.header ? 1 : 0);
  $("list-map").innerHTML = `<h3 class="list-h">${esc(t("list_map"))} <span class="hint">${esc(t("list_rows", data.length))}</span></h3>
    <label class="check"><input type="checkbox" id="list-header" ${d.header ? "checked" : ""}> ${esc(t("list_header"))}</label>
    <div class="form-grid list-grid">${listLogic.FIELDS.map(
      (f) => `<label><span>${esc(t("list_f_" + f))}</span><select data-list-f="${f}"><option value="">${esc(t("list_unused"))}</option>${names
        .map((n, i) => `<option value="${i}" ${d.map[f] === i ? "selected" : ""}>${esc(n)}</option>`)
        .join("")}</select></label>`
    ).join("")}</div>
    <div class="table-wrap"><table class="list-preview"><thead><tr>${listLogic.FIELDS.map((f) => `<th>${esc(t("list_f_" + f))}</th>`).join("")}</tr></thead>
    <tbody>${data
      .slice(0, 4)
      .map((r) => `<tr>${listLogic.FIELDS.map((f) => `<td>${esc(d.map[f] !== undefined ? r[d.map[f]] || "" : "")}</td>`).join("")}</tr>`)
      .join("")}</tbody></table></div>
    <div class="row"><button type="button" id="list-check">${esc(t("list_check"))}</button></div>`;
}

const listRows = () => listLogic.rows(listData);

/// Liste prüfen (Vorschau), dann übernehmen; Baustein „Vorschau → Übernehmen“
function listCheck() {
  if (listData?.map.id === undefined) throw new Error(t("list_need_id"));
  const rows = listRows();
  return ui.previewApply($("list-result"), {
    title: t("list_check"),
    run: (apply) => api("meters/import-list", { method: "POST", body: { rows, apply } }),
    done: async (cs) => {
      toast(t("list_done", cs.added.length, cs.changed.length));
      $("list-panel").hidden = true;
      $("list-paste").value = "";
      listData = null;
      $("list-map").innerHTML = "";
      await loadMeters();
    },
  });
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
    for (const f of listLogic.FIELDS) if (listData.map[f] === v && v !== undefined) delete listData.map[f];
    listData.map[el.dataset.listF] = v;
  }
  $("list-result").innerHTML = "";
  renderListMap();
});
$("list-panel").addEventListener("click", (ev) => {
  if (ev.target.id === "list-check") guarded(ev.target, listCheck);
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

// Namen nach Muster (Vorschau → Übernehmen wie überall)
function renderNames() {
  $("names-panel").innerHTML = ui.panel(
    t("names_title"),
    `<p class="hint">${esc(t("names_hint"))}</p>
    <div class="row names-row"><label class="grow"><span>${esc(t("names_pattern"))}</span> <input id="names-pattern" class="mono" value="{ort} {medium}"></label>
      <label class="check"><input type="checkbox" id="names-only"> ${esc(t("names_only_empty"))}</label>
      <button type="button" class="small tonal" id="names-preview">${esc(t("quick_names"))}</button></div>
    <div id="names-result" hidden></div>`,
    { close: false, actions: ui.iconBtn(ICON_DEL, t("names_close"), { "names-close": "" }) }
  );
}

$("names-panel").addEventListener("click", (ev) => {
  // × beendet das Benennen (wie „Fertig“)
  if (ev.target.closest("[data-names-close]")) return $("quick-toggle").click();
  if (ev.target.id !== "names-preview") return;
  const body = { pattern: $("names-pattern").value, only_empty: $("names-only").checked };
  guarded(ev.target, () =>
    ui.previewApply($("names-result"), {
      title: t("names_title"),
      run: (apply) => api("meters/names", { method: "POST", body: { ...body, apply } }),
      done: async (cs) => {
        toast(t("names_done", cs.changed.length));
        await loadMeters();
      },
    })
  );
});
