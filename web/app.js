"use strict";

// ------------------------------------------------------------ Sprache

const UI = {
  de: {
    disconnected: "Getrennt",
    nav_read: "Auslesen",
    nav_config: "Einstellen",
    nav_export: "Export",
    nav_log: "Log",
    connection: "Verbindung",
    port: "Port",
    reload_ports: "Ports neu laden",
    port_manual: "Oder manuell",
    baud: "Baudrate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatisch",
    connect: "Verbinden",
    disconnect: "Trennen",
    connection_hint:
      "8 Datenbits, gerade Parität, 1 Stopbit. Netzwerk-Gateways als tcp://IP:Port angeben (Baudrate dann am Gateway einstellen). „SIMULATOR“ startet einen virtuellen Bus zum Testen.",
    scan: "Scan",
    from: "Von Adresse",
    to: "Bis Adresse",
    retries: "Wiederholungen",
    scan_primary: "Primärscan (PA)",
    scan_secondary: "Sekundärscan (SA)",
    scan_primary_start: "Primärscan starten",
    scan_secondary_start: "Sekundärscan starten",
    scan_primary_hint:
      "Fragt jede Primäradresse im Bereich einzeln ab. Schnell, aber Zähler mit gleicher Adresse (z.B. alle auf 0) erscheinen nur als Kollision.",
    scan_secondary_hint:
      "Sucht über die ID der Zähler, unabhängig von der Primäradresse. Findet auch Zähler, die sich eine Adresse teilen. Dauert länger, kein Bereich nötig.",
    cancel: "Abbrechen",
    scan_bauds: "Baudraten:",
    scan_bauds_hint:
      "„Prüfen“ fragt in wenigen Sekunden alle Baudraten ab und markiert die, auf denen Geräte antworten (●). Gescannt wird nur auf den markierten Baudraten.",
    bauds_all: "Alle",
    busbar_view: "Ansehen",
    job_done: (k, n) => `${k} fertig · ${n} Treffer`,
    job_cancelled: (k, n) => `${k} abgebrochen · ${n} Treffer`,
    busy_read: "Lese Zähler …",
    busy_set: "Setze Primäradresse …",
    busy_baud: "Stelle Baudrate um …",
    busy_detect: "Suche Baudrate …",
    busy_reset: "Sende Reset …",
    busy_raw: "Sende Telegramm …",
    bauds_none: "Keine",
    bus_bauds: "Prüfen",
    pick_baud: "Für den Scan mindestens eine Baudrate markieren",
    estimate: (t) => `Primärscan dauert höchstens ca. ${t}`,
    bus_bauds_busy: "Prüfe 300 bis 38400 Bd …",
    bus_bauds_step: (b, k, n, found) => `Prüfe ${b} Bd … (${k}/${n})` + (found.length ? ` · Antwort auf ${found.join(", ")} Bd` : ""),
    bus_bauds_checking: "Prüfe …",
    theme_auto: "Design wie System",
    theme_light: "Hell",
    theme_dark: "Dunkel",
    bus_bauds_none: "Auf keiner Baudrate antwortet ein Gerät",
    bus_bauds_found: (l, b) => `Geräte antworten auf: ${l} Bd` + (l.split(", ").some((x) => Number(x) !== b) ? " – Achtung, nicht nur auf der eingestellten Baudrate!" : ""),
    duration: (s) => (s < 90 ? `${Math.ceil(s)} s` : `${Math.ceil(s / 60)} min`),
    col_baud: "Baud",
    col_primary: "PA",
    col_secondary: "Sekundäradresse",
    col_id: "ID",
    col_version: "Version",
    sa_raw: "Sekundäradresse als Hex (16-stellig)",
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
    new_primary: "Neue Primäradresse",
    set: "Setzen",
    set_hint: "Zähler über Primäradresse, ID oder Sekundäradresse angeben. Stehen mehrere Zähler auf derselben Adresse (z.B. 0), die ID verwenden.",
    configure: "Zähler einstellen",
    meter: "Zähler",
    baud_title: "Baudrate",
    new_baud: "Neue Baudrate",
    switch: "Umstellen",
    detect_baud: "Baudrate suchen",
    baud_hint: "„Suchen“ probiert 300 bis 38400 Bd durch. Nach dem Umstellen antwortet der Zähler nur noch mit der neuen Baudrate; in der Zählerliste wird sie übernommen.",
    confirm_switch: (a, b) => `Zähler ${a} auf ${b} Bd umstellen?`,
    baud_switched: (b) => `Zähler läuft jetzt mit ${b} Bd`,
    baud_switched_list: (b) => `Zähler läuft jetzt mit ${b} Bd, in der Zählerliste übernommen`,
    detect_busy: "Suche …",
    detect_none: "Keine Antwort auf allen Baudraten",
    detect_found: (l) => `Antwortet mit: ${l} Bd`,
    reset_title: "Application Reset",
    reset_hint: "Setzt die Kommunikation des Zählers zurück (z.B. Auswahl der Telegramme). Zählerstände bleiben erhalten.",
    reset: "Reset senden",
    confirm_reset: (a) => `Application Reset an Zähler ${a} senden?`,
    reset_done: "Reset quittiert",
    meters: "Zählerliste",
    meters_hint: "Zähler merken, benennen und gesammelt auslesen. Die Liste wird auf dem Rechner gespeichert, auf dem mbusio läuft.",
    meters_file: (p) => `Datei: ${p}`,
    meters_empty: "Noch keine Zähler. In der Scan-Liste „Merken“ klicken oder „Zähler hinzufügen“.",
    meter_add: "Zähler hinzufügen",
    meters_read_all: "Alle auslesen",
    export: "Exportieren",
    import: "Importieren",
    meter_name: "Name",
    meter_name_ph: "z.B. Whg. 3 Warmwasser",
    meter_location: "Ort",
    meter_location_ph: "z.B. Keller links",
    meter_note: "Notiz",
    baud_bus: "wie Bus",
    save: "Speichern",
    edit: "Bearbeiten",
    remember: "Merken",
    remembered: "In Zählerliste übernommen",
    last_value: "Letzter Wert",
    meter_saved: "Gespeichert",
    confirm_delete_meter: (n) => `„${n}“ aus der Zählerliste entfernen?`,
    imported: (n) => `${n} Zähler importiert`,
    import_bad: "Datei enthält keine Zählerliste",
    job_read_all: "Alle auslesen",
    read_all_done: "Zählerliste ausgelesen",
    read_all_cancelled: "Auslesen der Zählerliste abgebrochen",
    raw_title: "Telegramm senden (Experten)",
    raw_hint: "Bytes werden unverändert gesendet, die Prüfsumme muss stimmen. Beispiel: 10 40 01 41 16 = SND_NKE an Adresse 1.",
    raw_bytes: "Bytes (Hex)",
    send: "Senden",
    raw_none: "Keine Antwort",
    raw_ack: "ACK (E5)",
    raw_collision: "Gestörte Antwort",
    raw_frame: "Telegramm",
    readings_export: "Messwerte & Export",
    readings_count: (n) => `Auslesung${n === 1 ? "" : "en"} gespeichert`,
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
    nav_read: "Read",
    nav_config: "Configure",
    nav_export: "Export",
    nav_log: "Log",
    connection: "Connection",
    port: "Port",
    reload_ports: "Reload ports",
    port_manual: "Or enter manually",
    baud: "Baud rate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatic",
    connect: "Connect",
    disconnect: "Disconnect",
    connection_hint:
      "8 data bits, even parity, 1 stop bit. Network gateways as tcp://IP:port (set the baud rate on the gateway). “SIMULATOR” starts a virtual bus for testing.",
    scan: "Scan",
    from: "From address",
    to: "To address",
    retries: "Retries",
    scan_primary: "Primary scan (PA)",
    scan_secondary: "Secondary scan (SA)",
    scan_primary_start: "Start primary scan",
    scan_secondary_start: "Start secondary scan",
    scan_primary_hint:
      "Polls every primary address in the range. Fast, but meters sharing an address (e.g. all on 0) only show up as a collision.",
    scan_secondary_hint:
      "Searches by meter ID, independent of the primary address. Also finds meters that share an address. Takes longer, no range needed.",
    cancel: "Cancel",
    scan_bauds: "Baud rates:",
    scan_bauds_hint:
      "“Check” polls all baud rates in a few seconds and selects those where devices answer (●). The scan only runs at the selected baud rates.",
    bauds_all: "All",
    busbar_view: "View",
    job_done: (k, n) => `${k} done · ${n} hits`,
    job_cancelled: (k, n) => `${k} cancelled · ${n} hits`,
    busy_read: "Reading meter …",
    busy_set: "Setting primary address …",
    busy_baud: "Switching baud rate …",
    busy_detect: "Detecting baud rate …",
    busy_reset: "Sending reset …",
    busy_raw: "Sending telegram …",
    bauds_none: "None",
    bus_bauds: "Check",
    pick_baud: "Select at least one baud rate for the scan",
    estimate: (t) => `Primary scan takes at most about ${t}`,
    bus_bauds_busy: "Checking 300 to 38400 Bd …",
    bus_bauds_step: (b, k, n, found) => `Checking ${b} Bd … (${k}/${n})` + (found.length ? ` · answer at ${found.join(", ")} Bd` : ""),
    bus_bauds_checking: "Checking …",
    theme_auto: "Theme like system",
    theme_light: "Light",
    theme_dark: "Dark",
    bus_bauds_none: "No device answers at any baud rate",
    bus_bauds_found: (l, b) => `Devices answer at: ${l} Bd` + (l.split(", ").some((x) => Number(x) !== b) ? " – note: not only at the configured baud rate!" : ""),
    duration: (s) => (s < 90 ? `${Math.ceil(s)} s` : `${Math.ceil(s / 60)} min`),
    col_baud: "Baud",
    col_primary: "PA",
    col_secondary: "Secondary address",
    col_id: "ID",
    col_version: "Version",
    sa_raw: "Secondary address as hex (16 digits)",
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
    new_primary: "New primary address",
    set: "Set",
    set_hint: "Give the meter by primary address, ID or secondary address. If several meters share an address (e.g. 0), use the ID.",
    configure: "Configure meter",
    meter: "Meter",
    baud_title: "Baud rate",
    new_baud: "New baud rate",
    switch: "Switch",
    detect_baud: "Detect baud rate",
    baud_hint: "“Detect” tries 300 to 38400 Bd. After switching, the meter only answers at the new baud rate; it is stored in the meter list.",
    confirm_switch: (a, b) => `Switch meter ${a} to ${b} Bd?`,
    baud_switched: (b) => `Meter now runs at ${b} Bd`,
    baud_switched_list: (b) => `Meter now runs at ${b} Bd, updated in the meter list`,
    detect_busy: "Searching …",
    detect_none: "No answer at any baud rate",
    detect_found: (l) => `Answers at: ${l} Bd`,
    reset_title: "Application reset",
    reset_hint: "Resets the meter's communication (e.g. telegram selection). Meter readings are kept.",
    reset: "Send reset",
    confirm_reset: (a) => `Send application reset to meter ${a}?`,
    reset_done: "Reset acknowledged",
    meters: "Meter list",
    meters_hint: "Remember meters, name them and read them all at once. The list is stored on the computer running mbusio.",
    meters_file: (p) => `File: ${p}`,
    meters_empty: "No meters yet. Click “Remember” in the scan list or “Add meter”.",
    meter_add: "Add meter",
    meters_read_all: "Read all",
    export: "Export",
    import: "Import",
    meter_name: "Name",
    meter_name_ph: "e.g. Flat 3 hot water",
    meter_location: "Location",
    meter_location_ph: "e.g. basement left",
    meter_note: "Note",
    baud_bus: "same as bus",
    save: "Save",
    edit: "Edit",
    remember: "Remember",
    remembered: "Added to meter list",
    last_value: "Last value",
    meter_saved: "Saved",
    confirm_delete_meter: (n) => `Remove “${n}” from the meter list?`,
    imported: (n) => `${n} meters imported`,
    import_bad: "File does not contain a meter list",
    job_read_all: "Read all",
    read_all_done: "Meter list read",
    read_all_cancelled: "Reading the meter list cancelled",
    raw_title: "Send telegram (experts)",
    raw_hint: "Bytes are sent unchanged, the checksum must be correct. Example: 10 40 01 41 16 = SND_NKE to address 1.",
    raw_bytes: "Bytes (hex)",
    send: "Send",
    raw_none: "No reply",
    raw_ack: "ACK (E5)",
    raw_collision: "Corrupted reply",
    raw_frame: "Telegram",
    readings_export: "Readings & export",
    readings_count: (n) => `reading${n === 1 ? "" : "s"} stored`,
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
  renderMeters();
  refreshStatus().catch(() => {});
}

document.querySelectorAll("[data-lang]").forEach((b) => (b.onclick = () => setLang(b.dataset.lang)));

// Design: auto (wie System), hell oder dunkel; das Setzen beim Laden steht in index.html
function setTheme(theme) {
  if (theme === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  storageSet("theme", theme);
  document.querySelectorAll("[data-theme-set]").forEach((b) => b.classList.toggle("active", b.dataset.themeSet === theme));
}
document.querySelectorAll("[data-theme-set]").forEach((b) => (b.onclick = () => setTheme(b.dataset.themeSet)));
setTheme(["light", "dark"].includes(storageGet("theme")) ? storageGet("theme") : "auto");

// ------------------------------------------------------------ Helfer

const $ = (id) => document.getElementById(id);
let connected = false;
let jobRunning = false;
let logSeq = 0;
let lastReading = null;
let lastJob = null;
let connBaud = null;
let checking = false;
let conn = null;
let baudsTouched = false;
const BAUDS = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];

/// Anfragen, die den Bus belegen: Pfad, Text, Bereich für "Ansehen"
const BUS_ACTIONS = [
  [/^read$|^meters\/\d+\/read$/, "busy_read", "#sec-read"],
  [/^set-address$/, "busy_set", "#sec-config"],
  [/^switch-baud$/, "busy_baud", "#sec-config"],
  [/^detect-baud$/, "busy_detect", "#sec-config"],
  [/^reset$/, "busy_reset", "#sec-config"],
  [/^raw$/, "busy_raw", "#sec-log"],
];

async function api(path, options = {}) {
  const busy = options.method === "POST" && BUS_ACTIONS.find(([re]) => re.test(path));
  if (!busy) return request(path, options);
  busbar({ text: t(busy[1]), link: busy[2] });
  try {
    return await request(path, options);
  } finally {
    busbar(null);
  }
}

async function request(path, options = {}) {
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
      return request(path, { ...options, retried: true });
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
  $("meters-export").href = "api/meters/export" + (token ? `?token=${encodeURIComponent(token)}` : "");
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
    // Knöpfe ohne Verbindungsbedarf wieder freigeben; die anderen regelt updateButtons
    button.disabled = false;
    updateButtons();
  }
}

