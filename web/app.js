"use strict";

const $ = (id) => document.getElementById(id);
let connected = false;
let jobRunning = false;
let logSeq = 0;

async function api(path, options = {}) {
  const opts = { ...options, headers: { "Content-Type": "application/json" } };
  if (opts.body && typeof opts.body !== "string") opts.body = JSON.stringify(opts.body);
  const res = await fetch("api/" + path, opts);
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

function toast(msg, isError = false) {
  const t = $("toast");
  t.textContent = msg;
  t.className = isError ? "error" : "";
  t.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.hidden = true), isError ? 6000 : 3000);
}

function esc(s) {
  return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

async function guarded(button, fn) {
  button.disabled = true;
  try {
    await fn();
  } catch (e) {
    toast(e.message, true);
  } finally {
    updateButtons();
  }
}

function updateButtons() {
  document.querySelectorAll(".needs-conn").forEach((b) => (b.disabled = !connected || jobRunning));
  $("connect").disabled = jobRunning;
  $("disconnect").disabled = !connected || jobRunning;
  $("scan-cancel").hidden = !jobRunning;
}

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
  jobRunning = s.job_running;
  const badge = $("conn-badge");
  if (connected) {
    const c = s.connection;
    badge.textContent = `${c.port} · ${c.baud} Bd`;
    badge.className = "badge on";
  } else {
    badge.textContent = "Getrennt";
    badge.className = "badge off";
  }
  $("readings-count").textContent = `${s.readings} Auslesung${s.readings === 1 ? "" : "en"} gespeichert`;
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
    toast(`Verbunden mit ${port}`);
    await refreshStatus();
  });

$("disconnect").onclick = () =>
  guarded($("disconnect"), async () => {
    await api("disconnect", { method: "POST" });
    await refreshStatus();
  });

$("refresh-ports").onclick = () => loadPorts().catch((e) => toast(e.message, true));

// ------------------------------------------------------------ Scan

let polling = false;

