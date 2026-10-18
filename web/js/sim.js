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
  const media = sim.media.map((v) => [v, t("sim_m_" + v)]);
  // Spalte: Feld zum Bearbeiten (eigene Simulation) bzw. Text (Vorlage)
  const col = (head, f, show, cls = "") => ({ head, cls: ro ? cls : "", cell: (m) => (ro ? show(m) : ui.input({ label: head, ...f }, m[f.name] ?? "")) });
  const columns = [
    col(t("sim_col_pa"), { name: "primary", cls: "mono sim-pa", max: 3 }, (m) => m.primary, "num mono"),
    col(t("sim_col_id"), { name: "id", cls: "mono sim-id", max: 8 }, (m) => esc(m.id), "mono"),
    col(t("sim_col_man"), { name: "manufacturer", cls: "mono sim-man", max: 3 }, (m) => esc(m.manufacturer), "mono"),
    col(t("sim_col_medium"), { name: "medium", type: "select", options: media }, (m) => esc(t("sim_m_" + m.medium))),
    col(t("sim_col_baud"), { name: "baud", type: "select", options: [["", t("sim_baud_bus")], ...BAUDS.map((b) => [b, b])] }, (m) => m.baud ?? esc(t("sim_baud_bus")), "mono"),
    col(t("sim_col_fault"), { name: "fault", type: "select", options: sim.faults.map((v) => [v, t("sim_f_" + v)]) }, (m) =>
      m.fault === "none" ? esc(t("sim_f_none")) : ui.chip("warn", t("sim_f_" + m.fault))
    ),
    { cell: (m) => (ro ? "" : ui.iconBtn(ICON_DEL, t("delete"), { "sim-del": m.i }, "del")) },
  ];
  const faults = meters.filter((m) => m.fault !== "none").length;
  const warn = simWarnings(meters);
  $("sim-editor").innerHTML = `
    ${ro ? `<p class="hint">${esc(t("sim_builtin_note"))}</p>` : `<div class="row"><label class="grow"><span>${esc(t("sim_name"))}</span> <input id="sim-name" value="${esc(simDraft.name)}"></label></div>`}
    <p class="hint"><b>${esc(t("sim_count", meters.length, faults))}</b></p>
    ${warn.length ? `<ul class="sim-warn">${warn.map((w) => `<li>${esc(w)}</li>`).join("")}</ul>` : ""}
    ${ui.table({ columns, rows: meters.map((m, i) => ({ ...m, i })), cls: "sim-table", empty: t("sim_empty"), rowAttrs: (m) => `data-i="${m.i}"` })}
    ${
      ro
        ? ""
        : `<div class="row sim-actions">
        <button type="button" class="small ghost" data-sim-add>${esc(t("sim_add"))}</button>
        <span class="sim-many"><input id="sim-n" class="mono" value="10" inputmode="numeric" aria-label="${esc(t("sim_add_n"))}">
          ${ui.input({ name: "sim-n-medium", label: t("sim_col_medium"), type: "select", options: media }, "water")}
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
  if (!row || !el.name) return;
  const m = simDraft.meters[Number(row.dataset.i)];
  const f = el.name;
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
    const medium = $("sim-editor").querySelector("[name=sim-n-medium]").value;
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
