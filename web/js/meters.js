"use strict";

// Zählerliste und Messwerte

// ------------------------------------------------------------ Zählerliste

let meters = [];
let editingId = null;
/// Schnellerfassung: Name, Ort, Notiz direkt in der Tabelle
let quickEdit = false;

const svg = (d) =>
  `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON_EDIT = svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>');
const ICON_DEL = svg('<path d="M6 6l12 12M18 6L6 18"/>');
const ICON_UP = svg('<path d="M6 15l6-6 6 6"/>');
const ICON_DOWN = svg('<path d="M6 9l6 6 6-6"/>');
const ICON_CHART = svg('<path d="M4 19h16M5 15l4-5 4 3 6-7"/>');
const ICON_ALERT = svg('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16.5v.01"/>');

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
  const typing = document.activeElement?.dataset?.q && table.contains(document.activeElement);
  if (typing) {
    document.activeElement.addEventListener("blur", () => setTimeout(renderMeters, 0), { once: true });
    return;
  }
  const withPlan = showPlanColumn();
  $("th-plan").hidden = !withPlan;
  table.hidden = meters.length === 0;
  $("meters-clear").hidden = meters.length === 0;
  $("meters-empty").hidden = meters.length > 0;
  table.querySelector("tbody").innerHTML = meters
    .map((m) => {
      // ID nur zusätzlich zeigen, wenn die Adresse nicht schon die ID ist
      // die jeweils andere Adresse klein dazu: bei SA die PA, bei PA die ID
      const viaPa = /^\d+$/.test(m.address);
      const info = [
        !viaPa && m.primary_address != null ? `PA ${m.primary_address}` : null,
        m.meter_id !== m.address ? m.meter_id : null,
        m.manufacturer,
      ]
        .filter(Boolean)
        .join(" · ");
      const sub = (text) => (text ? `<span class="sub">${esc(text)}</span>` : "");
      const q = (f, v, extra = "", cls = "quick") => `<input class="${cls}" data-q="${f}" data-id="${m.id}" value="${esc(v)}" ${extra}>`;
      const nameCell = quickEdit
        ? `<td>${q("name", m.name, `placeholder="${esc(t("meter_name"))}" aria-label="${esc(t("meter_name"))}"`)}${q("note", m.note, `placeholder="${esc(t("meter_note"))}" aria-label="${esc(t("meter_note"))}"`, "quick sub-input")}</td>`
        : `<td class="wrap"><span class="name">${esc(m.name || "–")}</span>${sub(m.note)}</td>`;
      const locCell = quickEdit
        ? `<td><div class="quick-loc">${q("location", m.location, `list="loc-list" placeholder="${esc(t("meter_location"))}" aria-label="${esc(t("meter_location"))}"`)}<button class="icon-btn" data-q-same="${m.id}" title="${esc(t("quick_same"))}" aria-label="${esc(t("quick_same"))}">${ICON_DOWN}</button></div></td>`
        : `<td class="wrap">${esc(m.location)}</td>`;
      return `<tr>
        ${nameCell}
        ${locCell}
        <td class="mono">${esc(m.address)}${sub(info)}</td>
        <td>${esc(label(m.medium))}</td>
        <td class="num mono">${m.baud ?? ""}</td>
        ${withPlan ? meterPlanCell(m) : ""}
        <td class="wrap">${m.last_value ? `<span class="val">${esc(m.last_value)}</span>` : ""}${sub(
          [m.last_value ? label(m.last_quantity) : "", m.last_read].filter(Boolean).join(" · ")
        )}${m.last_error ? `<span class="err">${ICON_ALERT}${esc(m.last_error)}</span>` : ""}</td>
        <td><div class="actions">
          <button class="small quiet needs-conn" data-meter-read="${m.id}">${t("read")}</button>
          <button class="small quiet" data-meter-transfer="${m.id}" title="${esc(t("dp_values_title"))}">${esc(t("dp_values_btn"))}</button>
          <button class="icon-btn" data-meter-hist="${m.id}" title="${esc(t("history"))}" aria-label="${esc(t("history"))}">${ICON_CHART}</button>
          <button class="icon-btn" data-meter-edit="${m.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
          <button class="icon-btn del" data-meter-del="${m.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
        </div></td>
      </tr>`;
    })
    .join("");
  // Ortsvorschläge aus allen schon verwendeten Orten
  renderLocList();
  table.classList.toggle("quick-mode", quickEdit);
  $("quick-toggle").textContent = t(quickEdit ? "quick_done" : "quick_edit");
  $("quick-toggle").classList.toggle("on", quickEdit);
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
    { type: "html", html: planField(m?.plan ?? null, t("plan_inherit_line", planText(line?.plan_info?.effective || "off")), t("plan_hint_meter")) },
  ];
}

function openMeterForm(m) {
  editingId = m ? m.id : null;
  $("mf-fields").innerHTML = ui.form(meterFields(m), { ...m, baud: m?.baud ?? "" });
  wirePlanField($("mf-fields"));
  $("mf-exchange").hidden = !m;
  $("meter-form").hidden = false;
  $("mf-fields").querySelector("input").focus();
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
    $("meter-form").hidden = true;
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  });
};
$("mf-cancel").onclick = () => ($("meter-form").hidden = true);

$("meter-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("meter-form").querySelector("[type=submit]"), async () => {
    const old = meters.find((m) => m.id === editingId) || {};
    // alles zuerst prüfen (auch die Uhrzeit des Plans), dann speichern
    const body = { ...old, ...ui.formValues($("mf-fields"), meterFields(old)) };
    const plan = planFrom(planInputs($("mf-fields")));
    const saved = editingId === null ? await api("meters", { method: "POST", body }) : await api("meters/" + editingId, { method: "PUT", body });
    if (plan !== (old.plan ?? null)) await savePlan("meters/" + saved.id, plan);
    $("meter-form").hidden = true;
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
    $("meter-form").hidden = true;
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
