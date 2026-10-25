// Tastatur an einer Stelle (Konzept neue Oberfläche §6). Einzeltasten wirken nicht, solange man
// in ein Feld tippt. Escape schliesst immer das zuletzt Geöffnete (Rückfrage, Menü, Auswahl …).

const bindings = new Map(); // Taste → { run, label }
const escapes = []; // Stapel: zuletzt geöffnet zuerst

/// Taste belegen; `label` erscheint in der Übersicht (?)
export function bind(key, run, label) {
  bindings.set(key, { run, label });
}

/// Etwas Geöffnetes meldet sich für Escape an; liefert die Funktion zum Abmelden
export function onEscape(close) {
  escapes.push(close);
  return () => {
    const i = escapes.lastIndexOf(close);
    if (i >= 0) escapes.splice(i, 1);
  };
}

export function keyList() {
  return [...bindings].filter(([, b]) => b.label).map(([key, b]) => ({ key, label: b.label }));
}

const typing = (el) => el && (el.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(el.tagName));

document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape" && escapes.length) {
    ev.preventDefault();
    escapes[escapes.length - 1]();
    return;
  }
  if (ev.ctrlKey || ev.metaKey || ev.altKey || typing(ev.target)) return;
  const b = bindings.get(ev.key);
  if (!b) return;
  ev.preventDefault();
  b.run(ev);
});
