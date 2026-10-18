"use strict";

// Verbindung, Scan, Statusleiste, Auffälligkeiten, Alle beheben, Baudraten

// ------------------------------------------------------------ Verbindung

async function loadPorts() {
  const ports = await api("ports");
  const sel = $("port");
  const prev = sel.value;
  sel.innerHTML = ports.map((p) => `<option>${esc(p)}</option>`).join("");
  if (ports.includes(prev)) sel.value = prev;
}

async function refreshStatus() {
  const s = await api("status");
  connected = s.connected;
  const prevBaud = connBaud;
  connBaud = s.connection?.baud ?? null;
  conn = s.connection;
  activeLineId = s.active?.line.id ?? null;
  // Termin fertig geworden: Zählerliste und Projekt-Karte zeigen das neue Ergebnis
  const runsBefore = planRuns.length;
  planRuns = s.plans || [];
  timeState = s.time;
  planActive = !!s.plan_active;
  disk = s.disk;
  mqttStatus = s.mqtt;
  if (transfer.data && !$("sec-outputs").hidden) renderOutputs();
  if (runsBefore && !planRuns.length) loadMeters().catch(() => {});
  // verbunden: Verbindung als eine Zeile, Formular erst auf "Ändern"
  $("sec-conn").classList.toggle("compact", connected && !connEditing);
  $("conn-summary").hidden = !connected || connEditing;
  if (connected) $("conn-summary-text").textContent = t("conn_summary", s.connection.port, s.connection.baud);
  // "Baudrate abweichend" hängt von der Bus-Baudrate ab: Trefferliste neu zeichnen, wenn sie sich ändert
  if (connBaud !== prevBaud && lastJob) renderJob(lastJob);
  renderBusbar();
  // Auswahl folgt der eingestellten Baudrate, bis man selbst etwas anklickt
  if (connBaud && connBaud !== prevBaud && !baudsTouched) setPickedBauds([connBaud]);
  updateEstimate();
  jobRunning = s.job_running;
  const badge = $("conn-badge");
  const c = s.connection;
  badge.className = connected ? "chip on" : "chip off";
  badge.querySelector(".chip-text").textContent = connected ? `${c.port} · ${c.baud} Bd` : t("disconnected");
  $("readings-count").innerHTML = `<b>${s.readings}</b>${esc(t("readings_count", s.readings))}`;
  $("meters-file").textContent = s.meters_file ? t("meters_file", s.meters_file) : "";
  updateButtons();
  if (jobRunning) pollJob();
}

$("connect").onclick = () =>
  guarded($("connect"), async () => {
    const port = $("port-manual").value.trim() || $("port").value;
    await api("connect", {
      method: "POST",
      body: { port, baud: Number($("baud").value), timeout_ms: Number($("timeout").value) || 0 },
    });
    toast(t("connected_to", port));
    connEditing = false;
    await refreshStatus();
  });

$("disconnect").onclick = () =>
  guarded($("disconnect"), async () => {
    const r = await api("disconnect", { method: "POST" });
    if (r.plan_paused) toast(t("plan_paused_disconnect"));
    await refreshStatus();
    loadTree().catch(() => {});
  });

$("busbar-time").onclick = () => {
  const now = new Date();
  if (!confirm(t("confirm_time", now.toLocaleString(lang === "de" ? "de-CH" : "en-GB")))) return;
  guarded($("busbar-time"), async () => {
    const r = await api("time", { method: "POST", body: { epoch_ms: Date.now() } });
    toast(t("time_taken", Math.round(r.offset_s)));
    await refreshStatus();
  });
};

$("refresh-ports").onclick = () => loadPorts().catch((e) => toast(e.message, true));

// ------------------------------------------------------------ Scan

let polling = false;
let renderedJob = null;
let renderedHits = 0;

// ------------------------------------------------------------ Leiste unten

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

let cancelRequested = false;

function cancelJob() {
  const kind = currentJobKind;
  if (!confirm(t(kind === "read_all" ? "confirm_cancel_read" : "confirm_cancel_scan"))) return;
  cancelRequested = true;
  api("job/cancel", { method: "POST" }).catch((e) => toast(e.message, true));
}

$("busbar-cancel").onclick = cancelJob;

function jobLink(job) {
  return job.kind === "read_all" ? "#sec-meters" : "#sec-scan";
}

