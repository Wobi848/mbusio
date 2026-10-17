"use strict";

// Bericht der Zählerliste zum Drucken bzw. "Als PDF speichern" im Druckdialog.

const TEXT = {
  de: {
    print: "Drucken / Als PDF speichern",
    with_details: "Alle Messwerte anhängen",
    with_issues: "Auffälligkeiten anzeigen",
    changes: "Durchgeführte Änderungen",
    open_issues: "Offene Auffälligkeiten",
    project: "Projekt",
    lines: "Linien",
    th_line: "Gateway › Linie",
    clear_changes: "Änderungen leeren",
    confirm_clear_changes: "Liste der durchgeführten Änderungen leeren?",
    th_time: "Zeitpunkt",
    th_change: "Änderung",
    close: "Schliessen",
    title: "M-Bus Zählerprotokoll",
    subtitle: "Erstellt mit mbusio",
    customer: "Kunde",
    site: "Objekt / Anlage",
    technician: "Bearbeiter",
    order: "Auftrag",
    remarks: "Bemerkungen",
    meters: "Zähler",
    date: "Datum",
    connection: "Verbindung",
    plan: "Ausleseplan",
    plan_monthly: "monatlich am 1.,",
    plan_daily: "täglich",
    plan_hourly: "stündlich",
    plan_15min: "alle 15 min",
    time_unsure: "Zeit unsicher",
    count: "Zähler",
    ok_count: "gelesen",
    err_count: "mit Fehler",
    th_no: "Nr",
    th_name: "Name / Ort",
    th_address: "Adresse",
    th_medium: "Medium",
    th_pa: "PA",
    th_baud: "Baud",
    th_value: "Zählerstand",
    th_read: "Gelesen",
    th_status: "Status",
    st_ok: "OK",
    st_err: "Fehler",
    st_none: "nicht gelesen",
    bus: "Bus",
    details: "Messwerte",
    no_details: "Keine Auslesung in dieser Sitzung gespeichert.",
    th_desc: "Beschreibung",
    th_val: "Wert",
    th_id: "ID / Adresse",
    th_unit: "Einheit",
    th_kind: "Art",
    storage: "Speicher",
    tariff: "Tarif",
    empty: "Die Zählerliste ist leer. Zähler in mbusio mit „Merken“ übernehmen und auslesen.",
    sign_tech: "Datum, Unterschrift Bearbeiter",
    sign_customer: "Datum, Unterschrift Kunde",
    foot: (v) => `mbusio ${v} · Werte wie vom Zähler übertragen, ohne Gewähr für die Richtigkeit der Zählerdaten.`,
  },
  en: {
    print: "Print / save as PDF",
    with_details: "Attach all readings",
    with_issues: "Show issues",
    changes: "Changes made",
    open_issues: "Open issues",
    project: "Project",
    lines: "lines",
    th_line: "Gateway › line",
    clear_changes: "Clear changes",
    confirm_clear_changes: "Clear the list of changes made?",
    th_time: "Time",
    th_change: "Change",
    close: "Close",
    title: "M-Bus meter report",
    subtitle: "Created with mbusio",
    customer: "Customer",
    site: "Site / installation",
    technician: "Technician",
    order: "Order",
    remarks: "Remarks",
    meters: "Meters",
    date: "Date",
    connection: "Connection",
    plan: "Read plan",
    plan_monthly: "monthly on the 1st,",
    plan_daily: "daily",
    plan_hourly: "hourly",
    plan_15min: "every 15 min",
    time_unsure: "time unsure",
    count: "meters",
    ok_count: "read",
    err_count: "with errors",
    th_no: "No",
    th_name: "Name / location",
    th_address: "Address",
    th_medium: "Medium",
    th_pa: "PA",
    th_baud: "Baud",
    th_value: "Reading",
    th_read: "Read at",
    th_status: "Status",
    st_ok: "OK",
    st_err: "Error",
    st_none: "not read",
    bus: "bus",
    details: "Readings",
    no_details: "No reading stored in this session.",
    th_desc: "Description",
    th_val: "Value",
    th_id: "ID / address",
    th_unit: "Unit",
    th_kind: "Kind",
    storage: "Storage",
    tariff: "Tariff",
    empty: "The meter list is empty. Add meters in mbusio with “Remember” and read them.",
    sign_tech: "Date, signature technician",
    sign_customer: "Date, signature customer",
    foot: (v) => `mbusio ${v} · Values as transmitted by the meters, no warranty for the correctness of meter data.`,
  },
};

