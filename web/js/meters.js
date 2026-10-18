"use strict";

// Zählerliste und Messwerte

// ------------------------------------------------------------ Zählerliste

let meters = [];
let editingId = null;
/// Schnellerfassung: Name, Ort, Notiz direkt in der Tabelle
let quickEdit = false;
/// Filter und Suche der Zählerliste
let meterFilter = "all";
let meterQuery = "";

const METER_FILTERS = {
  all: () => true,
  err: (m) => !!(m.last_error || m.plan_info?.disturbed),
  noname: (m) => !m.name,
};
const meterMatches = (m, q) =>
  !q || [m.name, m.location, m.note, m.meter_id, m.address].some((x) => String(x ?? "").toLowerCase().includes(q));


async function loadMeters() {
  meters = await api("meters");
  renderMeters();
  // Anzahl Zähler und letzte Auslesung je Linie in der Projekt-Karte mitführen
  if (tree) await loadTree().catch(() => {});
}

/// Plan-Spalte nur, wenn auf der Linie überhaupt etwas geplant ist
const showPlanColumn = () => meters.some((m) => m.plan || (m.plan_info && m.plan_info.effective !== "off"));

function meterPlanCell(m) {
  const pi = m.plan_info || {};
  const own = !!m.plan;
  if (pi.effective === "off") return `<td><span class="${own ? "" : "muted"}">${esc(t("plan_off"))}</span></td>`;
  const chips = [
    pi.disturbed ? ui.chip("warn", t("meter_disturbed"), t("meter_disturbed_title", pi.fails)) : "",
    pi.paused_until ? ui.chip("muted", t("meter_paused", slotText(pi.paused_until)), t("meter_paused_title")) : "",
  ].join("");
  return `<td class="wrap"><span class="${own ? "" : "muted"}">${esc(planText(pi.effective))}</span>${chips}${
    pi.next ? `<span class="sub">${esc(t("plan_next", slotText(pi.next)))}</span>` : ""
  }</td>`;
}

function renderMeters() {
  const table = $("meters-table");
  // beim Tippen nicht neu zeichnen (sonst geht die Eingabe verloren); danach nachholen
  const form = $("meter-form");
  const typing = (document.activeElement?.dataset?.q && table.contains(document.activeElement)) || form.contains(document.activeElement);
  if (typing) {
    document.activeElement.addEventListener("blur", () => setTimeout(renderMeters, 0), { once: true });
    return;
  }
  const withPlan = showPlanColumn();
  $("th-plan").hidden = !withPlan;
  table.hidden = meters.length === 0 && form.hidden;
  $("meters-clear").hidden = meters.length === 0;
  $("meters-empty").hidden = meters.length > 0 || !form.hidden;
  renderMetersHead();
  // Formular aus der Tabelle nehmen, bevor sie neu gezeichnet wird; danach in die Zeile setzen
  $("meters-empty").before(form);
  const q = meterQuery.trim().toLowerCase();
  const shown = meters.filter((m) => METER_FILTERS[meterFilter](m) && meterMatches(m, q));
  const cols = 4 + (withPlan ? 1 : 0);
  table.querySelector("tbody").innerHTML =
    (meters.length && !shown.length ? `<tr><td colspan="${cols}" class="empty-row">${esc(t("meters_none_match"))}</td></tr>` : "") +
    shown
    .map((m) => {
      const sub = (text) => (text ? `<span class="sub">${esc(text)}</span>` : "");
      const q = (f, label, more = {}) => ui.input({ name: f, label, placeholder: label, cls: "quick", data: { q: f, id: m.id }, ...more }, m[f]);
      const nameCell = quickEdit
        ? `<td><div class="mtr">${ui.mediumIcon(m.medium)}<span class="mtr-text">${q("name", t("meter_name"))}${q("note", t("meter_note"), {
            cls: "quick sub-input",
          })}</span></div></td>`
        : `<td class="wrap">${ui.meter(meterBlock(m), { alert: !!m.last_error })}</td>`;
      const locCell = quickEdit
        ? `<td><div class="quick-loc">${q("location", t("meter_location"), { list: "loc-list" })}<button class="icon-btn" data-q-same="${m.id}" title="${esc(t("quick_same"))}" aria-label="${esc(t("quick_same"))}">${ICON_DOWN}</button></div></td>`
        : `<td class="wrap">${esc(m.location)}${sub(m.note)}</td>`;
      const open = !form.hidden && editingId === m.id;
      return `<tr data-row="${m.id}"${open ? ' class="sel"' : ""}>
        ${nameCell}
        ${locCell}
        <td class="wrap">${m.last_value ? `<span class="val">${esc(m.last_value)}</span>` : ""}${sub(
          [m.last_value ? label(m.last_quantity) : "", m.last_read].filter(Boolean).join(" · ")
        )}${m.last_error ? `<span class="err">${ICON_ALERT}${esc(m.last_error)}</span>` : ""}</td>
        ${withPlan ? meterPlanCell(m) : ""}
        <td><div class="actions">
          <button class="small quiet needs-conn" data-meter-read="${m.id}">${t("read")}</button>
          <button class="small quiet" data-meter-transfer="${m.id}" title="${esc(t("dp_values_title"))}">${esc(t("dp_values_btn"))}</button>
          <button class="icon-btn" data-meter-hist="${m.id}" title="${esc(t("history"))}" aria-label="${esc(t("history"))}">${ICON_CHART}</button>
          <button class="icon-btn${open ? " on" : ""}" data-meter-edit="${m.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}" aria-expanded="${open}">${ICON_EDIT}</button>
          <button class="icon-btn del" data-meter-del="${m.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
        </div></td>
      </tr>`;
    })
    .join("");
  // Bearbeiten klappt in der Zeile auf; neuer Zähler: oben in der Tabelle
  if (!form.hidden) {
    const tr = document.createElement("tr");
    tr.className = "edit-row";
    const td = tr.insertCell();
    td.colSpan = cols;
    td.append(form);
    const row = editingId !== null && table.querySelector(`tr[data-row="${editingId}"]`);
    if (row) row.after(tr);
    else table.querySelector("tbody").prepend(tr);
  }
  // Ortsvorschläge aus allen schon verwendeten Orten
  renderLocList();
  table.classList.toggle("quick-mode", quickEdit);
  $("quick-toggle").textContent = t(quickEdit ? "quick_done" : "quick_edit");
  // eingeschaltet: „Fertig“ als sekundärer Knopf, sonst leise
  $("quick-toggle").classList.toggle("tonal", quickEdit);
  $("quick-toggle").classList.toggle("quiet", !quickEdit);
  updateButtons();
}

