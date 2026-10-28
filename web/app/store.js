// Zustand der Oberfläche an einer Stelle. Teile melden sich für die Felder an, die sie zeigen,
// und werden nur benachrichtigt, wenn sich genau diese geändert haben (Konzept neue Oberfläche §5.2).

export const state = {
  status: null, // /api/status
  tree: null, // /api/projects
  ports: null, // /api/ports, erst beim Öffnen des Anschluss-Formulars
  meters: null, // /api/meters der aktiven Linie
  job: null, // /api/job, solange ein Auftrag läuft
  scanNew: null, // { line, n }: im Scan gefunden, aber noch nicht in der Zählerliste
  area: "zaehler", // Bereich aus der Adresse (#/zaehler)
  offline: false, // mbusio antwortet nicht (letzte Statusabfrage gescheitert)
};

const listeners = new Map(); // Feld → Set von Funktionen

/// Felder setzen; benachrichtigt wird nur, wer ein geändertes Feld zeigt (Vergleich als JSON,
/// die Daten kommen ohnehin als JSON vom Server)
export function set(changes) {
  const changed = Object.keys(changes).filter((k) => JSON.stringify(state[k]) !== JSON.stringify(changes[k]));
  Object.assign(state, changes);
  const due = new Set(changed.flatMap((k) => [...(listeners.get(k) || [])]));
  for (const fn of due) fn(state);
}

/// `fn` bei Änderung eines der Felder aufrufen; gleich einmal mit dem aktuellen Zustand
export function watch(fields, fn) {
  for (const f of fields) {
    if (!listeners.has(f)) listeners.set(f, new Set());
    listeners.get(f).add(fn);
  }
  fn(state);
}

/// Aktive Linie mit allem aus dem Projektbaum (Plan, Dauerbetrieb, Zähler, Datenpunkte)
export function activeLine() {
  const id = state.status?.active?.line?.id;
  for (const p of state.tree?.projects || []) for (const g of p.gateways) for (const l of g.lines) if (l.id === id) return l;
  return null;
}

/// Gestört: antwortet nicht, im Plan mehrmals nicht gelesen oder meldet selbst einen Fehler (eine
/// schwache Batterie zählt nicht). Eine Regel für Filter „Gestört“, „Nächster Schritt“ und Anlage
export const faulty = (m) => !!(m.last_error || m.plan_info?.disturbed || (m.fault && m.fault !== "power_low"));

/// Alle Linien im Dauerbetrieb über alle Projekte: was dieses Gerät im Hintergrund tut
export function operatingLines() {
  return (state.tree?.projects || []).flatMap((p) => p.gateways.flatMap((g) => g.lines.filter((l) => l.operation).map((l) => ({ p, g, l }))));
}