async function pollJob() {
  if (polling) return;
  polling = true;
  try {
    for (;;) {
      const job = await api("job");
      renderJob(job);
      // Zählerliste während "Alle auslesen" mitlaufend aktualisieren
      if (job.kind === "read_all") await loadMeters().catch(() => {});
      if (!job.running) {
        jobRunning = false;
        updateButtons();
        const cancelled = cancelRequested;
        cancelRequested = false;
        const done =
          job.kind === "read_all"
            ? t(cancelled ? "read_all_cancelled" : "read_all_done")
            : t(cancelled ? "job_cancelled" : "job_done", t("job_" + job.kind), job.hits.length);
        busbar({
          text: job.error || done,
          frac: 1,
          link: jobLink(job),
          done: true,
          error: !!job.error,
        });
        // Erfolg meldet die Leiste unten, nur Fehler zusätzlich als Toast
        if (job.error) toast(job.error, true);
        break;
      }
      await new Promise((r) => setTimeout(r, 600));
    }
  } catch (e) {
    toast(e.message, true);
  } finally {
    polling = false;
    refreshStatus().catch(() => {});
  }
}

/// Ursache eines Treffers in einfachen Worten: [was ist los, was tun]
function explainReason(reason) {
  const [key, arg] = (reason || "").split(":");
  switch (key) {
    case "wrong_address":
      return t("why_wrong_address", arg);
    case "app_error":
      return t("why_app_error", arg, t("app_errors")[arg] || "?");
    case "collision":
    case "checksum":
    case "stop":
    case "garbled":
    case "truncated":
    case "no_reply":
    case "unexpected":
    case "decode":
    case "same_id":
      return t("why_" + key);
    default:
      return t("why_unknown");
  }
}

/// geöffnete "Warum?"-Zeilen, bleiben beim Neuzeichnen offen
const openWhy = new Set();
const hitKey = (h) => `${h.baud}:${h.primary_address}:${h.secondary_address}`;

function whyRow(h, cols) {
  const [what, tip] = explainReason(h.reason);
  return `<tr class="why-row"><td colspan="${cols}"><div class="why">
    <p><b>${esc(what)}</b></p>
    <p><span class="why-label">${esc(t("why_tip"))}</span> ${esc(tip)}</p>
    ${h.raw ? `<p class="why-raw"><span class="why-label">${esc(t("why_raw"))}:</span> <code>${esc(h.raw)}</code><a href="#analysis" class="analyze-link" data-analyze="${esc(h.raw)}">${esc(t("why_analyze"))} →</a></p>` : ""}
  </div></td></tr>`;
}

/// Auffälligkeiten: doppelte Primäradressen (je Baudrate), doppelte IDs, fremde Baudraten
function analyzeHits(hits) {
  const group = (key) => {
    const m = new Map();
    hits.forEach((h, i) => {
      const k = key(h);
      if (k !== null) m.set(k, [...(m.get(k) || []), i]);
    });
    return new Map([...m].filter(([, list]) => list.length > 1));
  };
  // Kollisionen aus dem Primärscan sind schon eine Zeile für sich, die zählen nicht doppelt
  const dupPa = group((h) => (h.primary_address == null || h.status === "collision" ? null : `${h.baud}:${h.primary_address}`));
  const dupId = group((h) => (h.id && /^[0-9]{8}$/.test(h.id) ? h.id : null));
  const rowDupPa = new Set([...dupPa.values()].flat());
  // gleiche ID ist nur ein Problem, wenn auch der Hersteller gleich ist (IDs sind je Hersteller eindeutig)
  const dupIdSame = new Map(
    [...dupId].filter(([, list]) => new Set(list.map((i) => hits[i].manufacturer)).size < list.length)
  );
  const rowDupId = new Set([...dupId.values()].flat());
  const rowDupIdSame = new Set([...dupIdSame.values()].flat());
  const otherBaud = hits.filter((h) => h.baud && h.baud !== connBaud).length;
  return { dupPa, dupId, dupIdSame, rowDupPa, rowDupId, rowDupIdSame, otherBaud };
}