const $ = (id) => document.getElementById(id);

function get(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function put(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* privater Modus */
  }
}

let lang = get("lang") || (navigator.language || "de").slice(0, 2);
if (!TEXT[lang]) lang = "de";
const t = (k, ...a) => {
  const v = TEXT[lang][k] ?? k;
  return typeof v === "function" ? v(...a) : v;
};
const token = get("token") || "";
let labels = {};

function label(key) {
  if (!key) return "";
  const [base, arg] = key.split(":");
  const text = labels[base] ?? base;
  return arg ? `${text} ${arg}` : text;
}

function esc(s) {
  return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

async function api(path, options = {}) {
  const headers = { "X-Lang": lang, "Content-Type": "application/json" };
  if (token) headers["X-Token"] = token;
  const res = await fetch("api/" + path, { ...options, headers, body: options.body && JSON.stringify(options.body) });
  if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || res.statusText);
  return res.json();
}

// ------------------------------------------------------------ Kopffelder (gehören zum Projekt)

/// Feld im Bericht → Feld im Projekt
const FIELDS = { customer: "customer", site: "site", technician: "technician", order: "order_no", remarks: "remarks" };
let saveTimer = null;

function loadFields() {
  for (const [f, key] of Object.entries(FIELDS)) {
    const el = $("f-" + f);
    el.value = data.project[key] || "";
    el.addEventListener("input", () => {
      markBlank();
      // gesammelt speichern, nicht bei jedem Tastendruck
      clearTimeout(saveTimer);
      saveTimer = setTimeout(saveFields, 600);
    });
  }
  markBlank();
}

async function saveFields() {
  for (const [f, key] of Object.entries(FIELDS)) data.project[key] = $("f-" + f).value;
  try {
    await api(`projects/${data.project.id}`, { method: "PUT", body: data.project });
  } catch (e) {
    console.warn(e);
  }
}

/// Leere Felder erscheinen nicht im Ausdruck; Bemerkungen wachsen mit dem Text
function markBlank() {
  const r = $("f-remarks");
  r.style.height = "auto";
  r.style.height = r.scrollHeight + "px";
  for (const f of Object.keys(FIELDS)) $("f-" + f).closest(".field").classList.toggle("blank", !$("f-" + f).value.trim());
}

// ------------------------------------------------------------ Inhalt

let data = null;

function status(m) {
  if (m.last_error) return `<span class="err">${t("st_err")}</span><span class="small">${esc(m.last_error)}</span>`;
  if (m.last_read) return `<span class="ok">${t("st_ok")}</span>`;
  return `<span class="none">${t("st_none")}</span>`;
}

/// Alle Zähler des Projekts mit Gateway und Linie, fortlaufend nummeriert
function allMeters() {
  const out = [];
  for (const g of data.gateways)
    for (const l of g.lines) for (const e of l.meters) out.push({ ...e, gateway: g.gateway, line: l.line });
  return out;
}

/// "daily 00:05" → "täglich 00:05"
function planText(key) {
  const [kind, time] = key.split(" ");
  return `${t("plan_" + kind)}${time ? " " + time : ""}`;
}

/// Ausleseplan der Linie (eigener oder vom Projekt), nur wenn eingeschaltet
function planNote(l) {
  const key = l.line.plan || data.project.plan || "off";
  return key === "off" ? "" : ` <span class="small inline">· ${t("plan")}: ${esc(planText(key))}</span>`;
}

