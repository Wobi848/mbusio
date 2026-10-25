"use strict";

// Verbindung: Anschlüsse, Verbinden/Trennen, Zustand von mbusio alle paar Sekunden, Uhrzeit bestätigen

async function loadPorts() {
  const ports = await api("ports");
  const sel = $("port");
  const prev = sel.value;
  sel.innerHTML = ports.map((p) => `<option value="${esc(p.port)}">${esc(p.label ? `${p.dev || p.port} · ${p.label}` : p.port)}</option>`).join("");
  if (ports.some((p) => p.port === prev)) sel.value = prev;
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
  for (const o of Object.values(OUTPUT_UI)) o.fromStatus?.(s);
  outputsInfo = s.outputs || [];
  host = { background: !!s.background, os: s.os || "" };
  if (transfer.data && !$("sec-outputs").hidden) renderOutputState();
  renderPlantState();
  if (runsBefore && !planRuns.length) loadMeters().catch(() => {});
  // "Baudrate abweichend" hängt von der Bus-Baudrate ab: Trefferliste neu zeichnen, wenn sie sich ändert
  if (connBaud !== prevBaud && lastJob) renderJob(lastJob);
  renderBusbar();
  // Auswahl folgt der eingestellten Baudrate, bis man selbst etwas anklickt
  if (connBaud && connBaud !== prevBaud && !baudsTouched) setPickedBauds([connBaud]);
  updateEstimate();
  jobRunning = s.job_running;
  $("settings-version").textContent = `mbusio ${s.version}`;
  renderLineBar();
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
