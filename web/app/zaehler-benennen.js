// Bereich Zähler: Benennen direkt in der Liste (Name und Ort als Felder in den Zeilen). Enter springt
// zum gleichen Feld beim nächsten Zähler (Shift+Enter: vorheriger), „wie oben“ übernimmt den Ort des
// Zählers darüber. Gespeichert wird beim Verlassen eines Felds.

import { report } from "./data.js";

/// Felder der Zeile im Benennen-Modus
export function namingCells(m) {
  return {
    name: `<input class="quick" data-q="name" data-id="${m.id}" value="${esc(m.name)}" placeholder="${esc(t("meter_name"))}" aria-label="${esc(t("meter_name"))}">`,
    location: `<span class="quick-loc"><input class="quick" data-q="location" data-id="${m.id}" value="${esc(m.location)}" list="zl-locations" placeholder="${esc(
      t("meter_location")
    )}" aria-label="${esc(t("meter_location"))}"><button type="button" class="icon" data-q-same="${m.id}" title="${esc(t("zl_same"))}" aria-label="${esc(t("zl_same"))}">↓</button></span>`,
  };
}

/// Vorschläge für den Ort: alle Orte der Linie
export function locationsHtml(meters) {
  return [...new Set(meters.map((m) => m.location).filter(Boolean))]
    .sort()
    .map((l) => `<option value="${esc(l)}">`)
    .join("");
}

/// Feld speichern; der Zähler in der Liste wird gleich nachgeführt, ohne die Liste neu zu laden
/// (sonst ginge das Feld verloren, in dem gerade getippt wird)
export async function saveQuick(input, meters) {
  const m = meters.find((x) => x.id === Number(input.dataset.id));
  const field = input.dataset.q;
  const value = input.value.trim();
  if (!m || (m[field] || "") === value) return;
  try {
    Object.assign(m, await request(`meters/${m.id}`, { method: "PUT", body: { ...m, [field]: value } }));
    input.classList.add("saved");
    setTimeout(() => input.classList.remove("saved"), 900);
  } catch (e) {
    input.value = m[field] || "";
    report(e);
  }
}

/// Ort vom Zähler darüber (in der gezeigten Reihenfolge)
export function sameAsAbove(button, rows, meters) {
  const id = Number(button.dataset.qSame);
  const k = rows.findIndex((m) => m.id === id);
  if (k < 1) return;
  const input = button.parentElement.querySelector("input");
  input.value = rows[k - 1].location || "";
  saveQuick(input, meters);
}

/// Enter: speichern und zum gleichen Feld beim nächsten Zähler
export function quickEnter(ev, list, meters) {
  const el = ev.target;
  ev.preventDefault();
  const all = [...list.querySelectorAll(`input[data-q=${el.dataset.q}]`)];
  const next = all[all.indexOf(el) + (ev.shiftKey ? -1 : 1)];
  saveQuick(el, meters);
  if (!next) return el.blur();
  next.focus();
  next.select();
}