function renderHitsWarn(hits, a) {
  const items = [];
  for (const [k, list] of a.dupPa) {
    const [baud, pa] = k.split(":");
    items.push(t("warn_dup_pa", pa, baud, list.map((i) => hits[i].id || "?")));
  }
  for (const [id, list] of a.dupIdSame) items.push(t("warn_dup_id", id, list.length));
  const collisions = hits.filter((h) => h.status === "collision");
  for (const h of collisions) {
    if (h.primary_address != null) items.push(t("warn_collision_pa", h.primary_address, h.baud));
    else if (h.id) items.push(t("warn_collision_id", h.id));
  }
  // Zähler ohne Daten einzeln, mit Ursache (auch für den Bericht verständlich)
  for (const h of hits.filter((x) => x.status === "no_data")) {
    const name = h.id || `PA ${h.primary_address}`;
    items.push(`${name}: ${explainReason(h.reason)[0]}`);
  }
  if (a.otherBaud) items.push(t("warn_other_baud", a.otherBaud, connBaud));
  const el = $("hits-warn");
  el.hidden = items.length === 0;
  issueCount = items.length;
  // erst wenn der Scan durch ist: bei der Linie merken (Projekt-Karte, Bericht)
  if (!lastJob?.running) saveIssues(items);
  renderBusbar();
  el.innerHTML = items.length
    ? `<b>⚠ ${esc(t("warn_title"))}</b><ul>${items.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>` +
      (a.dupPa.size ? `<p class="hint">${esc(t("warn_fix_hint"))}</p>` : "") +
      (planSteps(hits, a).steps.length
        ? `<button class="small cta needs-conn" data-fix-all>${esc(t("fix_all"))}</button> `
        : "") +
      // Kollision im Primärscan: der Sekundärscan findet die Zähler einzeln
      (collisions.some((h) => h.primary_address != null)
        ? `<button class="small cta needs-conn" data-start-secondary>${esc(t("start_secondary"))}</button>`
        : "")
    : "";
}