function renderTable() {
  const list = allMeters();
  if (!list.length) return `<p class="empty">${t("empty")}</p>`;
  let n = 0;
  return data.gateways
    .flatMap((g) => g.lines.map((l) => ({ g, l })))
    .filter(({ l }) => l.meters.length || (issuesShown() && l.issues?.count))
    .map(({ g, l }) => {
      const rows = l.meters
        .map(({ meter: m, reading: r }) => {
          n += 1;
          // ID steht auf dem Zähler, deshalb vorne; volle Adresse klein darunter
          const main = m.meter_id || m.address;
          const info = [main !== m.address ? m.address : null, m.manufacturer].filter(Boolean).join(" · ");
          return `<tr>
            <td class="num">${n}</td>
            <td><b>${esc(m.name || "–")}</b><span class="small">${esc([m.location, m.note].filter(Boolean).join(" · "))}</span></td>
            <td class="mono">${esc(main)}<span class="small">${esc(info)}</span></td>
            <td>${esc(label(m.medium))}</td>
            <td class="num mono">${m.primary_address ?? ""}</td>
            <td class="num mono">${m.baud ?? l.line.baud}</td>
            <td class="num">${esc(m.last_value ?? "")}<span class="small">${esc(m.last_value ? label(m.last_quantity) : "")}</span></td>
            <td class="mono">${esc(m.last_read ?? "")}${
              r?.time_unsure && r.timestamp === m.last_read ? `<span class="small">${t("time_unsure")}</span>` : ""
            }</td>
            <td>${status(m)}</td>
          </tr>`;
        })
        .join("");
      const head = `<h3 class="line-head">${esc(g.gateway.name)} › ${esc(l.line.name)}${
        l.line.connection ? ` <span class="small inline">${esc(l.line.connection)} · ${l.line.baud} Bd</span>` : ""
      }${planNote(l)}</h3>`;
      // Linie ohne Zähler, aber mit Auffälligkeiten: keine leere Tabelle
      if (!l.meters.length) return issuesShown() ? head + issuesBlock(l.issues) : "";
      return `${head}
      <table>
        <thead><tr>
          <th class="num">${t("th_no")}</th><th>${t("th_name")}</th><th>${t("th_id")}</th><th>${t("th_medium")}</th>
          <th class="num">${t("th_pa")}</th><th class="num">${t("th_baud")}</th><th class="num">${t("th_value")}</th>
          <th>${t("th_read")}</th><th>${t("th_status")}</th>
        </tr></thead>
        <tbody>${rows}</tbody>
      </table>${issuesBlock(l.issues)}`;
    })
    .join("");
}

/// Offene Auffälligkeiten der Linie aus dem letzten Scan
/// Auffälligkeiten im Bericht zeigen? (Häkchen in der Werkzeugleiste, Standard ja)
const issuesShown = () => $("show-issues").checked;

function issuesBlock(issues) {
  if (!issues?.count || !issuesShown()) return "";
  return `<div class="issues"><b>${t("open_issues")} (${issues.count})</b><ul>${issues.items
    .map((x) => `<li>${esc(x)}</li>`)
    .join("")}</ul></div>`;
}

function renderDetails() {
  return (
    `<section class="details-section"><h2>${t("details")}</h2>` +
    allMeters()
      .map(({ meter: m, reading: r, line }, i) => {
        const head = `<h3>${i + 1}. ${esc(m.name || m.meter_id || m.address)} <span class="small inline">${esc(line.name)}${
          r ? " · " + esc(r.timestamp) + (r.time_unsure ? ` (${t("time_unsure")})` : "") : ""
        }</span></h3>`;
        if (!r) return `<div class="detail">${head}<p class="empty">${t("no_details")}</p></div>`;
        const rows = r.records
          .map((x) => {
            const extra = [];
            if (x.storage) extra.push(`${t("storage")} ${x.storage}`);
            if (x.tariff) extra.push(`${t("tariff")} ${x.tariff}`);
            const notes = x.notes.length ? ` (${x.notes.map(label).join(", ")})` : "";
            return `<tr>
              <td>${esc(label(x.quantity) + notes)}</td>
              <td class="num mono">${esc(x.value_text)}</td>
              <td>${esc(x.unit)}</td>
              <td>${esc(label(x.function))}${extra.length ? " · " + extra.join(", ") : ""}</td>
            </tr>`;
          })
          .join("");
        return `<div class="detail">${head}<table>
          <thead><tr><th>${t("th_desc")}</th><th class="num">${t("th_val")}</th><th>${t("th_unit")}</th><th>${t("th_kind")}</th></tr></thead>
          <tbody>${rows}</tbody></table></div>`;
      })
      .join("") +
    "</section>"
  );
}

