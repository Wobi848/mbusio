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
  renderPlantState();
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
    // der Anschluss ist jetzt bei der Linie gespeichert (Projekt-Karte)
    await loadTree();
  });

$("disconnect").onclick = () =>
  guarded($("disconnect"), async () => {
    const r = await api("disconnect", { method: "POST" });
    if (r.plan_paused) toast(t("plan_paused_disconnect"));
    await refreshStatus();
    loadTree().catch(() => {});
  });

/// „Uhrzeit vom Gerät übernehmen“ (Leiste unten und Zustand der Anlage)
function confirmTime(button) {
  const now = new Date();
  if (!confirm(t("confirm_time", now.toLocaleString(lang === "de" ? "de-CH" : "en-GB")))) return;
  guarded(button, async () => {
    const r = await api("time", { method: "POST", body: { epoch_ms: Date.now() } });
    toast(t("time_taken", Math.round(r.offset_s)));
    await refreshStatus();
  });
}

$("busbar-time").onclick = () => confirmTime($("busbar-time"));

$("refresh-ports").onclick = () => loadPorts().catch((e) => toast(e.message, true));

// ------------------------------------------------------------ Scan

let polling = false;
let renderedJob = null;
let renderedHits = 0;

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
  const chg = scanChanges(job);
  renderChanges(chg);
  table.hidden = job.hits.length === 0 && !chg?.missing.length;
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
      const dupPa = issues.rowDupPa.has(i);
      const dupId = issues.rowDupId.has(i);
      // Zeile mit Problem hervorheben: keine Daten, Kollision, doppelte PA/ID, fremde Baudrate
      const problem = h.status !== "ok" || dupPa || issues.rowDupIdSame.has(i) || other;
      const change = chg?.rows.get(i);
      const cls = [i >= fresh ? "new" : "", problem ? "issue" : "", change ? "chg" : ""].filter(Boolean).join(" ");
      const anim = (cls ? ` class="${cls}"` : "") + (i >= fresh ? ` style="animation-delay:${Math.min(i - fresh, 8) * 50}ms"` : "");
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
        <td>${ui.chip({ ok: "ok" }[h.status] || "warn", t("st_" + h.status))}${warnChips}${otherChip}${change ? " " + ui.chip("info", change.text) : ""}</td>
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
    .join("") +
    // fehlen diesmal: grau am Ende, gleiche Spalten
    (chg?.missing || [])
      .map((h) => {
        const td = (x, cls = "") => `<td class="${cls}">${esc(x ?? "")}</td>`;
        const ident = raw
          ? td(h.secondary_address ?? "–", "mono") + td(h.manufacturer)
          : td(h.id ?? "–", "mono") + td(h.manufacturer) + td(h.version, "num mono");
        return `<tr class="missing">${td(h.primary_address ?? "–", "num mono")}${td(h.baud, "num mono")}${ident}<td>${esc(label(h.medium))}</td><td>${ui.chip("warn", t("chg_missing"))}</td><td></td></tr>`;
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
const addressing = () => tree?.active.line.addressing || "primary";

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

// ------------------------------------------------------------ Vergleich mit dem letzten Scan

/// Gleicher Zähler: ID und Hersteller; ohne ID (Kollision, keine Daten) über Baudrate und PA
const hitIdentity = (h) => (h.id ? `${h.id}:${h.manufacturer}` : `pa:${h.baud}:${h.primary_address}`);

/// Unterschiede zum letzten vollständigen Scan gleicher Art und gleichen Bereichs (job.previous):
/// rows: Index → { kind: "new" | "changed", text }, missing: Treffer, die diesmal fehlen
function scanChanges(job) {
  const prev = job.previous;
  if (job.running || !prev) return null;
  const pool = new Map();
  for (const h of prev.hits) pool.set(hitIdentity(h), [...(pool.get(hitIdentity(h)) || []), h]);
  const rows = new Map();
  job.hits.forEach((h, i) => {
    const old = pool.get(hitIdentity(h))?.shift();
    if (!old) return rows.set(i, { kind: "new", text: t("chg_new") });
    const diff = [
      old.primary_address !== h.primary_address ? `PA ${old.primary_address ?? "–"} → ${h.primary_address ?? "–"}` : "",
      old.baud !== h.baud ? `${old.baud} → ${h.baud} Bd` : "",
      old.status !== h.status ? `${t("st_" + old.status)} → ${t("st_" + h.status)}` : "",
    ].filter(Boolean);
    if (diff.length) rows.set(i, { kind: "changed", text: t("chg_changed", diff.join(", ")) });
  });
  // eine frühere Kollision oder „keine Daten“ auf einer PA, die jetzt antwortet, fehlt nicht: sie ist aufgelöst
  const nowPa = new Set(job.hits.map((h) => `${h.baud}:${h.primary_address}`));
  const missing = [...pool.values()].flat().filter((h) => h.status === "ok" || !nowPa.has(`${h.baud}:${h.primary_address}`));
  return { rows, missing, time: prev.time };
}

function renderChanges(chg) {
  const el = $("hits-changes");
  el.hidden = !chg;
  if (!chg) return;
  const count = (k) => [...chg.rows.values()].filter((r) => r.kind === k).length;
  const [added, changed, gone] = [count("new"), count("changed"), chg.missing.length];
  el.innerHTML =
    added + changed + gone
      ? `<b>${esc(t("chg_since", slotText(chg.time)))}</b> ` +
        [added ? ui.chip("info", t("chg_n_new", added)) : "", gone ? ui.chip("warn", t("chg_n_missing", gone)) : "", changed ? ui.chip("info", t("chg_n_changed", changed)) : ""].join(" ")
      : esc(t("chg_none", slotText(chg.time)));
}

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