function renderJob(job) {
  currentJobKind = job.kind;
  // lastJob ist die Trefferliste; "Alle auslesen" darf sie nicht ersetzen
  if (job.kind !== "read_all") lastJob = job;
  if (job.running) {
    const frac = job.total ? Math.min(job.done / job.total, 1) : 0;
    const found = job.kind === "read_all" ? "" : ` · ${t("found", job.hits.length)}`;
    busbar({ text: `${t("job_" + job.kind)} · ${job.message}${found}`, frac, link: jobLink(job), cancel: true });
  }
  if (job.kind === "read_all") {
    const el = $("meters-job");
    el.hidden = false;
    el.textContent = job.running
      ? `${t("job_read_all")}: ${Math.min(job.done + 1, job.total)}/${job.total} · ${job.message}`
      : `${t("job_read_all")}: ${job.error || job.message}`;
    updateButtons();
    return;
  }
  $("job").hidden = false;
  const p = $("job-progress");
  const frac = job.total ? Math.min(job.done / job.total, 1) : 0;
  p.querySelector(".bar-fill").style.transform = `scaleX(${job.running ? frac : 1})`;
  p.classList.toggle("running", job.running);
  p.setAttribute("aria-valuenow", Math.round(frac * 100));
  $("job-msg").innerHTML = `<b>${esc(t("job_" + job.kind))}</b>${esc(job.message)} · ${esc(t("found", job.hits.length))}`;
  // nur neu hinzugekommene Treffer einblenden, nicht bei jeder Abfrage alle
  const fresh = job.kind === renderedJob && job.hits.length >= renderedHits ? renderedHits : 0;
  renderedJob = job.kind;
  renderedHits = job.hits.length;
  const table = $("hits");
  table.hidden = job.hits.length === 0;
  const issues = analyzeHits(job.hits);
  renderHitsWarn(job.hits, issues);
  document.querySelector(".hits-opt").hidden = table.hidden;
  // Sekundäradresse lesbar zerlegt (ID, Version) oder roh als 16 Hex-Zeichen
  const raw = $("sa-raw").checked;
  const cols = raw
    ? ["col_primary", "col_secondary", "col_manufacturer", "col_medium", "col_status", ""]
    : ["col_primary", "col_id", "col_manufacturer", "col_version", "col_medium", "col_status", ""];
  cols.splice(1, 0, "col_baud");
  const numCols = ["col_primary", "col_baud", "col_version"];
  table.querySelector("thead").innerHTML = `<tr>${cols
    .map((c) => {
      if (!c) return "<th></th>";
      const on = hitSort?.col === c;
      const cls = [numCols.includes(c) ? "num" : "", "sortable", on ? "sorted" : ""].filter(Boolean).join(" ");
      const aria = on ? ` aria-sort="${hitSort.dir > 0 ? "ascending" : "descending"}"` : "";
      return `<th class="${cls}" data-sort="${c}" title="${esc(t("sort_title"))}"${aria}>${t(c)}<span class="sort-ind">${
        on ? (hitSort.dir > 0 ? "▲" : "▼") : ""
      }</span></th>`;
    })
    .join("")}</tr>`;
  table.querySelector("tbody").innerHTML = sortedHits(job.hits)
    .map(([h, i]) => {
      const target = h.status === "ok" ? hitAddress(h) : "";
      const sa = raw
        ? `<td class="mono">${esc(h.secondary_address ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>`
        : `<td class="mono">${esc(h.id ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>
        <td class="num mono">${h.version ?? ""}</td>`;
      const other = h.baud && h.baud !== connBaud;
      const anim = i >= fresh ? ` class="new" style="animation-delay:${Math.min(i - fresh, 8) * 50}ms"` : "";
      const dupPa = issues.rowDupPa.has(i);
      const dupId = issues.rowDupId.has(i);
      const paCount = dupPa ? issues.dupPa.get(`${h.baud}:${h.primary_address}`).length : 0;
      const warnChips =
        (dupPa ? " " + ui.chip("warn", t("dup_pa"), t("dup_pa_title", h.primary_address, paCount)) : "") +
        (dupId
          ? issues.rowDupIdSame.has(i)
            ? " " + ui.chip("warn", t("dup_id"), t("dup_id_title"))
            : " " + ui.chip("muted", t("dup_id_info"), t("dup_id_info_title"))
          : "");
      // PA ändern: lieber über die ID, bei doppelter ID über die volle Sekundäradresse
      const fixTarget = dupId ? h.secondary_address : h.id || h.secondary_address;
      const fix =
        dupPa && fixTarget
          ? `<button class="small ghost needs-conn" data-fix="${esc(fixTarget)}" data-from="${other ? h.baud : ""}">${t("fix_pa")}</button>`
          : "";
      // Empfehlung: Zähler auf die Baudrate des Busses umstellen
      const fixBaud =
        other && h.status === "ok" && fixTarget
          ? `<button class="small ghost needs-conn" data-fixbaud="${esc(fixTarget)}" data-from="${h.baud}">${t("fix_baud")}</button>`
          : "";
      const otherChip = other && h.status === "ok"
        ? " " + ui.chip("warn", t("other_baud"), t("other_baud_title", h.baud, connBaud))
        : "";
      return `<tr${anim}>
        <td class="num mono${dupPa ? " st-other" : ""}"${dupPa ? ` title="${esc(t("dup_pa_title", h.primary_address, paCount))}"` : ""}>${h.primary_address ?? "–"}${dupPa ? " ⚠" : ""}</td>
        <td class="num mono${other ? " st-other" : ""}">${h.baud ?? ""}</td>
        ${sa}
        <td>${esc(label(h.medium))}</td>
        <td>${ui.chip({ ok: "ok", collision: "warn" }[h.status] || "muted", t("st_" + h.status))}${warnChips}${otherChip}</td>
        <td><div class="actions">${
          h.status !== "ok" ? `<button class="small ghost" data-why="${esc(hitKey(h))}">${t("why")}</button>` : ""
        }${fix}${fixBaud}${
          target !== ""
            ? `<button class="small quiet needs-conn" data-read="${esc(target)}" data-baud="${h.baud ?? ""}">${t("read")}</button>
               <button class="small ghost" data-remember="${i}">${t("remember")}</button>`
            : ""
        }</div></td>
      </tr>${openWhy.has(hitKey(h)) ? whyRow(h, cols.length) : ""}`;
    })
    .join("");
  updateButtons();
}

/// Eintrag für die Zählerliste aus einem Treffer
/// Adresse zum Ansprechen eines Treffers: volle Sekundäradresse, ausser sie ist mehrdeutig
/// (gleiche ID und gleicher Hersteller), dann die Primäradresse
function hitAddress(h) {
  const same = (lastJob?.hits || []).filter((x) => x.secondary_address && x.secondary_address === h.secondary_address);
  if (h.secondary_address && (same.length < 2 || h.primary_address == null)) return h.secondary_address;
  return String(h.primary_address);
}

/// Wie die aktive Linie Zähler merkt: "secondary" (SA) oder "primary" (PA)
const addressing = () => tree?.active.line.addressing || "secondary";

/// Adresse zum Merken nach der Einstellung der Linie; null + Grund, wenn sie nicht eindeutig ist
function rememberAddress(h) {
  const hits = lastJob?.hits || [];
  if (addressing() === "primary") {
    const pa = h.primary_address;
    if (pa == null) return { reason: t("skip_pa_unknown") };
    if (pa === 0) return { reason: t("skip_pa_zero") };
    if (hits.filter((x) => x.status === "ok" && x.baud === h.baud && x.primary_address === pa).length > 1)
      return { reason: t("skip_pa_dup", pa) };
    return { address: String(pa) };
  }
  // Platzhalter (F) z.B. bei alten Zählern ohne Herstellerkennung: geht, solange eindeutig
  const sa = h.secondary_address;
  if (!sa || /^F{8}/.test(sa)) return { reason: t("skip_sa_unknown") };
  if (hits.filter((x) => x.secondary_address === sa).length > 1) return { reason: t("skip_sa_dup") };
  return { address: sa };
}

