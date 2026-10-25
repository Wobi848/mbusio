"use strict";

// Ausleseplan: Felder, Text eines Termins, speichern, Plan einer Linie (für Projekt und Zählerliste)

const PLAN_KINDS = ["off", "monthly", "daily", "hourly", "15min"];

/// Felder des Ausleseplans (Baustein Formular); `inherit` = Text für „wie übergeordnet“
function planFields(value, inherit, hint) {
  const [kind, time] = value ? value.split(" ") : [inherit ? "" : "off", null];
  const opts = (inherit ? [["", inherit]] : []).concat(PLAN_KINDS.map((k) => [k, t("plan_opt_" + k)]));
  return [
    { name: "plan_kind", label: t("plan"), type: "select", options: opts, value: kind },
    { name: "plan_time", label: t("plan_time"), type: "time24", cls: "mono time24", placeholder: "HH:MM", value: time || "00:05", hint },
  ];
}

/// Uhrzeit nur bei täglich und monatlich
function wirePlanField(root) {
  const sel = root.querySelector("[name=plan_kind]");
  const wrap = root.querySelector("[data-field=plan_time]");
  const update = () => (wrap.hidden = !["daily", "monthly"].includes(sel.value));
  sel.addEventListener("change", update);
  update();
}

/// Plan aus den Formularwerten; null = erben
function planFrom(values) {
  const k = values.plan_kind;
  if (!k) return null;
  return k === "daily" || k === "monthly" ? `${k} ${values.plan_time}` : k;
}

async function savePlan(path, plan) {
  const r = await api(path + "/plan", { method: "PUT", body: { plan } });
  if (r.warnings?.length) toast(t("plan_saved_warn", r.warnings.join(" ")), true);
  // geerbte Pläne der Zähler ändern sich mit
  await loadMeters().catch(() => {});
}

/// Zeile unter der Linie: Plan, nächster Termin, letztes Ergebnis, Störungen
function linePlanHtml(l) {
  const p = l.plan_info;
  const sw = `<label class="op-mini${l.operation ? " on" : ""}"><input type="checkbox" class="switch" data-ln-op="${l.id}"${l.operation ? " checked" : ""}>${esc(t("op_chip"))}</label>`;
  // ohne Plan: Schalter trotzdem da (führt zum Plan), damit man sieht, dass es ihn gibt
  if (!p || !p.meters) return `<div class="ln-plan off">${sw}<span>${esc(t("op_no_plan"))}</span></div>`;
  const parts = [`<b>${esc(p.effective !== "off" ? planText(p.effective) : t("plan_meters_own", p.meters))}</b>`];
  const svc = l.service;
  if (l.held) parts.push(ui.chip("muted", t("plan_held"), t("plan_held_title")));
  else if (svc && !svc.connected && svc.error) parts.push(ui.chip("warn", t("plan_offline"), svc.error));
  if (p.next && !l.held && l.operation) parts.push(esc(t("plan_next", slotText(p.next))));
  const r = p.result;
  if (r && r.time && !r.offline) {
    if (r.line_down) parts.push(ui.chip("warn", t("plan_line_down"), r.error || ""));
    else parts.push(ui.chip(r.ok === r.total ? "ok" : "warn", t("plan_last", r.ok, r.total), `${r.time}${r.error ? "\n" + r.error : ""}`));
  }
  if (p.disturbed) parts.push(ui.chip("warn", t("plan_disturbed", p.disturbed)));
  if (p.paused) parts.push(ui.chip("muted", t("plan_paused", p.paused), t("meter_paused_title")));
  if (p.load_warn) parts.push(ui.chip("warn", t("plan_load", Math.round(p.load * 100)), t("plan_load_title")));
  return `<div class="ln-plan">${sw}${ICON_CLOCK}${parts.join(" · ")}</div>`;
}
