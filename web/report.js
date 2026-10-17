"use strict";

// Bericht der Zählerliste zum Drucken bzw. "Als PDF speichern" im Druckdialog.

const TEXT = {
  de: {
    print: "Drucken / Als PDF speichern",
    with_details: "Alle Messwerte anhängen",
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

async function api(path) {
  const headers = { "X-Lang": lang };
  if (token) headers["X-Token"] = token;
  const res = await fetch("api/" + path, { headers });
  if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || res.statusText);
  return res.json();
}

// ------------------------------------------------------------ Kopffelder

const FIELDS = ["customer", "site", "technician", "order", "remarks"];

function loadFields() {
  let saved = {};
  try {
    saved = JSON.parse(get("report_fields") || "{}");
  } catch {
    /* kaputt gespeichert: leer anfangen */
  }
  for (const f of FIELDS) {
    const el = $("f-" + f);
    el.value = saved[f] || "";
    el.addEventListener("input", saveFields);
  }
  markBlank();
}

function saveFields() {
  put("report_fields", JSON.stringify(Object.fromEntries(FIELDS.map((f) => [f, $("f-" + f).value]))));
  markBlank();
}

/// Leere Felder erscheinen nicht im Ausdruck; Bemerkungen wachsen mit dem Text
function markBlank() {
  const r = $("f-remarks");
  r.style.height = "auto";
  r.style.height = r.scrollHeight + "px";
  for (const f of FIELDS) $("f-" + f).closest(".field").classList.toggle("blank", !$("f-" + f).value.trim());
}

// ------------------------------------------------------------ Inhalt

let data = null;

function status(m) {
  if (m.last_error) return `<span class="err">${t("st_err")}</span><span class="small">${esc(m.last_error)}</span>`;
  if (m.last_read) return `<span class="ok">${t("st_ok")}</span>`;
  return `<span class="none">${t("st_none")}</span>`;
}

function renderTable(meters) {
  if (!meters.length) return `<p class="empty">${t("empty")}</p>`;
  const rows = meters
    .map((m, i) => {
      // ID steht auf dem Zähler, deshalb vorne; volle Adresse klein darunter
      const main = m.meter_id || m.address;
      const info = [main !== m.address ? m.address : null, m.manufacturer].filter(Boolean).join(" · ");
      return `<tr>
        <td class="num">${i + 1}</td>
        <td><b>${esc(m.name || "–")}</b><span class="small">${esc([m.location, m.note].filter(Boolean).join(" · "))}</span></td>
        <td class="mono">${esc(main)}<span class="small">${esc(info)}</span></td>
        <td>${esc(label(m.medium))}</td>
        <td class="num mono">${m.primary_address ?? ""}</td>
        <td class="num mono">${m.baud ?? t("bus")}</td>
        <td class="num">${esc(m.last_value ?? "")}<span class="small">${esc(m.last_value ? label(m.last_quantity) : "")}</span></td>
        <td class="mono">${esc(m.last_read ?? "")}</td>
        <td>${status(m)}</td>
      </tr>`;
    })
    .join("");
  return `<table>
    <thead><tr>
      <th class="num">${t("th_no")}</th><th>${t("th_name")}</th><th>${t("th_id")}</th><th>${t("th_medium")}</th>
      <th class="num">${t("th_pa")}</th><th class="num">${t("th_baud")}</th><th class="num">${t("th_value")}</th>
      <th>${t("th_read")}</th><th>${t("th_status")}</th>
    </tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

/// Letzte gespeicherte Auslesung eines Zählers (über ID oder Adresse)
function latestReading(m, readings) {
  for (let i = readings.length - 1; i >= 0; i--) {
    const r = readings[i];
    if ((m.meter_id && r.header?.id === m.meter_id) || r.target === m.address) return r;
  }
  return null;
}

function renderDetails(meters, readings) {
  return (
    `<section class="details-section"><h2>${t("details")}</h2>` +
    meters
      .map((m, i) => {
        const r = latestReading(m, readings);
        const head = `<h3>${i + 1}. ${esc(m.name || m.meter_id || m.address)}${r ? ` <span class="small">${esc(r.timestamp)}</span>` : ""}</h3>`;
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

function render() {
  const { status: s, meters, readings } = data;
  const ok = meters.filter((m) => m.last_read && !m.last_error).length;
  const err = meters.filter((m) => m.last_error).length;
  const c = s.connection;
  const meta = [
    [t("date"), new Date().toLocaleString(lang === "de" ? "de-CH" : "en-GB")],
    c ? [t("connection"), `${c.port} · ${c.baud} Bd`] : null,
    [t("count"), `${meters.length} · ${ok} ${t("ok_count")}` + (err ? ` · ${err} ${t("err_count")}` : "")],
  ].filter(Boolean);
  $("meta").innerHTML = meta.map(([k, v]) => `<span>${esc(k)}: <b>${esc(v)}</b></span>`).join("");
  $("table").innerHTML = renderTable(meters);
  $("details-out").innerHTML = $("details").checked && meters.length ? renderDetails(meters, readings) : "";
  $("foot").textContent = t("foot", s.version);
}

async function main() {
  document.documentElement.lang = lang;
  document.querySelectorAll("[data-t]").forEach((el) => (el.textContent = t(el.dataset.t)));
  document.title = `mbusio – ${t("title")}`;
  loadFields();
  $("details").checked = get("report_details") === "1";
  $("details").onchange = () => {
    put("report_details", $("details").checked ? "1" : "0");
    render();
  };
  $("print").onclick = () => window.print();
  $("close").onclick = () => window.close();
  try {
    const [status, meters, readings, l] = await Promise.all([
      api("status"),
      api("meters"),
      api("readings"),
      api("labels?lang=" + lang),
    ]);
    labels = l;
    data = { status, meters, readings };
    render();
  } catch (e) {
    $("table").innerHTML = `<p class="err">${esc(e.message)}</p>`;
  }
}

main();