function meterFromHit(h) {
  return {
    address: rememberAddress(h).address,
    secondary_address: h.secondary_address || null,
    primary_address: h.primary_address,
    // eigene Baudrate nur merken, wenn sie vom Bus abweicht
    baud: h.baud && h.baud !== connBaud ? h.baud : null,
    meter_id: h.id,
    manufacturer: h.manufacturer || null,
    medium: h.medium,
  };
}

$("hits-warn").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-start-secondary]")) $("scan-secondary").click();
  if (ev.target.closest("[data-fix-all]")) guarded(ev.target.closest("button"), openPlan);
});

// ------------------------------------------------------------ Alle beheben

let plan = null;
let planRunning = false;

/// Was sich automatisch beheben lässt (Schritte) und was von Hand (Texte)
function planSteps(hits, a) {
  const steps = [];
  const manual = [];
  const name = (h) => h.id || h.secondary_address || `PA ${h.primary_address}`;
  // eindeutig ansprechbar nur über die volle Sekundäradresse, sofern sie nicht mehrfach vorkommt
  const target = (h) =>
    h.secondary_address && hits.filter((x) => x.secondary_address === h.secondary_address).length === 1
      ? h.secondary_address
      : null;
  for (const h of hits) {
    if (h.status === "ok" && h.baud && h.baud !== connBaud && target(h)) {
      steps.push({ kind: "baud", target: target(h), name: name(h), from: h.baud, to: connBaud });
    }
  }
  for (const [k, list] of a.dupPa) {
    const pa = Number(k.split(":")[1]);
    const members = list.map((i) => hits[i]).sort((x, y) => String(x.id).localeCompare(String(y.id)));
    // PA 0 ist der Werkszustand: alle umziehen; sonst darf der erste bleiben
    for (const h of pa === 0 ? members : members.slice(1)) {
      if (!target(h)) continue;
      steps.push({ kind: "pa", target: target(h), name: name(h), from: pa, to: null, fromBaud: h.baud !== connBaud ? h.baud : null });
    }
  }
  for (const h of hits) {
    if (h.status === "collision" && h.primary_address != null) manual.push(t("plan_manual_collision", h.primary_address, h.baud));
    // Sekundärscan: gleiche ID, über die ID allein nicht trennbar
    if (h.status === "collision" && h.primary_address == null && h.id) manual.push(t("plan_manual_id_collision", h.id));
    if (h.status === "no_data") manual.push(t("plan_manual_nodata", name(h)));
  }
  for (const id of a.dupIdSame.keys()) manual.push(t("plan_manual_sameid", id));
  return { steps, manual };
}

async function openPlan() {
  const hits = lastJob.hits;
  plan = planSteps(hits, analyzeHits(hits));
  // freie Adressen vorher suchen, damit der Plan konkrete Zahlen zeigt
  const skip = new Set(hits.map((h) => h.primary_address).filter((x) => x != null && x > 0));
  const paSteps = plan.steps.filter((s) => s.kind === "pa");
  if (paSteps.length) busbar({ text: t("plan_busy_free"), link: "#fix-plan" });
  try {
    for (const s of paSteps) {
      const r = await request("free-address", { method: "POST", body: { from: 1, skip: [...skip] } });
      s.to = r.address;
      skip.add(r.address);
    }
  } finally {
    if (paSteps.length) busbar(null);
  }
  renderPlan();
  $("fix-plan").scrollIntoView({ behavior: "smooth", block: "center" });
}

