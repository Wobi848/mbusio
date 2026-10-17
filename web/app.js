"use strict";

// ------------------------------------------------------------ Sprache

const UI = {
  de: {
    disconnected: "Getrennt",
    connection: "Verbindung",
    port: "Port",
    reload_ports: "Ports neu laden",
    port_manual: "Oder manuell",
    baud: "Baudrate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatisch",
    connect: "Verbinden",
    disconnect: "Trennen",
    connection_hint: "8 Datenbits, gerade Parität, 1 Stopbit. „SIMULATOR“ startet einen virtuellen Bus zum Testen.",
    scan: "Scan",
    from: "Von",
    to: "Bis",
    retries: "Wiederholungen",
    scan_primary: "Primärscan",
    scan_secondary: "Sekundärscan",
    cancel: "Abbrechen",
    col_primary: "Primär",
    col_secondary: "Sekundäradresse",
    col_manufacturer: "Hersteller",
    col_medium: "Medium",
    col_status: "Status",
    read_meter: "Zähler auslesen",
    address: "Adresse",
    address_ph: "Primär (0–250), ID (8) oder Sekundär (16)",
    address_ph_short: "Primär, ID oder Sekundär",
    read: "Auslesen",
    reading_busy: "Lese …",
    set_primary: "Primäradresse setzen",
    current_address: "Aktuelle Adresse",
    new_primary: "Neue Primäradresse",
    set: "Setzen",
    set_hint: "Bei mehreren Zählern auf derselben Adresse (z.B. 0) die Sekundäradresse oder ID verwenden.",
    readings_export: "Messwerte & Export",
    readings_count: (n) => `${n} Auslesung${n === 1 ? "" : "en"} gespeichert`,
    delete: "Löschen",
    telegram_log: "Telegramm-Log",
    clear_log: "Log leeren",
    follow: "mitscrollen",
    connected_to: (p) => `Verbunden mit ${p}`,
    job_primary: "Primärscan",
    job_secondary: "Sekundärscan",
    found: (n) => `${n} gefunden`,
    hits: (n) => `${n} Treffer`,
    st_ok: "OK",
    st_collision: "Kollision",
    st_no_data: "Keine Daten",
    meta_id: "ID",
    meta_manufacturer: "Hersteller",
    meta_medium: "Medium",
    meta_version: "Version",
    meta_primary: "Primäradresse",
    meta_secondary: "Sekundäradresse",
    meta_access: "Zugriffsnr.",
    meta_status: "Status",
    meta_time: "Zeitpunkt",
    th_desc: "Beschreibung",
    th_value: "Wert",
    th_unit: "Einheit",
    th_kind: "Art",
    storage: "Speicher",
    tariff: "Tarif",
    subunit: "Untereinheit",
    enter_address: "Adresse eingeben",
    enter_both: "Aktuelle und neue Adresse eingeben",
    confirm_set: (a, n) => `Primäradresse von ${a} auf ${n} ändern?`,
    address_set: (n) => `Neue Primäradresse ${n} gesetzt`,
    confirm_clear: "Alle gespeicherten Auslesungen löschen?",
    token_prompt: "Zugriffsschlüssel (Token) eingeben:",
  },
  en: {
    disconnected: "Disconnected",
    connection: "Connection",
    port: "Port",
    reload_ports: "Reload ports",
    port_manual: "Or enter manually",
    baud: "Baud rate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatic",
    connect: "Connect",
    disconnect: "Disconnect",
    connection_hint: "8 data bits, even parity, 1 stop bit. “SIMULATOR” starts a virtual bus for testing.",
    scan: "Scan",
    from: "From",
    to: "To",
    retries: "Retries",
    scan_primary: "Primary scan",
    scan_secondary: "Secondary scan",
    cancel: "Cancel",
    col_primary: "Primary",
    col_secondary: "Secondary address",
    col_manufacturer: "Manufacturer",
    col_medium: "Medium",
    col_status: "Status",
    read_meter: "Read meter",
    address: "Address",
    address_ph: "Primary (0–250), ID (8) or secondary (16)",
    address_ph_short: "Primary, ID or secondary",
    read: "Read",
    reading_busy: "Reading …",
    set_primary: "Set primary address",
    current_address: "Current address",
    new_primary: "New primary address",
    set: "Set",
    set_hint: "If several meters share an address (e.g. 0), use the secondary address or ID.",
    readings_export: "Readings & export",
    readings_count: (n) => `${n} reading${n === 1 ? "" : "s"} stored`,
    delete: "Delete",
    telegram_log: "Telegram log",
    clear_log: "Clear log",
    follow: "auto-scroll",
    connected_to: (p) => `Connected to ${p}`,
    job_primary: "Primary scan",
    job_secondary: "Secondary scan",
    found: (n) => `${n} found`,
    hits: (n) => `${n} hit${n === 1 ? "" : "s"}`,
    st_ok: "OK",
    st_collision: "Collision",
    st_no_data: "No data",
    meta_id: "ID",
    meta_manufacturer: "Manufacturer",
    meta_medium: "Medium",
    meta_version: "Version",
    meta_primary: "Primary address",
    meta_secondary: "Secondary address",
    meta_access: "Access no.",
    meta_status: "Status",
    meta_time: "Timestamp",
    th_desc: "Description",
    th_value: "Value",
    th_unit: "Unit",
    th_kind: "Type",
    storage: "Storage",
    tariff: "Tariff",
    subunit: "Subunit",
    enter_address: "Enter an address",
    enter_both: "Enter current and new address",
    confirm_set: (a, n) => `Change primary address from ${a} to ${n}?`,
    address_set: (n) => `New primary address ${n} set`,
    confirm_clear: "Delete all stored readings?",
    token_prompt: "Enter access token:",
  },
};

