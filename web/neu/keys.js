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
