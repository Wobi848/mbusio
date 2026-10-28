// Bereich Zähler: Liste vom Kunden importieren (Spalten zuordnen, Vorschau) und Namen nach Muster,
// als Feld im Detailbereich. `box` ist der Detailbereich; `done` lädt neu und schliesst das Feld.

import { toast, previewApply } from "./view.js";
import { report } from "./data.js";

let data = null; // erkannte Liste: { rows, header, cols, map }

/// Inhalt des Felds: „import“ (Liste) oder „names“ (Namen für `n` gewählte, 0 = alle)
export function panelHtml(kind, n) {
  const back = `<button type="button" class="quiet back" data-close-panel>← ${esc(t("tab_meters"))}</button>`;
  if (kind === "import") {
    data = null;
    return `${back}
      <header class="dhead"><b>${esc(t("list_title"))}</b></header><p class="sub">${esc(t("list_hint"))}</p>
      <textarea data-paste rows="5" class="mono" placeholder="${esc(t("list_paste_ph"))}" aria-label="${esc(t("zl_paste"))}"></textarea>
      <label class="quiet filebtn">${esc(t("zl_file"))}<input type="file" accept=".csv,.txt,text/csv" data-file hidden></label>
      <div data-map></div><div data-result></div>
      <div class="actions"><span class="grow"></span><button type="button" class="quiet" data-close-panel>${esc(t("zl_close"))}</button></div>`;
  }
  return `${back}
      <header class="dhead"><b>${esc(t("names_title"))}</b></header><p class="sub">${esc(t("names_hint"))}</p>
      <div class="fields"><label><span>${esc(t("names_pattern"))}</span><input data-pattern class="mono" value="{ort} {medium}"></label>
      <label class="check"><span>${esc(t("names_only_empty"))}</span><input type="checkbox" data-only-empty></label></div>
      <p class="sub">${esc(t("zl_names_scope", n))}</p>
      <div class="actions"><button type="button" data-names-preview>${esc(t("quick_names"))}</button><button type="button" class="quiet" data-close-panel>${esc(t("zl_close"))}</button></div>
      <div data-result></div>`;
}

/// Eingefügter oder geladener Text: Spalten erkennen und zur Zuordnung zeigen
export function listLoad(text_, box) {
  data = listLogic.guess(parseTable(text_));
  renderMap(box);
}

/// Änderung an der Zuordnung (Überschrift, Spalte); true, wenn sie hierher gehörte
export function listChange(el, box) {
  if (el.matches("[data-file]") && el.files[0]) {
    el.files[0].text().then((x) => listLoad(x, box));
    return true;
  }
  if (el.matches("[data-header]")) {
    data.header = el.checked;
    renderMap(box);
    return true;
  }
  if (!el.matches("[data-map-f]")) return false;
  const f = el.dataset.mapF;
  const v = el.value === "" ? undefined : Number(el.value);
  // eine Spalte gehört zu höchstens einem Feld
  for (const g of listLogic.FIELDS) if (data.map[g] === v && v !== undefined) delete data.map[g];
  data.map[f] = v;
  renderMap(box);
  return true;
}

function renderMap(box) {
  const out = box.querySelector("[data-map]");
  if (!out) return;
  if (!data) return (out.innerHTML = "");
  const d = data;
  const names = Array.from({ length: d.cols }, (_, i) => (d.header && d.rows[0][i]) || t("list_col", i + 1));
  out.innerHTML = `<h3>${esc(t("list_map"))} <span class="sub">${esc(t("list_rows", d.rows.length - (d.header ? 1 : 0)))}</span></h3>
    <div class="fields"><label class="check"><span>${esc(t("list_header"))}</span><input type="checkbox" data-header${d.header ? " checked" : ""}></label>
    ${listLogic.FIELDS.map(
      (f) => `<label><span>${esc(t("list_f_" + f))}</span><select data-map-f="${f}"><option value="">${esc(t("list_unused"))}</option>${names
        .map((n_, i) => `<option value="${i}"${d.map[f] === i ? " selected" : ""}>${esc(n_)}</option>`)
        .join("")}</select></label>`
    ).join("")}</div>
    <div class="actions"><button type="button" data-list-check>${esc(t("list_check"))}</button></div>`;
}

/// Liste prüfen: Vorschau der Änderungen, dann übernehmen
export async function listCheck(box, done) {
  if (data?.map.id === undefined) return toast(t("list_need_id"), { error: true });
  const rows = listLogic.rows(data);
  await previewApply(box.querySelector("[data-result]"), {
    title: t("list_check"),
    run: (apply) => request("meters/import-list", { method: "POST", body: { rows, apply } }),
    done: async (cs) => {
      if (!cs) return;
      toast(t("list_done", cs.added.length, cs.changed.length));
      await done();
    },
  }).catch(report);
}

/// Namen nach Muster für die gewählten Zähler (`ids` leer = alle): Vorschau, dann übernehmen
export async function namesPreview(box, ids, done) {
  const body = { pattern: box.querySelector("[data-pattern]").value, only_empty: box.querySelector("[data-only-empty]").checked };
  if (ids.length) body.ids = ids;
  await previewApply(box.querySelector("[data-result]"), {
    title: t("names_title"),
    run: (apply) => request("meters/names", { method: "POST", body: { ...body, apply } }),
    done: async (cs) => {
      if (!cs) return;
      toast(t("names_done", cs.changed.length));
      await done();
    },
  }).catch(report);
}