async function pollJob() {
  if (polling) return;
  polling = true;
  try {
    for (;;) {
      const job = await api("job");
      renderJob(job);
      if (!job.running) {
        jobRunning = false;
        updateButtons();
        if (job.error) toast(job.error, true);
        else toast(`${job.kind}: ${job.hits.length} Treffer`);
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
  $("job").hidden = false;
  const p = $("job-progress");
  p.max = Math.max(job.total, 1);
  p.value = job.done;
  $("job-msg").textContent = `${job.kind}: ${job.message} · ${job.hits.length} gefunden`;
  const table = $("hits");
  table.hidden = job.hits.length === 0;
  table.querySelector("tbody").innerHTML = job.hits
    .map((h) => {
      const target = h.status === "ok" ? h.secondary_address || h.primary_address : "";
      const statusText = { ok: "OK", kollision: "Kollision", keine_daten: "Keine Daten" }[h.status] || h.status;
      return `<tr>
        <td class="num">${h.primary_address ?? "–"}</td>
        <td class="mono">${esc(h.secondary_address ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>
        <td>${esc(h.medium ?? "")}</td>
        <td class="st-${h.status}">${statusText}</td>
        <td>${target !== "" ? `<button class="small ghost needs-conn" data-read="${esc(target)}">Auslesen</button>` : ""}</td>
      </tr>`;
    })
    .join("");
  updateButtons();
}

$("hits").addEventListener("click", (ev) => {
  const addr = ev.target.dataset?.read;
  if (!addr) return;
  $("read-addr").value = addr;
  $("set-old").value = addr;
  readMeter();
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
      from: Number($("scan-from").value),
      to: Number($("scan-to").value),
      retries: Number($("scan-retries").value),
    })
  );

$("scan-secondary").onclick = () => guarded($("scan-secondary"), () => startScan("scan/secondary", {}));

$("scan-cancel").onclick = () => api("job/cancel", { method: "POST" }).catch((e) => toast(e.message, true));

// ------------------------------------------------------------ Auslesen

function renderReading(r) {
  const h = r.header || {};
  const meta = [
    ["ID", h.id],
    ["Hersteller", h.manufacturer],
    ["Medium", h.medium],
    ["Version", h.version],
    ["Primäradresse", r.primary_address],
    ["Sekundäradresse", h.secondary_address],
    ["Zugriffsnr.", h.access_no],
    ["Status", h.status !== undefined ? "0x" + h.status.toString(16).padStart(2, "0").toUpperCase() : undefined],
    ["Zeitpunkt", r.timestamp],
  ]
    .filter(([, v]) => v !== undefined && v !== null)
    .map(([k, v]) => `<div><span>${k}</span><span class="mono">${esc(v)}</span></div>`)
    .join("");
  const rows = r.records
    .map((x, i) => {
      const extra = [];
      if (x.storage) extra.push(`Speicher ${x.storage}`);
      if (x.tariff) extra.push(`Tarif ${x.tariff}`);
      if (x.subunit) extra.push(`Einheit ${x.subunit}`);
      return `<tr>
        <td class="num">${i + 1}</td>
        <td>${esc(x.description)}</td>
        <td class="num mono">${esc(x.value_text)}</td>
        <td>${esc(x.unit)}</td>
        <td>${esc(x.function)}${extra.length ? " · " + extra.join(", ") : ""}</td>
        <td class="mono">${esc(x.dif)} | ${esc(x.vif)}</td>
      </tr>`;
    })
    .join("");
  $("reading").innerHTML = `
    <div class="meta">${meta}</div>
    <div class="table-wrap"><table class="reading-table">
      <thead><tr><th>#</th><th>Beschreibung</th><th>Wert</th><th>Einheit</th><th>Art</th><th>DIF | VIF</th></tr></thead>
      <tbody>${rows}</tbody>
    </table></div>`;
}

async function readMeter() {
  const btn = $("read");
  await guarded(btn, async () => {
    const address = $("read-addr").value.trim();
    if (!address) throw new Error("Adresse eingeben");
    $("reading").innerHTML = '<p class="hint">Lese …</p>';
    try {
      const r = await api("read", { method: "POST", body: { address } });
      renderReading(r);
      await refreshStatus();
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    }
  });
}

$("read").onclick = readMeter;
$("read-addr").addEventListener("keydown", (e) => e.key === "Enter" && readMeter());

// ------------------------------------------------------------ Adresse setzen

$("set-addr").onclick = () =>
  guarded($("set-addr"), async () => {
    const address = $("set-old").value.trim();
    const newAddr = $("set-new").value;
    if (!address || newAddr === "") throw new Error("Aktuelle und neue Adresse eingeben");
    if (!confirm(`Primäradresse von ${address} auf ${newAddr} ändern?`)) return;
    await api("set-address", { method: "POST", body: { address, new_address: Number(newAddr) } });
    toast(`Neue Primäradresse ${newAddr} gesetzt`);
  });

// ------------------------------------------------------------ Messwerte

$("clear-readings").onclick = () =>
  guarded($("clear-readings"), async () => {
    if (!confirm("Alle gespeicherten Auslesungen löschen?")) return;
    await api("readings", { method: "DELETE" });
    await refreshStatus();
  });

// ------------------------------------------------------------ Log

async function pollLog() {
  if (!$("log-box").open) return;
  const entries = await api("log?since=" + logSeq);
  if (!entries.length) return;
  logSeq = entries[entries.length - 1].seq;
  const pre = $("log");
  pre.insertAdjacentHTML(
    "beforeend",
    entries.map((e) => `<span class="log-${e.dir}">${e.time} ${e.dir.padEnd(4)} ${esc(e.text)}</span>\n`).join("")
  );
  while (pre.childElementCount > 1000) pre.firstElementChild.remove();
  if ($("log-follow").checked) pre.scrollTop = pre.scrollHeight;
}

$("clear-log").onclick = async () => {
  await api("log", { method: "DELETE" }).catch(() => {});
  $("log").innerHTML = "";
};

// ------------------------------------------------------------ Start

loadPorts().catch((e) => toast(e.message, true));
refreshStatus().catch((e) => toast(e.message, true));
setInterval(() => pollLog().catch(() => {}), 1000);
setInterval(() => !jobRunning && refreshStatus().catch(() => {}), 5000);