/// Felder des Zählerformulars (Baustein Formular), mit Plan je Zähler
function meterFields(m) {
  const line = tree ? findLine(activeLineId) : null;
  return [
    { name: "name", label: t("meter_name"), placeholder: t("meter_name_ph") },
    { name: "location", label: t("meter_location"), placeholder: t("meter_location_ph"), list: "loc-list" },
    { name: "address", label: t("address"), cls: "mono", placeholder: t("address_ph"), required: true },
    { name: "baud", label: t("baud"), type: "select", num: true, options: [["", t("baud_bus")], ...BAUDS.map((b) => [b, b])] },
    { name: "note", label: t("meter_note"), wide: true },
    ...planFields(m?.plan ?? null, t("plan_inherit_line", planText(line?.plan_info?.effective || "off")), t("plan_hint_meter")),
  ];
}

/// Ergebnis von „Alle auslesen“: wie viele Zähler antworten, wie viele nicht (zu finden über den
/// Filter „Gestört“). Ersetzt eine eigene Schnellprüfung: Auslesen prüft jeden bekannten Zähler.
function readAllSummary() {
  const bad = meters.filter((m) => m.last_error).length;
  return bad ? t("read_all_sum_bad", meters.length - bad, meters.length, bad) : t("read_all_sum_ok", meters.length);
}

/// Kopf und Leiste der Tabelle: Untertitel, Filter mit Anzahl, „Alle N auslesen“
function renderMetersHead() {
  const last = meters.map((m) => m.last_read).filter(Boolean).sort().pop();
  $("meters-sub").textContent = tree ? t("meters_sub", meters.length, tree.active.line.name, last ? slotText(last) : "") : "";
  $("meters-filter").innerHTML = Object.keys(METER_FILTERS)
    .map((k) => {
      const n = meters.filter(METER_FILTERS[k]).length;
      return `<button type="button" data-filter="${k}" class="${k === meterFilter ? "active" : ""}">${esc(t("mf_" + k))} <span class="seg-n${
        k === "err" && n ? " err" : ""
      }">${n}</span></button>`;
    })
    .join("");
  $("meters-read-all").textContent = t("meters_read_all_n", meters.length);
}

$("meters-filter").onclick = (ev) => {
  const b = ev.target.closest("[data-filter]");
  if (!b) return;
  meterFilter = b.dataset.filter;
  renderMeters();
};
$("meters-search").oninput = () => {
  meterQuery = $("meters-search").value;
  renderMeters();
};