function renderPlan() {
  const el = $("fix-plan");
  if (!plan) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  const text = (s) => (s.kind === "baud" ? t("plan_baud", s.name, s.from, s.to) : t("plan_pa", s.name, s.from, s.to));
  const checked = plan.steps.filter((s) => !s.skip).length;
  // fertig erst, wenn die Ausführung durch ist (nicht schon beim ersten laufenden Schritt)
  const finished = !planRunning && plan.steps.some((s) => s.state);
  el.innerHTML = `
    <h3>${esc(plan.steps.length ? t("plan_title", plan.steps.length) : t("plan_none"))}</h3>
    ${plan.steps.length && !finished ? `<p class="hint">${esc(t("plan_hint"))}</p>` : ""}
    <ul>${plan.steps
      .map(
        (s, k) => `<li><label><input type="checkbox" data-step="${k}" ${s.skip ? "" : "checked"} ${planRunning || finished ? "disabled" : ""}> ${esc(text(s))}</label>
        <span class="plan-st ${s.state === "ok" ? "ok" : s.state === "err" ? "err" : ""}">${
          s.state === "ok" ? "✓" : s.state === "err" ? "✗ " + esc(s.error) : s.state === "run" ? "…" : ""
        }</span></li>`
      )
      .join("")}</ul>
    ${plan.manual.length ? `<p class="hint"><b>${esc(t("plan_manual"))}</b></p><ul class="manual">${plan.manual.map((m) => `<li>${esc(m)}</li>`).join("")}</ul>` : ""}
    <div class="row">
      ${!finished && !planRunning && plan.steps.length ? `<button class="cta needs-conn" data-plan-run ${checked ? "" : "disabled"}>${esc(t("plan_run", checked))}</button>` : ""}
      ${finished ? `<span class="hint result">${esc(t("plan_done", plan.steps.filter((s) => s.state === "ok").length, plan.steps.filter((s) => s.state).length))}</span>` : ""}
      <button class="quiet" data-plan-close ${planRunning ? "disabled" : ""}>${esc(t(finished ? "plan_close" : "cancel"))}</button>
    </div>`;
  updateButtons();
}

$("fix-plan").addEventListener("change", (ev) => {
  const k = ev.target.dataset.step;
  if (k === undefined) return;
  plan.steps[Number(k)].skip = !ev.target.checked;
  renderPlan();
});

$("fix-plan").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-plan-close]")) {
    plan = null;
    renderPlan();
  } else if (ev.target.closest("[data-plan-run]")) {
    runPlan();
  }
});

async function runPlan() {
  const todo = plan.steps.filter((s) => !s.skip);
  planRunning = true;
  renderPlan();
  // Baudraten zuerst: danach läuft der Zähler mit der Bus-Baudrate, auch für die Adressänderung
  todo.sort((x, y) => (x.kind === "baud" ? 0 : 1) - (y.kind === "baud" ? 0 : 1));
  const switched = new Set();
  for (const [k, s] of todo.entries()) {
    busbar({ text: t("plan_running", k + 1, todo.length), frac: k / todo.length, link: "#fix-plan" });
    s.state = "run";
    renderPlan();
    try {
      if (s.kind === "baud") {
        await doSwitchBaud(s.target, s.to, s.from);
        switched.add(s.target);
      } else {
        await doSetAddress(s.target, s.to, switched.has(s.target) ? null : s.fromBaud);
      }
      s.state = "ok";
    } catch (e) {
      s.state = "err";
      s.error = e.message;
    }
  }
  planRunning = false;
  const ok = todo.filter((s) => s.state === "ok").length;
  busbar({ text: t("plan_done", ok, todo.length), frac: 1, link: "#fix-plan", done: true, error: ok < todo.length });
  renderPlan();
}

function renderRememberMode() {
  document.querySelectorAll(".remember-mode [data-mode]").forEach((b) => b.classList.toggle("active", b.dataset.mode === addressing()));
}

document.querySelector(".remember-mode").addEventListener("click", (ev) => {
  const mode = ev.target.closest("[data-mode]")?.dataset.mode;
  if (!mode || mode === addressing() || !tree) return;
  if (meters.length && !confirm(t("confirm_mode", mode === "primary" ? "PA" : "SA", meters.length))) return;
  guarded(ev.target.closest("button"), async () => {
    const r = await api(`lines/${tree.active.line.id}/addressing`, { method: "PUT", body: { mode } });
    await loadMeters();
    renderRememberMode();
    const msg = t("mode_done", mode === "primary" ? "PA" : "SA", r.changed);
    if (r.kept.length) toast(`${msg} · ${t("mode_kept", r.kept.length)}: ${r.kept.join("; ")}`, true);
    else toast(msg);
  });
});

$("remember-all").onclick = () =>
  guarded($("remember-all"), async () => {
    // nur Zähler, die nach der Einstellung der Linie eindeutig sind: keine gemischte Liste
    const ok = (lastJob?.hits || []).filter((h) => h.status === "ok");
    const list = ok.filter((h) => rememberAddress(h).address).map(meterFromHit);
    const skipped = ok.length - list.length;
    const r = await api("meters/import", { method: "POST", body: { meters: list } });
    toast(t("remembered_all", r.added, list.length - r.added) + (skipped ? " · " + t("remember_skipped", skipped) : ""), skipped > 0);
    await loadMeters();
  });

