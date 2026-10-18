"use strict";

// Simulator-Reiter

// ------------------------------------------------------------ Simulator

let sim = null;
let simKey = null;
/// eigene Simulation in Bearbeitung: { name, meters }
let simDraft = null;
let simDirty = false;

async function loadSim() {
  sim = await api("sim");
  const conn = sim.line.connection || "";
  const used = conn === "SIMULATOR" ? "praxis" : conn.startsWith("SIMULATOR:") ? conn.slice(10) : null;
  if (!simKey || !sim.scenarios.some((x) => x.key === simKey)) simKey = used || sim.scenarios[0].key;
  selectSim(simKey, true);
}

const simScenario = () => sim.scenarios.find((x) => x.key === simKey);
const simConn = (key) => (key === "praxis" ? "SIMULATOR" : `SIMULATOR:${key}`);

function selectSim(key, force = false) {
  if (!force && simDirty && !confirm(t("confirm_sim_discard"))) {
    $("sim-select").value = simKey;
    return;
  }
  simKey = key;
  const s = simScenario();
  simDraft = s.builtin ? null : { name: s.name, meters: s.meters.map((m) => ({ ...m })) };
  simDirty = false;
  renderSim();
}

function renderSim() {
  const s = simScenario();
  $("sim-select").innerHTML = sim.scenarios
    .map((x) => `<option value="${esc(x.key)}" ${x.key === simKey ? "selected" : ""}>${esc(x.name)}${x.builtin ? ` (${esc(t("sim_template"))})` : ""}</option>`)
    .join("");
  $("sim-delete").hidden = s.builtin;
  const inUse = sim.line.connection === simConn(s.key);
  $("sim-line").innerHTML = `${esc(t("sim_line", sim.line.name, sim.line.connection))}${inUse ? ` · <b>${esc(t("sim_line_uses"))}</b>` : ""}`;
  $("sim-use").disabled = inUse && connected;
  const meters = simDraft ? simDraft.meters : s.meters;
  const ro = !simDraft;
  const opt = (list, val, text) => list.map((v) => `<option value="${esc(v)}" ${String(v) === String(val) ? "selected" : ""}>${esc(text(v))}</option>`).join("");
  const rows = meters
    .map((m, i) =>
      ro
        ? `<tr><td class="num mono">${m.primary}</td><td class="mono">${esc(m.id)}</td><td class="mono">${esc(m.manufacturer)}</td>
            <td>${esc(t("sim_m_" + m.medium))}</td><td class="mono">${m.baud ?? esc(t("sim_baud_bus"))}</td>
            <td>${m.fault === "none" ? esc(t("sim_f_none")) : ui.chip("warn", t("sim_f_" + m.fault))}</td><td></td></tr>`
        : `<tr data-i="${i}">
            <td><input class="mono sim-pa" data-f="primary" value="${m.primary}" inputmode="numeric" aria-label="PA"></td>
            <td><input class="mono sim-id" data-f="id" value="${esc(m.id)}" inputmode="numeric" maxlength="8" aria-label="ID"></td>
            <td><input class="mono sim-man" data-f="manufacturer" value="${esc(m.manufacturer)}" maxlength="3" aria-label="${esc(t("sim_col_man"))}"></td>
            <td><select data-f="medium">${opt(sim.media, m.medium, (v) => t("sim_m_" + v))}</select></td>
            <td><select data-f="baud"><option value="">${esc(t("sim_baud_bus"))}</option>${opt(BAUDS, m.baud ?? "", String)}</select></td>
            <td><select data-f="fault">${opt(sim.faults, m.fault, (v) => t("sim_f_" + v))}</select></td>
            <td><button class="icon-btn del" data-sim-del="${i}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button></td>
          </tr>`
    )
    .join("");
  const faults = meters.filter((m) => m.fault !== "none").length;
  const warn = simWarnings(meters);
  $("sim-editor").innerHTML = `
    ${ro ? `<p class="hint">${esc(t("sim_builtin_note"))}</p>` : `<div class="row"><label class="grow"><span>${esc(t("sim_name"))}</span> <input id="sim-name" value="${esc(simDraft.name)}"></label></div>`}
    <p class="hint"><b>${esc(t("sim_count", meters.length, faults))}</b></p>
    ${warn.length ? `<ul class="sim-warn">${warn.map((w) => `<li>${esc(w)}</li>`).join("")}</ul>` : ""}
    ${
      meters.length
        ? `<div class="table-wrap"><table class="sim-table"><thead><tr><th class="num">${esc(t("sim_col_pa"))}</th><th>${esc(t("sim_col_id"))}</th><th>${esc(
            t("sim_col_man")
          )}</th><th>${esc(t("sim_col_medium"))}</th><th>${esc(t("sim_col_baud"))}</th><th>${esc(t("sim_col_fault"))}</th><th></th></tr></thead><tbody>${rows}</tbody></table></div>`
        : `<p class="empty">${esc(t("sim_empty"))}</p>`
    }
    ${
      ro
        ? ""
        : `<div class="row sim-actions">
        <button type="button" class="small ghost" data-sim-add>${esc(t("sim_add"))}</button>
        <span class="sim-many"><input id="sim-n" class="mono" value="10" inputmode="numeric" aria-label="${esc(t("sim_add_n"))}">
          <select id="sim-n-medium">${opt(sim.media, "water", (v) => t("sim_m_" + v))}</select>
          <button type="button" class="small ghost" data-sim-add-many>${esc(t("sim_add_many"))}</button></span>
        <span class="spacer"></span>
        <button type="button" data-sim-save ${simDirty ? "" : "disabled"}>${esc(t("sim_save"))}</button></div>`
    }`;
}