/// Änderungen am Bus (PA, Baudrate), vom Server je Linie protokolliert
function renderChanges() {
  const list = data.changes;
  $("clear-changes").hidden = list.length === 0;
  if (!list.length) return "";
  return `<h2>${t("changes")}</h2><table>
    <thead><tr><th>${t("th_time")}</th><th>${t("th_line")}</th><th>${t("th_change")}</th></tr></thead>
    <tbody>${list.map((c) => `<tr><td class="mono">${esc(c.time)}</td><td>${esc(c.line)}</td><td>${esc(c.text)}</td></tr>`).join("")}</tbody>
  </table>`;
}

function render() {
  const list = allMeters().map((e) => e.meter);
  const ok = list.filter((m) => m.last_read && !m.last_error).length;
  const err = list.filter((m) => m.last_error).length;
  const lines = data.gateways.reduce((n, g) => n + g.lines.length, 0);
  const open = data.gateways.reduce((n, g) => n + g.lines.reduce((k, l) => k + (l.issues?.count || 0), 0), 0);
  const meta = [
    [t("project"), data.project.name],
    [t("date"), new Date().toLocaleString(lang === "de" ? "de-CH" : "en-GB")],
    [t("lines"), `${data.gateways.length} Gateway · ${lines} ${t("lines")}`],
    [t("count"), `${list.length} · ${ok} ${t("ok_count")}` + (err ? ` · ${err} ${t("err_count")}` : "")],
    issuesShown() ? [t("open_issues"), String(open)] : null,
  ].filter(Boolean);
  $("meta").innerHTML = meta.map(([k, v]) => `<span>${esc(k)}: <b>${esc(v)}</b></span>`).join("");
  $("table").innerHTML = renderTable();
  $("changes").innerHTML = renderChanges();
  $("details-out").innerHTML = $("details").checked && list.length ? renderDetails() : "";
  $("foot").textContent = t("foot", data.version);
}

async function main() {
  document.documentElement.lang = lang;
  document.querySelectorAll("[data-t]").forEach((el) => (el.textContent = t(el.dataset.t)));
  document.title = `mbusio – ${t("title")}`;
  $("details").checked = get("report_details") === "1";
  $("show-issues").checked = get("report_issues") !== "0";
  $("show-issues").onchange = () => {
    put("report_issues", $("show-issues").checked ? "1" : "0");
    render();
  };
  $("details").onchange = () => {
    put("report_details", $("details").checked ? "1" : "0");
    render();
  };
  $("print").onclick = () => window.print();
  $("close").onclick = () => window.close();
  $("clear-changes").onclick = async () => {
    if (!confirm(t("confirm_clear_changes"))) return;
    await api(`projects/${data.project.id}/changes`, { method: "DELETE" });
    data.changes = [];
    render();
  };
  try {
    // ?project=ID für ein anderes als das aktive Projekt
    const q = new URLSearchParams(location.search).get("project");
    const [report, status, l] = await Promise.all([
      api("report" + (q ? `?project=${encodeURIComponent(q)}` : "")),
      api("status"),
      api("labels?lang=" + lang),
    ]);
    labels = l;
    data = { ...report, version: status.version };
    loadFields();
    render();
  } catch (e) {
    $("table").innerHTML = `<p class="err">${esc(e.message)}</p>`;
  }
}

main();