/// Bearbeiten (oder neuer Zähler) in der Zeile öffnen; nochmals = schliessen
function openMeterForm(m) {
  const form = $("meter-form");
  if (!form.hidden && m && editingId === m.id) return closeMeterForm();
  editingId = m ? m.id : null;
  $("mf-fields").innerHTML = ui.form(meterFields(m), { ...m, baud: m?.baud ?? "" });
  wirePlanField($("mf-fields"));
  $("mf-exchange").hidden = !m;
  form.hidden = false;
  renderMeters();
  $("mf-fields").querySelector("input").focus();
}

function closeMeterForm() {
  $("meter-form").hidden = true;
  editingId = null;
  renderMeters();
}

$("meter-add").onclick = () => openMeterForm(null);

/// Zähler getauscht: neuen Zähler auslesen, Zählerstände der Datenpunkte laufen weiter
$("mf-exchange").onclick = () => {
  const m = meters.find((x) => x.id === editingId);
  if (!m) return;
  const addr = prompt(t("exchange_prompt"));
  if (!addr || !addr.trim()) return;
  if (!confirm(t("exchange_confirm", m.meter_id || m.address, addr.trim()))) return;
  guarded($("mf-exchange"), async () => {
    const r = await api(`meters/${m.id}/exchange`, { method: "POST", body: { address: addr.trim() } });
    closeMeterForm();
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  });
};
$("mf-cancel").onclick = () => closeMeterForm();

$("meter-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("meter-form").querySelector("[type=submit]"), async () => {
    const old = meters.find((m) => m.id === editingId) || {};
    // alles zuerst prüfen (auch die Uhrzeit des Plans), dann speichern
    const body = { ...old, ...ui.formValues($("mf-fields"), meterFields(old)) };
    const plan = planFrom(body);
    const saved = editingId === null ? await api("meters", { method: "POST", body }) : await api("meters/" + editingId, { method: "PUT", body });
    if (plan !== (old.plan ?? null)) await savePlan("meters/" + saved.id, plan);
    closeMeterForm();
    toast(t("meter_saved"));
    await loadMeters();
  });
};

$("meters-table").addEventListener("click", (ev) => {
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  if (d.qSame) return quickSame(Number(d.qSame));
  const m = meters.find((x) => String(x.id) === (d.meterRead ?? d.meterEdit ?? d.meterDel ?? d.meterHist ?? d.meterTransfer));
  if (!m) return;
  if (d.meterEdit) return openMeterForm(m);
  if (d.meterTransfer) return guarded(btn, () => openTransfer(m.id));
  if (d.meterHist) return guarded(btn, () => openHistory(m));
  if (d.meterDel) {
    if (!confirm(t("confirm_delete_meter", m.name || m.address))) return;
    guarded(btn, async () => {
      await api("meters/" + m.id, { method: "DELETE" });
      await loadMeters();
    });
    return;
  }
  // Auslesen mit der Baudrate des Zählers, Ergebnis im Bereich "Zähler auslesen"
  $("read-addr").value = m.address;
  $("set-old").value = m.address;
  renderTarget();
  guarded(btn, async () => {
    $("reading").innerHTML = `<p class="loading"><span class="spin"></span>${t("reading_busy")}</p>`;
    try {
      renderReading(await api(`meters/${m.id}/read`, { method: "POST" }));
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    } finally {
      await loadMeters().catch(() => {});
      refreshStatus().catch(() => {});
    }
  });
});

$("meters-clear").onclick = () => {
  if (!confirm(t("confirm_meters_clear", meters.length))) return;
  guarded($("meters-clear"), async () => {
    const r = await api("meters", { method: "DELETE" });
    closeMeterForm();
    toast(t("meters_cleared", r.removed));
    await loadMeters();
  });
};

$("meters-read-all").onclick = () =>
  guarded($("meters-read-all"), async () => {
    await api("meters/read-all", { method: "POST" });
    jobRunning = true;
    updateButtons();
    pollJob();
  });

$("meters-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  // CSV/Text: Liste mit Spaltenzuordnung; JSON: Zählerliste aus mbusio (wie bisher)
  if (!/\.json$/i.test(file.name)) {
    file.text().then((text) => {
      $("list-paste").value = text;
      $("list-result").innerHTML = "";
      listLoad(text);
    });
    return;
  }
  guarded($("meter-add"), async () => {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("import_bad"));
    }
    const list = Array.isArray(data) ? data : data?.meters;
    if (!Array.isArray(list)) throw new Error(t("import_bad"));
    const r = await api("meters/import", { method: "POST", body: { meters: list } });
    toast(t("imported", r.added));
    await loadMeters();
  });
};

// ------------------------------------------------------------ Messwerte

$("clear-readings").onclick = () =>
  guarded($("clear-readings"), async () => {
    if (!confirm(t("confirm_clear"))) return;
    await api("readings", { method: "DELETE" });
    await refreshStatus();
  });