/// Hinweise, die in der Praxis zu Fehlern führen (manchmal gewollt)
function simWarnings(meters) {
  const out = [];
  const count = (key) => meters.reduce((m, x) => m.set(x[key], (m.get(x[key]) || 0) + 1), new Map());
  for (const [pa, n] of count("primary")) if (n > 1 && Number(pa) !== 0) out.push(t("sim_dup_pa", pa));
  for (const [id, n] of count("id")) if (n > 1) out.push(t("sim_dup_id", id));
  const zero = meters.filter((m) => Number(m.primary) === 0).length;
  if (zero) out.push(t("sim_pa0", zero));
  return out;
}

const SIM_MAN = { heat: "KAM", cooling: "KAM", heat_old: "KAM", electricity: "ABB", gas: "ELS" };

/// nächster freier Zähler: PA nach der grössten, ID fortlaufend
function simNext(medium) {
  const ms = simDraft.meters;
  const pa = Math.min(250, ms.reduce((m, x) => Math.max(m, Number(x.primary) || 0), 0) + 1);
  const ids = new Set(ms.map((m) => m.id));
  let id = ms.reduce((m, x) => Math.max(m, Number(x.id) || 0), 10000000) + 1;
  while (ids.has(String(id).padStart(8, "0"))) id++;
  return { primary: pa, id: String(id).padStart(8, "0"), manufacturer: SIM_MAN[medium] || "SEN", version: 1, medium, baud: null, fault: "none" };
}

function simChanged() {
  simDirty = true;
  renderSim();
}

$("sim-select").onchange = () => selectSim($("sim-select").value);

$("sim-editor").addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.id === "sim-name") {
    simDraft.name = el.value;
    return simChanged();
  }
  const row = el.closest("tr[data-i]");
  if (!row || !el.dataset.f) return;
  const m = simDraft.meters[Number(row.dataset.i)];
  const f = el.dataset.f;
  if (f === "primary") m.primary = Math.max(0, Math.min(250, Number(el.value) || 0));
  else if (f === "baud") m.baud = el.value ? Number(el.value) : null;
  else if (f === "manufacturer") m.manufacturer = el.value.trim().toUpperCase();
  else if (f === "id") m.id = el.value.trim().padStart(8, "0");
  else m[f] = el.value;
  simChanged();
});

$("sim-editor").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.simDel !== undefined) {
    simDraft.meters.splice(Number(b.dataset.simDel), 1);
    return simChanged();
  }
  if (b.dataset.simAdd !== undefined) {
    simDraft.meters.push(simNext("water"));
    return simChanged();
  }
  if (b.dataset.simAddMany !== undefined) {
    const n = Math.max(1, Math.min(100, Number($("sim-n").value) || 1));
    const medium = $("sim-n-medium").value;
    for (let k = 0; k < n; k++) simDraft.meters.push(simNext(medium));
    return simChanged();
  }
  if (b.dataset.simSave !== undefined) {
    guarded(b, async () => {
      const r = await api("sim/" + encodeURIComponent(simKey), { method: "PUT", body: { name: simDraft.name, meters: simDraft.meters } });
      simDirty = false;
      toast(t("sim_saved", r.restarted));
      await loadSim();
      refreshStatus().catch(() => {});
    });
  }
});

$("sim-use").onclick = () =>
  guarded($("sim-use"), async () => {
    if (simDirty && !confirm(t("confirm_sim_discard"))) return;
    const r = await api(`sim/${encodeURIComponent(simKey)}/use`, { method: "POST" });
    toast(t("sim_used", r.connection));
    await loadSim();
    await refreshStatus();
    loadTree().catch(() => {});
  });

async function simCreate(name, from) {
  if (simDirty && !confirm(t("confirm_sim_discard"))) return;
  const r = await api("sim", { method: "POST", body: { name, from } });
  simDirty = false;
  simKey = r.key;
  await loadSim();
  loadPorts().catch(() => {});
}

$("sim-copy").onclick = () =>
  guarded($("sim-copy"), () => {
    const name = prompt(t("sim_new_prompt"), t("sim_copy_name", simScenario().name));
    if (name && name.trim()) return simCreate(name.trim(), simKey);
  });

$("sim-new").onclick = () =>
  guarded($("sim-new"), () => {
    const name = prompt(t("sim_new_prompt"), t("sim_new_name"));
    if (name && name.trim()) return simCreate(name.trim(), null);
  });

$("sim-delete").onclick = () => {
  const s = simScenario();
  if (!confirm(t("confirm_sim_delete", s.name))) return;
  guarded($("sim-delete"), async () => {
    await api("sim/" + encodeURIComponent(s.key), { method: "DELETE" });
    simKey = null;
    simDirty = false;
    await loadSim();
    loadPorts().catch(() => {});
  });
};
