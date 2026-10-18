"use strict";

// Auffälligkeiten der Trefferliste: Streifen mit Ursache und Abhilfe unter der Zeile, Liste der
// Auffälligkeiten (Leiste unten, Projekt, Bericht), „Alle beheben“

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

/// Streifen unter einer Zeile: was ist los (fett), was tun, Knöpfe, die helfen
function issueStrip(cols, what, tip, buttons = "") {
  return `<tr class="issue-strip"><td colspan="${cols}"><div class="strip">${ICON_ALERT}<span><b>${esc(what)}</b> ${esc(tip)}</span><span class="spacer"></span>${buttons}</div></td></tr>`;
}

/// Streifen eines einzelnen Treffers (Kollision, keine Daten, fremde Baudrate); "" = nichts
function hitStrip(h, cols, fixTarget) {
  const analyze = h.raw ? `<a href="#analysis" class="button small quiet" data-analyze="${esc(h.raw)}">${esc(t("why_analyze"))}</a>` : "";
  if (h.status === "collision") {
    const what = h.primary_address != null ? t("warn_collision_pa", h.primary_address, h.baud) : t("warn_collision_id", h.id);
    const btn = h.primary_address != null ? `<button class="small ghost needs-conn" data-start-secondary>${esc(t("start_secondary"))}</button>` : "";
    return issueStrip(cols, what, "", btn + analyze);
  }
  if (h.status === "no_data") {
    const [what, tip] = explainReason(h.reason);
    return issueStrip(cols, what, tip, analyze);
  }
  if (h.baud && h.baud !== connBaud && fixTarget)
    return issueStrip(
      cols,
      t("other_baud_title", h.baud, connBaud),
      "",
      `<button class="small ghost needs-conn" data-fixbaud="${esc(fixTarget)}" data-from="${h.baud}">${esc(t("fix_baud"))}</button>`
    );
  return "";
}

/// Streifen nach dem letzten Zähler einer Gruppe (gleiche PA, gleiche ID und Hersteller): Index → html
function groupStrips(hits, a, order, cols) {
  const out = new Map();
  const after = (list, html) => out.set(list.reduce((last, i) => (order.indexOf(i) > order.indexOf(last) ? i : last), list[0]), html);
  for (const [k, list] of a.dupPa) {
    const [baud, pa] = k.split(":");
    after(list, issueStrip(cols, t("warn_dup_pa", pa, baud, list.map((i) => hits[i].id || "?")), t("warn_fix_hint")));
  }
  for (const [id, list] of a.dupIdSame) after(list, (out.get(list.at(-1)) || "") + issueStrip(cols, t("plan_manual_sameid", id), ""));
  return out;
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

/// Auffälligkeiten als Liste: Zähler unten in der Leiste, bei der Linie gemerkt (Projekt, Bericht)
function noteIssues(hits, a) {
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
  issueCount = items.length;
  // erst wenn der Scan durch ist: bei der Linie merken (Projekt-Karte, Bericht)
  if (!lastJob?.running) saveIssues(items);
  renderBusbar();
}

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
  // gehört zu diesem Scan: ein neuer Scan beginnt wieder bei „beheben“ (fixState)
  plan = { ...planSteps(hits, analyzeHits(hits)), job: lastJob };
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
      ${!finished && !planRunning && plan.steps.length ? `<button class="needs-conn" data-plan-run ${checked ? "" : "disabled"}>${esc(t("plan_run", checked))}</button>` : ""}
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

/// Für „Nächster Schritt“: "fix" = der letzte Scan hat behebbare Auffälligkeiten, "check" = eben
/// behoben, zur Kontrolle neu scannen; sonst null
function fixState() {
  if (!lastJob || lastJob.running || lastJob.kind === "read_all") return null;
  if (plan?.job === lastJob && plan.steps.some((s) => s.state === "ok")) return "check";
  return planSteps(lastJob.hits, analyzeHits(lastJob.hits)).steps.length ? "fix" : null;
}

async function runPlan() {
  const todo = plan.steps.filter((s) => !s.skip);
  planRunning = true;
  renderPlan();
  // Baudraten zuerst: danach läuft der Zähler mit der Bus-Baudrate, auch für die Adressänderung
  todo.sort((x, y) => (x.kind === "baud" ? 0 : 1) - (y.kind === "baud" ? 0 : 1));
  const switched = new Set();
  for (const [k, s] of todo.entries()) {
    busbar({ text: t("fix_running", k + 1, todo.length), frac: k / todo.length, link: "#fix-plan" });
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