function updateButtons() {
  document.querySelectorAll(".needs-conn").forEach((b) => (b.disabled = !connected || jobRunning || checking));
  // ohne markierte Baudrate gibt es nichts zu prüfen oder zu scannen
  const none = !$("scan-bauds").querySelector("input:checked");
  $("bauds-empty").hidden = !none;
  if (none) ["scan-primary", "scan-secondary"].forEach((id) => ($(id).disabled = true));
  const all = !$("scan-bauds").querySelector("input:not(:checked)");
  $("bauds-all").textContent = t(all ? "bauds_none" : "bauds_all");
  $("bauds-all").disabled = checking || jobRunning;
  $("connect").disabled = jobRunning;
  $("disconnect").disabled = !connected || jobRunning;
  $("connect").hidden = connected;
  $("disconnect").hidden = !connected;
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
  const prevBaud = connBaud;
  connBaud = s.connection?.baud ?? null;
  conn = s.connection;
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
let renderedJob = null;
let renderedHits = 0;

// ------------------------------------------------------------ Leiste unten

let busbarTimer = null;

/// `null` blendet aus; sonst { text, frac (0..1, ohne = unbestimmt), link, cancel, done, error }
function busbar(state) {
  const el = $("busbar");
  clearTimeout(busbarTimer);
  if (!state) {
    el.hidden = true;
    document.body.classList.remove("has-busbar");
    return;
  }
  el.hidden = false;
  document.body.classList.add("has-busbar");
  el.classList.toggle("done", !!state.done && !state.error);
  el.classList.toggle("error", !!state.error);
  $("busbar-text").textContent = state.text;
  const bar = el.querySelector(".busbar-progress");
  bar.hidden = state.frac === undefined;
  if (state.frac !== undefined) bar.querySelector(".bar-fill").style.transform = `scaleX(${state.frac})`;
  $("busbar-link").href = state.link || "#sec-scan";
  $("busbar-cancel").hidden = !state.cancel;
  // fertig: noch kurz stehen lassen
  if (state.done) busbarTimer = setTimeout(() => busbar(null), state.error ? 10000 : 6000);
}

let cancelRequested = false;

function cancelJob() {
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
        if (job.error) toast(job.error, true);
        else if (job.kind === "read_all") toast(t("read_all_done"));
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
  document.querySelector(".hits-opt").hidden = table.hidden;
  // Sekundäradresse lesbar zerlegt (ID, Version) oder roh als 16 Hex-Zeichen
  const raw = $("sa-raw").checked;
  const cols = raw
    ? ["col_primary", "col_secondary", "col_manufacturer", "col_medium", "col_status", ""]
    : ["col_primary", "col_id", "col_manufacturer", "col_version", "col_medium", "col_status", ""];
  cols.splice(1, 0, "col_baud");
  const numCols = ["col_primary", "col_baud", "col_version"];
  table.querySelector("thead").innerHTML = `<tr>${cols
    .map((c) => `<th${numCols.includes(c) ? ' class="num"' : ""}>${c && t(c)}</th>`)
    .join("")}</tr>`;
  table.querySelector("tbody").innerHTML = job.hits
    .map((h, i) => {
      const target = h.status === "ok" ? h.secondary_address || h.primary_address : "";
      const sa = raw
        ? `<td class="mono">${esc(h.secondary_address ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>`
        : `<td class="mono">${esc(h.id ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>
        <td class="num mono">${h.version ?? ""}</td>`;
      const other = h.baud && h.baud !== connBaud;
      const anim = i >= fresh ? ` class="new" style="animation-delay:${Math.min(i - fresh, 8) * 50}ms"` : "";
      return `<tr${anim}>
        <td class="num mono">${h.primary_address ?? "–"}</td>
        <td class="num mono${other ? " st-other" : ""}">${h.baud ?? ""}</td>
        ${sa}
        <td>${esc(label(h.medium))}</td>
        <td><span class="chip st-${h.status}">${t("st_" + h.status)}</span></td>
        <td><div class="actions">${
          target !== ""
            ? `<button class="small quiet needs-conn" data-read="${esc(target)}" data-baud="${h.baud ?? ""}">${t("read")}</button>
               <button class="small ghost" data-remember="${i}">${t("remember")}</button>`
            : ""
        }</div></td>
      </tr>`;
    })
    .join("");
  updateButtons();
}

$("sa-raw").checked = storageGet("sa_raw") === "1";
$("sa-raw").onchange = () => {
  storageSet("sa_raw", $("sa-raw").checked ? "1" : "0");
  if (lastJob) renderJob(lastJob);
};

$("hits").addEventListener("click", (ev) => {
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  if (d.remember !== undefined) {
    const h = lastJob.hits[Number(d.remember)];
    guarded(btn, async () => {
      await api("meters", {
        method: "POST",
        body: {
          address: h.secondary_address || String(h.primary_address),
          // eigene Baudrate nur merken, wenn sie vom Bus abweicht
          baud: h.baud && h.baud !== connBaud ? h.baud : null,
          meter_id: h.id,
          manufacturer: h.manufacturer || null,
          medium: h.medium,
        },
      });
      toast(t("remembered"));
      await loadMeters();
    });
    return;
  }
  if (!d.read) return;
  $("read-addr").value = d.read;
  $("set-old").value = d.read;
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
  // erster Datensatz gross, meist der Hauptzählerstand
  const first = r.records[0];
  const hero = first
    ? `<div class="hero">
        <span class="hero-label">${esc(label(first.quantity))} · ${esc(label(first.function))}</span>
        <div class="hero-value"><span class="mono">${esc(first.value_text)}</span><span class="unit">${esc(first.unit)}</span></div>
        <span class="hero-sub">${esc([h.manufacturer, h.medium && label(h.medium), r.timestamp].filter(Boolean).join(" · "))}</span>
      </div>`
    : "";
  const rows = r.records
    .map((x, i) => {
      const extra = [];
      if (x.storage) extra.push(`${t("storage")} ${x.storage}`);
      if (x.tariff) extra.push(`${t("tariff")} ${x.tariff}`);
      if (x.subunit) extra.push(`${t("subunit")} ${x.subunit}`);
      const notes = x.notes.length ? ` (${x.notes.map(label).join(", ")})` : "";
      return `<tr>
        <td class="num mono dim">${i + 1}</td>
        <td>${esc(label(x.quantity) + notes)}</td>
        <td class="num mono strong">${esc(x.value_text)}</td>
        <td>${esc(x.unit)}</td>
        <td class="dim">${esc(label(x.function))}${extra.length ? " · " + extra.join(", ") : ""}</td>
        <td class="mono dim">${esc(x.dif)} | ${esc(x.vif)}</td>
      </tr>`;
    })
    .join("");
  $("reading").innerHTML = `
    <div class="${hero ? "reading-top" : ""}">${hero}<div class="meta">${meta}</div></div>
    <div class="table-wrap"><table class="reading-table">
      <thead><tr><th>#</th><th>${t("th_desc")}</th><th class="num">${t("th_value")}</th><th>${t("th_unit")}</th><th>${t("th_kind")}</th><th>DIF | VIF</th></tr></thead>
      <tbody>${rows}</tbody>
    </table></div>`;
}

/// `baud`: Treffer aus dem Scan über alle Baudraten mit dessen Baudrate lesen
async function readMeter(baud = null) {
  await guarded($("read"), async () => {
    const address = $("read-addr").value.trim();
    if (!address) throw new Error(t("enter_address"));
    lastReading = null;
    $("reading").innerHTML = `<p class="loading"><span class="spin"></span>${t("reading_busy")}</p>`;
    try {
      renderReading(await api("read", { method: "POST", body: { address, baud } }));
      await refreshStatus();
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    }
  });
}

$("read").onclick = () => readMeter();
$("read-addr").addEventListener("keydown", (e) => e.key === "Enter" && readMeter());

// ------------------------------------------------------------ Zähler einstellen

function configTarget() {
  const address = $("set-old").value.trim();
  if (!address) throw new Error(t("enter_address"));
  return address;
}

/// Zähler der Liste mit derselben Adresse (gleiche Schreibweise)
function listedMeter(address) {
  return meters.find((m) => m.address.toUpperCase() === address.toUpperCase());
}

$("set-addr").onclick = () =>
  guarded($("set-addr"), async () => {
    const address = $("set-old").value.trim();
    const newAddr = $("set-new").value;
    if (!address || newAddr === "") throw new Error(t("enter_both"));
    if (!confirm(t("confirm_set", address, newAddr))) return;
    await api("set-address", { method: "POST", body: { address, new_address: Number(newAddr) } });
    toast(t("address_set", newAddr));
  });

$("switch-baud").onclick = () =>
  guarded($("switch-baud"), async () => {
    const address = configTarget();
    const baud = Number($("new-baud").value);
    if (!confirm(t("confirm_switch", address, baud))) return;
    await api("switch-baud", { method: "POST", body: { address, baud } });
    const m = listedMeter(address);
    if (m) {
      await api("meters/" + m.id, { method: "PUT", body: { ...m, baud } });
      await loadMeters();
      toast(t("baud_switched_list", baud));
    } else {
      toast(t("baud_switched", baud));
    }
  });

$("detect-baud").onclick = () =>
  guarded($("detect-baud"), async () => {
    const address = configTarget();
    const out = $("detect-result");
    out.textContent = t("detect_busy");
    try {
      const r = await api("detect-baud", { method: "POST", body: { address } });
      out.textContent = r.bauds.length ? t("detect_found", r.bauds.join(", ")) : t("detect_none");
      if (r.bauds.length) $("new-baud").value = r.bauds[0];
    } catch (e) {
      out.textContent = "";
      throw e;
    }
  });

$("app-reset").onclick = () =>
  guarded($("app-reset"), async () => {
    const address = configTarget();
    if (!confirm(t("confirm_reset", address))) return;
    await api("reset", { method: "POST", body: { address } });
    toast(t("reset_done"));
  });

// ------------------------------------------------------------ Zählerliste

let meters = [];
let editingId = null;

const svg = (d) =>
  `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON_EDIT = svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>');
const ICON_DEL = svg('<path d="M6 6l12 12M18 6L6 18"/>');
const ICON_ALERT = svg('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16.5v.01"/>');

async function loadMeters() {
  meters = await api("meters");
  renderMeters();
}

function renderMeters() {
  const table = $("meters-table");
  table.hidden = meters.length === 0;
  $("meters-empty").hidden = meters.length > 0;
  table.querySelector("tbody").innerHTML = meters
    .map((m) => {
      // ID nur zusätzlich zeigen, wenn die Adresse nicht schon die ID ist
      const info = [m.meter_id !== m.address ? m.meter_id : null, m.manufacturer].filter(Boolean).join(" · ");
      const sub = (text) => (text ? `<span class="sub">${esc(text)}</span>` : "");
      return `<tr>
        <td class="wrap"><span class="name">${esc(m.name || "–")}</span>${sub(m.note)}</td>
        <td class="wrap">${esc(m.location)}</td>
        <td class="mono">${esc(m.address)}${sub(info)}</td>
        <td>${esc(label(m.medium))}</td>
        <td class="num mono">${m.baud ?? ""}</td>
        <td class="wrap">${m.last_value ? `<span class="val">${esc(m.last_value)}</span>` : ""}${sub(
          [m.last_value ? label(m.last_quantity) : "", m.last_read].filter(Boolean).join(" · ")
        )}${m.last_error ? `<span class="err">${ICON_ALERT}${esc(m.last_error)}</span>` : ""}</td>
        <td><div class="actions">
          <button class="small quiet needs-conn" data-meter-read="${m.id}">${t("read")}</button>
          <button class="icon-btn" data-meter-edit="${m.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
          <button class="icon-btn del" data-meter-del="${m.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
        </div></td>
      </tr>`;
    })
    .join("");
  updateButtons();
}

function openMeterForm(m) {
  editingId = m ? m.id : null;
  $("mf-name").value = m?.name ?? "";
  $("mf-location").value = m?.location ?? "";
  $("mf-address").value = m?.address ?? "";
  $("mf-baud").value = m?.baud ?? "";
  $("mf-note").value = m?.note ?? "";
  $("meter-form").hidden = false;
  $("mf-name").focus();
}

$("meter-add").onclick = () => openMeterForm(null);
$("mf-cancel").onclick = () => ($("meter-form").hidden = true);

$("meter-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("meter-form").querySelector("[type=submit]"), async () => {
    const old = meters.find((m) => m.id === editingId) || {};
    const body = {
      ...old,
      name: $("mf-name").value.trim(),
      location: $("mf-location").value.trim(),
      address: $("mf-address").value.trim(),
      baud: $("mf-baud").value ? Number($("mf-baud").value) : null,
      note: $("mf-note").value.trim(),
    };
    if (editingId === null) await api("meters", { method: "POST", body });
    else await api("meters/" + editingId, { method: "PUT", body });
    $("meter-form").hidden = true;
    toast(t("meter_saved"));
    await loadMeters();
  });
};

$("meters-table").addEventListener("click", (ev) => {
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  const m = meters.find((x) => String(x.id) === (d.meterRead ?? d.meterEdit ?? d.meterDel));
  if (!m) return;
  if (d.meterEdit) return openMeterForm(m);
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

$("raw-send").onclick = () =>
  guarded($("raw-send"), async () => {
    const hex = $("raw-hex").value.trim();
    if (!hex) return;
    const r = await api("raw", { method: "POST", body: { hex } });
    const lines = [t("raw_" + r.kind) + (r.hex && r.kind !== "ack" ? ": " + r.hex : "")];
    if (r.decoded) {
      const h = r.decoded.header;
      if (h) lines.push(`ID ${h.id} · ${h.manufacturer} · ${label(h.medium)}`);
      for (const x of r.decoded.records) lines.push(`${label(x.quantity)}: ${x.value_text} ${x.unit}`.trim());
    }
    const out = $("raw-result");
    out.textContent = lines.join("\n");
    out.hidden = false;
  });
$("raw-hex").addEventListener("keydown", (e) => e.key === "Enter" && $("raw-send").click());

$("clear-log").onclick = async () => {
  await api("log", { method: "DELETE" }).catch(() => {});
  $("log").innerHTML = "";
};

// ------------------------------------------------------------ Start

setLang(lang);
loadPorts().catch((e) => toast(e.message, true));
loadMeters().catch((e) => toast(e.message, true));
setInterval(() => pollLog().catch(() => {}), 1000);
setInterval(() => !jobRunning && refreshStatus().catch(() => {}), 5000);
