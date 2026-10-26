// Bereich Zähler: Darstellung (Zeile der Liste, Details eines Zählers, neuer Zähler). Reine
// Bausteine; gespeichert und gelesen wird in zaehler.js.

import { historyHtml } from "./zaehler-verlauf.js";

/// Pläne zur Auswahl je Zähler (dazu „wie Linie“)
export const PLANS = ["off", "daily 00:05", "hourly", "15min", "monthly 00:05"];

export function rowHtml(m, { active, selected, busy }) {
  const sel = selected.has(m.id);
  const b = meterBlock(m);
  const ident = [b.ident, b.pa != null && b.pa !== "" ? `PA ${b.pa}` : ""].filter(Boolean).join(" · ");
  const state_ = busy.has(m.id)
    ? `<span class="chip">${esc(t("reading_busy"))}</span>`
    : m.swap
      ? `<span class="chip warn" title="${esc(t("zl_swap_title", m.swap.old_id || "?", m.swap.new_id || "?"))}">${esc(t("zl_swap"))}</span>`
      : m.last_error
      ? `<span class="chip err" title="${esc(m.last_error)}">${esc(t("n_no_answer"))}</span>`
      : m.plan_info?.disturbed
        ? `<span class="chip warn">${esc(t("meter_disturbed"))}</span>`
        : m.fault
          ? `<span class="chip ${m.fault === "power_low" ? "warn" : "err"}" title="${esc(t("meter_fault_title_" + m.fault))}">${esc(t("meter_fault_" + m.fault))}</span>`
          : m.clock_off != null
          ? `<span class="chip warn" title="${esc(t("meter_clock_title", m.clock_off))}">${esc(t("meter_clock", m.clock_off))}</span>`
          : m.plan_info?.effective && m.plan_info.effective !== "off"
            ? `<span class="sub">${esc(planText(m.plan_info.effective))}</span>`
            : m.plan === "off"
              ? `<span class="sub" title="${esc(t("plan"))}">${esc(planText("off"))}</span>`
              : "";
  const value = m.last_value
    ? `<b class="mono">${esc(m.last_value)}</b> <span class="sub">${esc(slotText(m.last_read))}</span>`
    : `<span class="sub">${esc(t("n_never_read"))}</span>`;
  return `<div class="row${m.id === active ? " active" : ""}" role="option" id="meter-${m.id}" aria-selected="${sel}">
    <span class="box" data-box aria-hidden="true">${sel ? "✓" : ""}</span>
    <span class="name">${mediumIcon(m.medium, true)}${m.name ? esc(m.name) : `<span class="sub">${esc(t("meter_unnamed"))}</span>`}</span>
    <span class="mono sub">${esc(ident)}</span>
    <span class="cut">${esc(m.location)}</span>
    <span class="cut">${value}</span>
    <span>${state_}</span>
  </div>`;
}

