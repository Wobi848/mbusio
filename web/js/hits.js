"use strict";

// Trefferliste des Scans: Zeilen, Adresse zum Merken, Filter, Sortieren, Vergleich mit dem letzten Scan

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

/// Eintrag für die Zählerliste aus einem Treffer
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

$("hits-block").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-start-secondary]")) {
    setScanKind("secondary");
    $("scan-secondary").click();
  }
  if (ev.target.closest("[data-fix-all]")) guarded(ev.target.closest("button"), openPlan);
  const f = ev.target.closest("[data-hit-filter]");
  if (f) {
    hitFilter = f.dataset.hitFilter;
    renderJob(lastJob);
  }
});

/// Filter der Trefferliste: alle, eindeutig, auffällig
let hitFilter = "all";

/// Kopf des Ergebnisses: Art und Anzahl, Filter, Satz mit den Hauptaktionen
function renderHitsHead(job, unique, attention, issues) {
  $("hits-meta").textContent = `${t("job_" + job.kind)} · ${t("found", job.hits.length)}`;
  const f = (k, n) => `<button type="button" data-hit-filter="${k}" class="${hitFilter === k ? "active" : ""}">${esc(t("hf_" + k))} <span class="seg-n${
    k === "attention" && n ? " warn" : ""
  }">${n}</span></button>`;
  $("hits-filter").innerHTML = f("all", job.hits.length) + f("unique", unique) + f("attention", attention);
  const done = !job.running && job.hits.length > 0;
  $("hits-action").hidden = !done;
  if (!done) return;
  const steps = planSteps(job.hits, issues).steps.length;
  // „Übernehmen“ nimmt alles, was über die gewählte Adresse (SA/PA) eindeutig ansprechbar ist,
  // also auch Zähler mit Auffälligkeit, z.B. gleiche ID bei „Merken über PA“: die Zahl sagt genau das
  const take = rememberable(job.hits).length;
  const extra = Math.max(0, take - unique);
  $("hits-action-text").innerHTML = t("hits_action", unique, attention - extra, extra, addressing() === "primary" ? "PA" : "SA");
  $("hits-fix").hidden = !steps;
  $("hits-fix").textContent = t("fix_n", steps);
  $("remember-all").hidden = !take;
  $("remember-all").innerHTML = `${esc(t("remember_n", take))} ${ICON_NEXT}`;
}

/// Treffer, die „Übernehmen“ in die Zählerliste nimmt: lesbar und über die gewählte Adresse
/// (SA oder PA) eindeutig ansprechbar
const rememberable = (hits) => hits.filter((h) => h.status === "ok" && rememberAddress(h).address);

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
    if (lastJob) renderJob(lastJob);
    const msg = t("mode_done", mode === "primary" ? "PA" : "SA", r.changed);
    if (r.kept.length) toast(`${msg} · ${t("mode_kept", r.kept.length)}: ${r.kept.join("; ")}`, true);
    else toast(msg);
  });
});

$("remember-all").onclick = () =>
  guarded($("remember-all"), async () => {
    // nur Zähler, die nach der Einstellung der Linie eindeutig sind: keine gemischte Liste
    const ok = (lastJob?.hits || []).filter((h) => h.status === "ok");
    const list = rememberable(lastJob?.hits || []).map(meterFromHit);
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
  col_meter: (h) => h.id || null,
  // Probleme zuerst
  col_status: (h) => ({ collision: 0, no_data: 1, ok: 2 })[h.status],
};

/// Treffer als Zähler-Baustein: Name aus der Zählerliste, sonst Medium · Hersteller; darunter
/// ID und Version, oder (Schalter) die Sekundäradresse roh als 16 Hex-Zeichen
function hitMeter(h) {
  const known = h.id && meters.find((m) => (m.meter_id || m.address) === h.id);
  const name = known?.name || (h.medium ? [label(h.medium), h.manufacturer].filter(Boolean).join(" · ") : t(h.status === "collision" ? "hit_many" : "hit_unknown"));
  const raw = $("sa-raw").checked;
  return ui.meter({
    name,
    ident: raw ? null : h.id,
    medium: h.medium,
    info: raw ? h.secondary_address : h.version != null ? `v${h.version}` : "",
  });
}

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
  renderTarget();
  reveal($("sec-read"), "start");
  readMeter(Number(d.baud) || null);
});