// ------------------------------------------------------------ Sortieren der Trefferliste

/// Spalte und Richtung (1 aufsteigend, -1 absteigend); null = Reihenfolge des Scans
let hitSort = null;
try {
  hitSort = JSON.parse(storageGet("hit_sort") || "null");
} catch {
  /* ungültig gespeichert */
}

const SORT_KEYS = {
  col_primary: (h) => h.primary_address,
  col_baud: (h) => h.baud,
  col_id: (h) => h.id,
  col_secondary: (h) => h.secondary_address,
  col_manufacturer: (h) => h.manufacturer || null,
  col_version: (h) => h.version,
  col_medium: (h) => (h.medium ? label(h.medium) : null),
  // Probleme zuerst
  col_status: (h) => ({ collision: 0, no_data: 1, ok: 2 })[h.status],
};

/// [Treffer, Index im Scan]; der Index bleibt für Merken, Warum? usw. gültig
function sortedHits(hits) {
  const order = hits.map((h, i) => [h, i]);
  const key = hitSort && SORT_KEYS[hitSort.col];
  if (!key) return order;
  return order.sort(([a, ia], [b, ib]) => {
    const x = key(a);
    const y = key(b);
    // leere Werte immer unten, egal in welche Richtung
    if (x == null || y == null) return x == null && y == null ? ia - ib : x == null ? 1 : -1;
    const c =
      typeof x === "number" && typeof y === "number"
        ? x - y
        : String(x).localeCompare(String(y), undefined, { numeric: true });
    return c * hitSort.dir || ia - ib;
  });
}

$("hits").querySelector("thead").addEventListener("click", (ev) => {
  const col = ev.target.closest("th[data-sort]")?.dataset.sort;
  if (!col) return;
  // auf → ab → wie gescannt
  hitSort = hitSort?.col !== col ? { col, dir: 1 } : hitSort.dir > 0 ? { col, dir: -1 } : null;
  storageSet("hit_sort", JSON.stringify(hitSort));
  if (lastJob) renderJob(lastJob);
});

$("sa-raw").checked = storageGet("sa_raw") === "1";
$("sa-raw").onchange = () => {
  storageSet("sa_raw", $("sa-raw").checked ? "1" : "0");
  if (lastJob) renderJob(lastJob);
};

$("hits").addEventListener("click", (ev) => {
  const an = ev.target.closest("[data-analyze]");
  if (an) {
    ev.preventDefault();
    analyzeHex(an.dataset.analyze).catch((e) => toast(e.message, true));
    return;
  }
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  if (d.why) {
    if (openWhy.has(d.why)) openWhy.delete(d.why);
    else openWhy.add(d.why);
    renderJob(lastJob);
    return;
  }
  if (d.fixbaud) {
    // Zähler unter "Zähler einstellen" eintragen, Ziel = Baudrate des Busses
    setConfigTarget(d.fixbaud, Number(d.from));
    $("new-baud").value = connBaud;
    spotlight("tile-baud", "switch-baud", t("spot_baud", d.fixbaud, d.from, connBaud));
    $("switch-baud").focus({ preventScroll: true });
    return;
  }
  if (d.fix) {
    // Zähler unter "Zähler einstellen" eintragen; freie Adresse fragt der Bus, nicht die Trefferliste:
    // Zähler ohne Daten haben dort keine PA, belegen sie aber trotzdem
    guarded(btn, async () => {
      setConfigTarget(d.fix, Number(d.from) || null);
      $("set-new").value = "";
      const skip = [...new Set(lastJob.hits.map((h) => h.primary_address).filter((a) => a != null && a > 0))];
      const r = await api("free-address", { method: "POST", body: { from: 1, skip } });
      $("set-new").value = r.address;
      spotlight("tile-pa", "set-addr", t("spot_pa", d.fix, r.address));
      $("set-addr").focus({ preventScroll: true });
    });
    return;
  }
  if (d.remember !== undefined) {
    const h = lastJob.hits[Number(d.remember)];
    guarded(btn, async () => {
      const a = rememberAddress(h);
      if (!a.address) throw new Error(a.reason);
      await api("meters", { method: "POST", body: meterFromHit(h) });
      toast(t("remembered"));
      await loadMeters();
    });
    return;
  }
  if (!d.read) return;
  $("read-addr").value = d.read;
  $("set-old").value = d.read;
  reveal($("sec-read"), "start");
  readMeter(Number(d.baud) || null);
});