/// Details: Felder, Plan, Aktionen, Tausch, Verlauf, Werte der letzten Auslesung (`vals` null =
/// noch am Laden)
export function detailHtml(m, { vals, busy, history, exchange, linePlan }) {
  const b = meterBlock(m);
  const plans = [["", t("plan_inherit_line", planText(linePlan))], ...PLANS.map((p) => [p, planText(p)])];
  if (m.plan && !PLANS.includes(m.plan)) plans.push([m.plan, planText(m.plan)]);
  const info = [b.ident ? `ID ${b.ident}` : "", b.pa != null && b.pa !== "" ? `PA ${b.pa}` : "", b.man, m.medium ? label(m.medium) : "", b.info].filter(Boolean).join(" · ");
  return `<button type="button" class="quiet back" data-back>← ${esc(t("tab_meters"))}</button>
    <header class="dhead">${mediumIcon(m.medium)}<div><b>${m.name ? esc(m.name) : esc(t("meter_unnamed"))}</b><div class="sub mono">${esc(info)}</div></div></header>
    ${
      m.swap
        ? `<div class="chip warn wide swap">${esc(t("zl_swap_detail", slotText(m.swap.time), m.swap.old_id || "?", m.swap.new_id || "?"))}
            <button type="button" class="tonal" data-swap-confirm="${m.swap.id}">${esc(t("zl_swap_confirm"))}</button></div>`
        : ""
    }
    ${m.last_error ? `<p class="chip err wide">${esc(m.last_error)}</p>` : ""}
    ${m.fault ? `<p class="chip ${m.fault === "power_low" ? "warn" : "err"} wide">${esc(t("meter_fault_title_" + m.fault))}</p>` : ""}
    ${m.clock_off != null ? `<p class="chip warn wide" title="${esc(t("meter_clock_title", m.clock_off))}">${esc(t("meter_clock", m.clock_off))}</p>` : ""}
    <div class="fields">
      ${field("name", t("meter_name"), m.name, t("meter_name_ph"))}
      ${field("location", t("meter_location"), m.location, t("meter_location_ph"))}
      ${field("note", t("meter_note"), m.note, "")}
      <label><span>${esc(t("plan"))}</span><select data-plan>${plans
        .map(([v, txt]) => `<option value="${esc(v)}"${(m.plan || "") === v ? " selected" : ""}>${esc(txt)}</option>`)
        .join("")}</select></label>
    </div>
    <div class="actions">
      <button type="button" data-read title="${esc(t("read"))} (R)">${esc(busy ? t("reading_busy") : t("read"))}</button>
      <button type="button" class="${history ? "tonal" : "quiet"}" data-history aria-pressed="${history}" title="${esc(t("zl_history"))} (V)">${esc(t("zl_history"))}</button>
      <button type="button" class="quiet" data-exchange aria-pressed="${exchange}">${esc(t("zl_exchange"))}</button>
      <span class="grow"></span>
      <button type="button" class="quiet danger-text" data-del>${esc(t("delete"))}</button>
    </div>
    ${
      exchange
        ? `<div class="fields inset"><p class="sub">${esc(t("exchange_title"))}</p><label><span>${esc(t("address"))}</span><input data-exchange-addr class="mono" placeholder="${esc(
            t("address_ph")
          )}" aria-label="${esc(t("zl_exchange_addr"))}"></label><div class="actions"><button type="button" data-exchange-go>${esc(t("zl_exchange_go"))}</button></div></div>`
        : ""
    }
    ${history ? historyHtml(m) : ""}
    <h3>${esc(t("n_last_reading"))} <span class="sub">${esc(m.last_read ? slotText(m.last_read) : t("n_never_read"))}</span></h3>
    ${
      vals
        ? vals.length
          ? `<div class="vals"><div class="vrow head"><span>${esc(t("history_series"))}</span><span></span><span>${esc(t("n_as_dp"))}</span></div>${vals
              .map(
                (v) =>
                  `<label class="vrow${v.main ? " main" : ""}"><span>${esc(v.label)}</span><span class="mono">${esc(v.value ?? "–")} ${esc(v.unit)}</span><input type="checkbox" data-dp="${esc(
                    v.key
                  )}"${v.selected ? " checked" : ""}></label>`
              )
              .join("")}</div>`
          : `<p class="sub">${esc(t("history_none"))}</p>`
        : `<p class="sub">…</p>`
    }`;
}

const field = (name, label_, value, ph) =>
  `<label><span>${esc(label_)}</span><input data-field="${name}" value="${esc(value)}" placeholder="${esc(ph)}" data-saved="${esc(value)}"></label>`;

export function addFormHtml() {
  return `<header class="dhead"><b>${esc(t("meter_add"))}</b></header>
    <div class="fields">
      <label><span>${esc(t("address"))}</span><input data-new="address" class="mono" placeholder="${esc(t("address_ph"))}" autofocus></label>
      <label><span>${esc(t("meter_name"))}</span><input data-new="name" placeholder="${esc(t("meter_name_ph"))}"></label>
      <label><span>${esc(t("meter_location"))}</span><input data-new="location" placeholder="${esc(t("meter_location_ph"))}"></label>
    </div>
    <div class="actions"><button type="button" data-save-new>${esc(t("remember"))}</button><button type="button" class="quiet" data-cancel-new>${esc(t("cancel"))}</button></div>`;
}
