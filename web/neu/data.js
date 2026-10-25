// Daten vom Server in den Zustand laden; von allen Bereichen benutzt

import { set, activeLine } from "./store.js";
import { toast } from "./view.js";

export async function loadStatus() {
  set({ status: await request("status") });
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
