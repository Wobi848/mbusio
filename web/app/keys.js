// Tastatur an einer Stelle (Konzept neue Oberfläche §6). Einzeltasten wirken nicht, solange man
// in ein Feld tippt. Escape schliesst immer das zuletzt Geöffnete (Rückfrage, Menü, Auswahl …).

const bindings = new Map(); // Taste → [{ run, label, when }]
const escapes = []; // Stapel: zuletzt geöffnet zuerst

/// Taste belegen; `label` erscheint in der Übersicht (?). `when` (z.B. „Bereich Zähler offen“)
/// erlaubt, dieselbe Taste je Bereich anders zu belegen.
export function bind(key, run, label, when = () => true) {
  if (!bindings.has(key)) bindings.set(key, []);
  bindings.get(key).push({ run, label, when });
}

/// Etwas Geöffnetes meldet sich für Escape an; liefert die Funktion zum Abmelden
export function onEscape(close) {
  escapes.push(close);
  return () => {
    const i = escapes.lastIndexOf(close);
    if (i >= 0) escapes.splice(i, 1);
  };
}

/// Tasten einer Liste mit Mehrfachauswahl (Zähler, Weitergabe): ↑↓ wählt (mit Umschalt auch
/// markieren), Leertaste markiert, Strg+A markiert alle sichtbaren. `rows`: sichtbare Einträge mit
/// `id`; `view`: { active, selected }. true, wenn die Taste verbraucht ist.
export function selectionKeys(ev, { rows, view, select, toggle, render }) {
  if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    const i = rows.findIndex((x) => x.id === view.active);
    const next = rows[Math.max(0, Math.min(rows.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))];
    if (!next) return true;
    if (ev.shiftKey && view.active != null) {
      view.selected.add(view.active);
      view.selected.add(next.id);
    }
    select(next.id);
  } else if (ev.key === " " && view.active != null) {
    ev.preventDefault();
    toggle(view.active);
  } else if (ev.key.toLowerCase() === "a" && (ev.ctrlKey || ev.metaKey)) {
    ev.preventDefault();
    for (const x of rows) view.selected.add(x.id);
    render();
  } else return false;
  return true;
}

/// Tasten, die gerade wirken, für die Übersicht
export function keyList() {
  return [...bindings].flatMap(([key, list]) => list.filter((b) => b.label && b.when()).map((b) => ({ key, label: b.label })));
}

const typing = (el) => el && (el.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(el.tagName));

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape" && escapes.length) {
    ev.preventDefault();
    escapes[escapes.length - 1]();
    return;
  }
  if (ev.ctrlKey || ev.metaKey || ev.altKey || typing(ev.target)) return;
  const b = (bindings.get(ev.key.length === 1 ? ev.key.toLowerCase() : ev.key) || []).find((x) => x.when());
  if (!b) return;
  ev.preventDefault();
  b.run(ev);
});