function storageGet(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function storageSet(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* privater Modus */
  }
}

let lang = storageGet("lang") || (navigator.language || "de").slice(0, 2);
if (!UI[lang]) lang = "de";
let labels = {}; // Messdaten-Begriffe vom Server
let token = storageGet("token") || "";

function t(key, ...args) {
  const v = UI[lang][key] ?? UI.de[key] ?? key;
  return typeof v === "function" ? v(...args) : v;
}

/// Schlüssel vom Server übersetzen, z.B. "note_error_code:05"
function label(key) {
  if (!key) return "";
  const [base, arg] = key.split(":");
  const text = labels[base] ?? base;
  return arg ? `${text} ${arg}` : text;
}

async function setLang(l) {
  lang = l;
  storageSet("lang", l);
  document.documentElement.lang = l;
  document.querySelectorAll("[data-lang]").forEach((b) => b.classList.toggle("active", b.dataset.lang === l));
  document.querySelectorAll("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n)));
  document.querySelectorAll("[data-i18n-ph]").forEach((el) => (el.placeholder = t(el.dataset.i18nPh)));
  document.querySelectorAll("[data-i18n-title]").forEach((el) => (el.title = t(el.dataset.i18nTitle)));
  labels = await api("labels?lang=" + l).catch(() => ({}));
  updateExportLinks();
  if (lastReading) renderReading(lastReading);
  if (lastJob) renderJob(lastJob);
  refreshStatus().catch(() => {});
}

document.querySelectorAll("[data-lang]").forEach((b) => (b.onclick = () => setLang(b.dataset.lang)));

// ------------------------------------------------------------ Helfer

const $ = (id) => document.getElementById(id);
let connected = false;
let jobRunning = false;
let logSeq = 0;
let lastReading = null;
let lastJob = null;

async function api(path, options = {}) {
  const headers = { "Content-Type": "application/json", "X-Lang": lang };
  if (token) headers["X-Token"] = token;
  const opts = { ...options, headers };
  if (opts.body && typeof opts.body !== "string") opts.body = JSON.stringify(opts.body);
  const res = await fetch("api/" + path, opts);
  if (res.status === 401 && !options.retried) {
    const entered = prompt(t("token_prompt"));
    if (entered) {
      token = entered.trim();
      storageSet("token", token);
      updateExportLinks();
      return api(path, { ...options, retried: true });
    }
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

function updateExportLinks() {
  const extra = `&lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  $("export-csv").href = "api/export?format=csv" + extra;
  $("export-json").href = "api/export?format=json" + extra;
}

function toast(msg, isError = false) {
  const el = $("toast");
  el.textContent = msg;
  el.className = isError ? "error" : "";
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), isError ? 6000 : 3000);
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
    badge.textContent = t("disconnected");
    badge.className = "badge off";
  }
  $("readings-count").textContent = t("readings_count", s.readings);
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
        else toast(`${t("job_" + job.kind)}: ${t("hits", job.hits.length)}`);
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
  lastJob = job;
  $("job").hidden = false;
  const p = $("job-progress");
  p.max = Math.max(job.total, 1);
  p.value = job.done;
  $("job-msg").textContent = `${t("job_" + job.kind)}: ${job.message} · ${t("found", job.hits.length)}`;
  const table = $("hits");
  table.hidden = job.hits.length === 0;
  table.querySelector("tbody").innerHTML = job.hits
    .map((h) => {
      const target = h.status === "ok" ? h.secondary_address || h.primary_address : "";
      return `<tr>
        <td class="num">${h.primary_address ?? "–"}</td>
        <td class="mono">${esc(h.secondary_address ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>
        <td>${esc(label(h.medium))}</td>
        <td class="st-${h.status}">${t("st_" + h.status)}</td>
        <td>${target !== "" ? `<button class="small ghost needs-conn" data-read="${esc(target)}">${t("read")}</button>` : ""}</td>
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
  lastReading = r;
  const h = r.header || {};
  const meta = [
    ["meta_id", h.id],
    ["meta_manufacturer", h.manufacturer],
    ["meta_medium", h.medium && label(h.medium)],
    ["meta_version", h.version],
    ["meta_primary", r.primary_address],
    ["meta_secondary", h.secondary_address],
    ["meta_access", h.access_no],
    ["meta_status", h.status !== undefined ? "0x" + h.status.toString(16).padStart(2, "0").toUpperCase() : undefined],
    ["meta_time", r.timestamp],
  ]
    .filter(([, v]) => v !== undefined && v !== null)
    .map(([k, v]) => `<div><span>${t(k)}</span><span class="mono">${esc(v)}</span></div>`)
    .join("");
  const rows = r.records
    .map((x, i) => {
      const extra = [];
      if (x.storage) extra.push(`${t("storage")} ${x.storage}`);
      if (x.tariff) extra.push(`${t("tariff")} ${x.tariff}`);
      if (x.subunit) extra.push(`${t("subunit")} ${x.subunit}`);
      const notes = x.notes.length ? ` (${x.notes.map(label).join(", ")})` : "";
      return `<tr>
        <td class="num">${i + 1}</td>
        <td>${esc(label(x.quantity) + notes)}</td>
        <td class="num mono">${esc(x.value_text)}</td>
        <td>${esc(x.unit)}</td>
        <td>${esc(label(x.function))}${extra.length ? " · " + extra.join(", ") : ""}</td>
        <td class="mono">${esc(x.dif)} | ${esc(x.vif)}</td>
      </tr>`;
    })
    .join("");
  $("reading").innerHTML = `
    <div class="meta">${meta}</div>
    <div class="table-wrap"><table class="reading-table">
      <thead><tr><th>#</th><th>${t("th_desc")}</th><th>${t("th_value")}</th><th>${t("th_unit")}</th><th>${t("th_kind")}</th><th>DIF | VIF</th></tr></thead>
      <tbody>${rows}</tbody>
    </table></div>`;
}

async function readMeter() {
  await guarded($("read"), async () => {
    const address = $("read-addr").value.trim();
    if (!address) throw new Error(t("enter_address"));
    lastReading = null;
    $("reading").innerHTML = `<p class="hint">${t("reading_busy")}</p>`;
    try {
      renderReading(await api("read", { method: "POST", body: { address } }));
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
    if (!address || newAddr === "") throw new Error(t("enter_both"));
    if (!confirm(t("confirm_set", address, newAddr))) return;
    await api("set-address", { method: "POST", body: { address, new_address: Number(newAddr) } });
    toast(t("address_set", newAddr));
  });

// ------------------------------------------------------------ Messwerte

$("clear-readings").onclick = () =>
  guarded($("clear-readings"), async () => {
    if (!confirm(t("confirm_clear"))) return;
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

setLang(lang);
loadPorts().catch((e) => toast(e.message, true));
setInterval(() => pollLog().catch(() => {}), 1000);
setInterval(() => !jobRunning && refreshStatus().catch(() => {}), 5000);