async function startScan(path, body) {
  await api(path, { method: "POST", body });
  jobRunning = true;
  updateButtons();
  pollJob();
}

$("scan-primary").onclick = () =>
  guarded($("scan-primary"), () =>
    startScan("scan/primary", {
      // leere Felder: Standardwerte statt 0
      from: Number($("scan-from").value || 0),
      to: Number($("scan-to").value || 250),
      retries: Number($("scan-retries").value || 1),
      bauds: pickedBauds(),
    })
  );

$("scan-secondary").onclick = () =>
  guarded($("scan-secondary"), () => startScan("scan/secondary", { bauds: pickedBauds() }));

// ------------------------------------------------------------ Baudraten für den Scan

$("scan-bauds").innerHTML = BAUDS.map((b) => `<label><input type="checkbox" value="${b}"><span>${b}</span></label>`).join("");
// bis zur Verbindung: Baudrate aus dem Verbindungsfeld
setPickedBauds([Number($("baud").value)]);

function pickedBauds() {
  const list = [...$("scan-bauds").querySelectorAll("input:checked")].map((i) => Number(i.value));
  if (!list.length) throw new Error(t("pick_baud"));
  return list;
}

function setPickedBauds(list) {
  $("scan-bauds").querySelectorAll("input").forEach((i) => (i.checked = list.includes(Number(i.value))));
  updateButtons();
  updateEstimate();
}

/// Obergrenze: jede Adresse ohne Antwort wartet den vollen Timeout ab, je Wiederholung
function updateEstimate() {
  const el = $("scan-estimate");
  const bauds = [...$("scan-bauds").querySelectorAll("input:checked")].map((i) => Number(i.value));
  if (!conn || !bauds.length) return (el.textContent = "");
  const from = Number($("scan-from").value || 0);
  const to = Number($("scan-to").value || 250);
  const tries = Number($("scan-retries").value || 1) + 1;
  const n = Math.max(0, to - from + 1);
  const secs = bauds.reduce((sum, b) => {
    const timeout = conn.timeout_auto ? 330 / b + 0.25 : conn.timeout_ms / 1000;
    return sum + n * tries * timeout;
  }, 0);
  el.textContent = t("estimate", t("duration", secs));
}

$("scan-bauds").addEventListener("change", () => {
  baudsTouched = true;
  updateEstimate();
  updateButtons();
});

$("bauds-all").onclick = () => {
  baudsTouched = true;
  const all = !$("scan-bauds").querySelector("input:not(:checked)");
  setPickedBauds(all ? [] : BAUDS);
};
["scan-from", "scan-to", "scan-retries"].forEach((id) => $(id).addEventListener("input", updateEstimate));

/// Baudraten einzeln abfragen, damit man sieht, was passiert
$("bus-bauds").onclick = async () => {
  const btn = $("bus-bauds");
  const out = $("bus-bauds-result");
  const chip = (b) => $("scan-bauds").querySelector(`input[value="${b}"] + span`);
  // immer alle Baudraten: dauert nur Sekunden, die Markierung gilt nur für den Scan
  const list = BAUDS;
  const found = [];
  checking = true;
  updateButtons();
  btn.textContent = t("bus_bauds_checking");
  BAUDS.forEach((b) => chip(b).classList.remove("found", "none", "checking"));
  try {
    for (const [k, b] of list.entries()) {
      out.textContent = t("bus_bauds_step", b, k + 1, list.length, found);
      busbar({ text: out.textContent, frac: k / list.length, link: "#sec-scan" });
      chip(b).classList.add("checking");
      const r = await api("bus-bauds", { method: "POST", body: { bauds: [b] } });
      chip(b).classList.remove("checking");
      if (r.bauds.length) found.push(b);
      chip(b).classList.add(r.bauds.length ? "found" : "none");
    }
    out.textContent = found.length ? t("bus_bauds_found", found.join(", "), connBaud) : t("bus_bauds_none");
    busbar({ text: out.textContent, frac: 1, link: "#sec-scan", done: true });
    // gefundene Baudraten für den Scan markieren
    if (found.length) {
      baudsTouched = true;
      setPickedBauds(found);
    }
  } catch (e) {
    BAUDS.forEach((b) => chip(b).classList.remove("checking"));
    out.textContent = "";
    busbar(null);
    toast(e.message, true);
  } finally {
    checking = false;
    btn.textContent = t("bus_bauds");
    updateButtons();
  }
};

$("scan-cancel").onclick = cancelJob;
