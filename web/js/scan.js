"use strict";

// Scan: Auftrag starten und verfolgen, Art des Scans, Baudraten (Trefferliste in hits.js,
// Verbindung in connection.js)

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
      // Zählerliste während "Alle auslesen" mitlaufend aktualisieren (vor dem Zeichnen: die
      // Zusammenfassung am Ende braucht den neuen Stand)
      if (job.kind === "read_all") await loadMeters().catch(() => {});
      renderJob(job);
      if (!job.running) {
        jobRunning = false;
        updateButtons();
        const cancelled = cancelRequested;
        cancelRequested = false;
        const done =
          job.kind === "read_all"
            ? cancelled
              ? t("read_all_cancelled")
              : readAllSummary()
            : t(cancelled ? "job_cancelled" : "job_done", t("job_" + job.kind), job.hits.length);
        // „Alle auslesen“ mit Zählern, die nicht antworten: rot wie ein Fehler
        const missing = job.kind === "read_all" && !cancelled && meters.some((m) => m.last_error);
        busbar({
          text: job.error || done,
          frac: 1,
          link: jobLink(job),
          done: true,
          error: !!job.error || missing,
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
      : `${t("job_read_all")}: ${job.error || (cancelRequested ? job.message : readAllSummary())}`;
    updateButtons();
    return;
  }
  // Fortschritt nur während der Suche; danach steht alles im Kopf des Ergebnisses
  $("job").hidden = !job.running;
  renderResume();
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
  $("hits-block").hidden = false;
  const chg = scanChanges(job);
  renderChanges(chg);
  table.hidden = job.hits.length === 0 && !chg?.missing.length;
  const issues = analyzeHits(job.hits);
  noteIssues(job.hits, issues);
  // Zeile mit Problem: keine Daten, Kollision, doppelte PA, gleiche ID und Hersteller, fremde Baudrate
  const problem = (h, i) => h.status !== "ok" || issues.rowDupPa.has(i) || issues.rowDupIdSame.has(i) || (h.baud && h.baud !== connBaud);
  const attention = job.hits.filter(problem).length;
  renderHitsHead(job, job.hits.length - attention, attention, issues);
  document.querySelector(".hits-opt").hidden = table.hidden;
  const cols = ["col_primary", "col_meter", "col_baud", "col_status", ""];
  const numCols = ["col_primary", "col_baud"];
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
  const order = sortedHits(job.hits).filter(([h, i]) => hitFilter === "all" || (hitFilter === "attention") === !!problem(h, i));
  const groups = groupStrips(job.hits, issues, order.map(([, i]) => i), cols.length);
  table.querySelector("tbody").innerHTML = order
    .map(([h, i]) => {
      const target = h.status === "ok" ? hitAddress(h) : "";
      const other = h.baud && h.baud !== connBaud;
      const dupPa = issues.rowDupPa.has(i);
      const dupId = issues.rowDupId.has(i);
      const change = chg?.rows.get(i);
      // Problem: orange, wenn der Zähler trotzdem übernommen wird, sonst rot
      const kept = rememberable([h]).length > 0;
      const cls = [i >= fresh ? "new" : "", problem(h, i) ? (kept ? "issue" : "issue fault") : "", change ? "chg" : ""].filter(Boolean).join(" ");
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
      const otherChip = other && h.status === "ok"
        ? " " + ui.chip("warn", t("other_baud"), t("other_baud_title", h.baud, connBaud))
        : "";
      return `<tr${anim}>
        <td class="num mono${dupPa ? " st-other" : ""}"${dupPa ? ` title="${esc(t("dup_pa_title", h.primary_address, paCount))}"` : ""}>${h.primary_address ?? "–"}${dupPa ? " ⚠" : ""}</td>
        <td>${hitMeter(h)}</td>
        <td class="num mono${other ? " st-other" : ""}">${h.baud ?? ""}</td>
        <td>${ui.chip(h.status === "ok" ? "ok" : "err", t("st_" + h.status))}${warnChips}${otherChip}${change ? " " + ui.chip("info", change.text) : ""}</td>
        <td><div class="actions">${fix}${
          target !== ""
            ? `<button class="small quiet needs-conn" data-read="${esc(target)}" data-baud="${h.baud ?? ""}">${t("read")}</button>
               <button class="small ghost" data-remember="${i}">${t("remember")}</button>`
            : ""
        }</div></td>
      </tr>${hitStrip(h, cols.length, fixTarget, !kept)}${groups.get(i) || ""}`;
    })
    .join("") +
    // fehlen diesmal: grau am Ende, gleiche Spalten
    (chg?.missing || [])
      .map((h) => {
        return `<tr class="missing"><td class="num mono">${h.primary_address ?? "–"}</td><td>${hitMeter(h)}</td><td class="num mono">${
          h.baud ?? ""
        }</td><td>${ui.chip("warn", t("chg_missing"))}</td><td></td></tr>`;
      })
      .join("");
  updateButtons();
}

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

/// Anfang der ID für den Sekundärscan: nur Ziffern, höchstens 7
function scanPrefix() {
  const p = $("scan-prefix").value.trim();
  if (!/^\d{0,7}$/.test(p)) throw new Error(t("scan_prefix_bad"));
  return p;
}

$("scan-secondary").onclick = () =>
  guarded($("scan-secondary"), () => startScan("scan/secondary", { bauds: pickedBauds(), prefix: scanPrefix() }));

// abgebrochenen Sekundärscan dort fortsetzen, wo er stand; bisherige Treffer bleiben
$("scan-resume").onclick = () =>
  guarded($("scan-resume"), () => startScan("scan/secondary", { bauds: pickedBauds(), prefix: scanPrefix(), resume: lastJob.resume }));

/// „Weitersuchen“ nur nach einem abgebrochenen Sekundärscan
function renderResume() {
  const r = scanKind === "secondary" && lastJob?.kind === "secondary" && !lastJob.running && lastJob.resume;
  $("scan-resume").hidden = !r;
  if (r) $("scan-resume").textContent = t("scan_resume", r.mask.replace(/F+$/, "") || "0", r.baud || "");
}

// ------------------------------------------------------------ Art des Scans

/// Primärscan (Adressbereich) oder Sekundärscan (über die ID); gemerkt im Browser
let scanKind = storageGet("scan_kind") === "secondary" ? "secondary" : "primary";

function setScanKind(kind) {
  scanKind = kind;
  storageSet("scan_kind", kind);
  document.querySelectorAll("#scan-kind [data-kind]").forEach((b) => b.classList.toggle("active", b.dataset.kind === kind));
  document.querySelectorAll("#sec-scan [data-for]").forEach((el) => (el.hidden = el.dataset.for !== kind));
  renderResume();
}

$("scan-kind").onclick = (ev) => {
  const b = ev.target.closest("[data-kind]");
  if (b) setScanKind(b.dataset.kind);
};

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
