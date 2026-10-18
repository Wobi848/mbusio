"use strict";

// Statusleiste unten: zeigt überall, ob der Bus beschäftigt ist; im Ruhezustand Verbindung,
// Uhrzeit, Speicher, MQTT und Auffälligkeiten

let busbarTimer = null;
/// laufende Aktion oder kurz stehende Fertigmeldung; null = Ruhezustand
let busbarState = null;
/// Anzahl Auffälligkeiten der letzten Suche; null = noch keine Suche
let issueCount = null;

/// `null` = Ruhezustand; sonst { text, frac (0..1, ohne = unbestimmt), link, cancel, done, error }
function busbar(state) {
  clearTimeout(busbarTimer);
  busbarState = state;
  renderBusbar();
  // fertig: noch kurz stehen lassen, dann zurück in den Ruhezustand
  if (state?.done) busbarTimer = setTimeout(() => busbar(null), state.error ? 10000 : 6000);
}

/// Die Leiste ist immer da: im Ruhezustand Verbindung bzw. "Nicht verbunden"
function renderBusbar() {
  const el = $("busbar");
  const run = planRuns[0];
  const state =
    busbarState ||
    (run
      ? { text: t("plan_running", run.label, run.done, run.total), frac: run.total ? run.done / run.total : 0, idle: true }
      : {
          text: connected && conn ? t("idle_ready", conn.port, conn.baud) : t("idle_off"),
          idle: true,
          off: !connected,
        });
  // Uhrzeit nur ein Thema, wenn geplant ausgelesen wird
  const tm = $("busbar-time");
  tm.hidden = !(planActive && timeState && !timeState.confirmed);
  // Speicher: "knapp" nur im Dauerbetrieb wichtig, "voll" immer
  const mq = $("busbar-mqtt");
  mq.hidden = mqttStatus?.state !== "error";
  if (!mq.hidden) mq.textContent = t("mqtt_busbar", mqttStatus.queued);
  const dk = $("busbar-disk");
  const level = disk?.level || "ok";
  dk.hidden = level === "ok" || (level === "low" && !planActive);
  if (!dk.hidden) {
    const pct = Math.round(disk.free_pct);
    dk.textContent = level === "full" ? t("disk_full", pct) : t("disk_low", pct);
    dk.classList.toggle("full", level === "full");
  }
  el.hidden = false;
  document.body.classList.add("has-busbar");
  el.classList.toggle("idle", !!state.idle);
  el.classList.toggle("off", !!state.off);
  el.classList.toggle("done", !!state.done && !state.error);
  el.classList.toggle("error", !!state.error);
  $("busbar-text").textContent = state.text;
  const bar = el.querySelector(".busbar-progress");
  bar.hidden = state.frac === undefined;
  if (state.frac !== undefined) bar.querySelector(".bar-fill").style.transform = `scaleX(${state.frac})`;
  const link = $("busbar-link");
  link.hidden = state.idle && !state.off;
  link.href = state.off ? "#sec-conn" : state.link || "#sec-scan";
  link.textContent = t(state.off ? "idle_connect" : "busbar_view");
  $("busbar-cancel").hidden = !state.cancel;
  const issues = $("busbar-issues");
  issues.hidden = issueCount === null;
  issues.className = "busbar-issues " + (issueCount ? "warn" : "ok");
  issues.textContent = issueCount ? t("issues", issueCount) : t("issues_none");
  issues.href = issueCount ? "#hits-warn" : "#hits";
}
