// Daten vom Server in den Zustand laden; von allen Bereichen benutzt

import { state, set, activeLine } from "./store.js";
import { toast, ask } from "./view.js";

export async function loadStatus() {
  const before = state.status?.changes;
  set({ status: await request("status") });
  // Daten im Hintergrund geändert (Plan, anderes Fenster): Zählerliste nachführen
  if (before != null && state.status.changes !== before && state.meters) loadMeters().catch(() => {});
}

/// Regelmässige Statusabfrage; antwortet mbusio nicht (beendet, Pi neu gestartet), zeigen Kopf und
/// Statusleiste das, statt beim letzten Stand „Verbunden“ stehen zu bleiben
export const pollStatus = () => loadStatus().then(() => set({ offline: false }), () => set({ offline: true }));

/// Dauerbetrieb einer Linie schalten (Schalter in Kopfzeile oder Anlage); aus nur nach Rückfrage
/// am Ort. Der Schalter zeigt erst den echten Zustand, wenn der Server zugestimmt hat
export async function switchOperation(sw, l) {
  const on = sw.checked;
  sw.checked = !on;
  if (!on && !(await ask(sw, t("n_op_off_q", l.name), t("n_switch_off")))) return;
  try {
    await request(`lines/${l.id}/operation`, { method: "PUT", body: { on } });
    await loadTree();
    toast(t(on ? "op_switched_on" : "op_switched_off", l.name));
  } catch (e) {
    report(e);
  }
}

export async function loadTree() {
  set({ tree: await request("projects") });
}

export async function loadMeters() {
  set({ meters: await request("meters") });
}

/// Nach einer Aktion: Status und Projektbaum gleich neu, ohne auf die Abfrage zu warten
export const reload = () => Promise.all([loadStatus(), loadTree()]);

/// Linie wechseln ohne Neuladen der Seite; Bereiche und ihre Einstellungen bleiben
export async function useLine(id) {
  const r = await request(`lines/${id}/activate`, { method: "POST" });
  await reload();
  if (r.error) toast(t("activate_failed", activeLine()?.name || "", r.error), { error: true });
}

/// Fehler einer Aktion als Meldung statt still zu verschwinden
export const report = (e) => toast(e.message, { error: true });
