"use strict";

// Basis für App und Bericht: Speicher, Sprache, Texte, API, Hilfen

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

const $ = (id) => document.getElementById(id);

/// Baudraten des M-Bus
const BAUDS = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];

/// Sprachen im Menü
const LANGUAGES = { de: "Deutsch", en: "English" };
/// Übersetzt, aber noch nicht geprüft: nicht im Menü, nur über ?lang=fr zu sehen (für die Prüfer)
const LANG_PREVIEW = { fr: "Français", it: "Italiano", es: "Español" };

/// Formate je Sprache (Gegenstück zu `Lang::date_format` und `Lang::decimal` im Server)
const LANG_FORMATS = {
  de: { locale: "de-CH", dateSep: ".", decimal: "," },
  en: { locale: "en-GB", dateSep: "/", decimal: ".", isoDay: true },
  fr: { locale: "fr-CH", dateSep: ".", decimal: "," },
  it: { locale: "it-CH", dateSep: ".", decimal: "," },
  es: { locale: "es-ES", dateSep: "/", decimal: "," },
};

const urlLang = new URLSearchParams(location.search).get("lang");
if (urlLang) storageSet("lang", urlLang);
let lang = storageGet("lang") || (navigator.language || "de").slice(0, 2);
if (!LANGUAGES[lang] && !LANG_PREVIEW[lang]) lang = "en";
const langFormat = () => LANG_FORMATS[lang];

/// Sprachdatei laden, falls die Sprache nicht in texts.js steht; vor dem ersten Zeichnen abwarten.
/// Schlägt das fehl, erscheint Englisch.
function loadLanguage() {
  if (UI[lang]) return Promise.resolve();
  return new Promise((resolve) => {
    const script = document.createElement("script");
    script.src = `js/lang-${lang}.js`;
    script.onload = script.onerror = () => resolve();
    document.head.append(script);
  });
}
let labels = {}; // Messdaten-Begriffe vom Server
let token = storageGet("token") || "";

/// Anschluss lesbar: fester Linux-Name /dev/serial/by-id/usb-Silicon_Labs_CP2102_…-if00-port0
/// wird zu „Silicon Labs CP2102 …“; alles andere bleibt, wie es ist
function portText(c) {
  const m = /^\/dev\/serial\/by-id\/(?:usb-)?(.+?)(?:-if\d+)?(?:-port\d+)?$/.exec(c || "");
  return m ? m[1].replace(/_/g, " ") : c || "";
}

/// Minuten lesbar: 63 → „1 h 3 min“, 12 → „12 min“
function durText(min) {
  const h = Math.floor(min / 60);
  return h ? `${h} h${min % 60 ? ` ${min % 60} min` : ""}` : `${min} min`;
}

function t(key, ...args) {
  const v = UI[lang]?.[key] ?? UI.en[key] ?? UI.de[key] ?? key;
  return typeof v === "function" ? v(...args) : v;
}

/// Schlüssel vom Server übersetzen, z.B. "note_error_code:05"
function label(key) {
  if (!key) return "";
  const [base, arg] = key.split(":");
  const text = labels[base] ?? base;
  return arg ? `${text} ${arg}` : text;
}

function esc(s) {
  return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

/// Zeit vom Server ("2026-10-18 00:05:00", Ortszeit) als Date
const parseTime = (s) => new Date(String(s).replace(" ", "T"));

/// "2026-10-18 00:05:00" → "heute 00:05", "morgen 00:05" oder "20.10. 00:05"
function slotText(s) {
  if (!s) return "";
  const d = parseTime(s);
  const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const days = Math.round((day(d) - day(new Date())) / 86400000);
  const hm = s.slice(11, 16);
  if (days === 0) return `${t("day_today")} ${hm}`;
  if (days === 1) return `${t("day_tomorrow")} ${hm}`;
  return `${d.getDate()}.${d.getMonth() + 1}. ${hm}`;
}

/// Bytes lesbar (kB, MB, GB)
function sizeText(b) {
  return b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${Math.round(b / 1e6)} MB` : `${Math.max(1, Math.round(b / 1e3))} kB`;
}

/// Zeitpunkt für den Bericht: "2026-10-18 00:05:00" oder Date → "18.10.2026 00:05" (englisch 18/10/2026)
function dateTime(s) {
  const d = s instanceof Date ? s : parseTime(s);
  if (!s || isNaN(d)) return String(s || "");
  const p = (n) => String(n).padStart(2, "0");
  const day = [p(d.getDate()), p(d.getMonth() + 1), d.getFullYear()].join(langFormat().dateSep);
  return `${day} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/// "daily 00:05" → "täglich 00:05"
function planText(key) {
  if (!key) return "";
  const [kind, time] = key.split(" ");
  return kind === "daily" || kind === "monthly" ? t("plan_" + kind, time) : t("plan_" + kind);
}

/// Link auf die API für Downloads (Links schicken keine Kopfzeilen): Sprache und Zugriffsschlüssel
/// als Parameter
function apiHref(path) {
  return `api/${path}${path.includes("?") ? "&" : "?"}lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
}

async function request(path, options = {}) {
  // eine Datei (Blob) geht unverändert, alles andere als JSON
  const file = options.body instanceof Blob;
  const headers = { "Content-Type": file ? "application/octet-stream" : "application/json", "X-Lang": lang, ...options.headers };
  if (token) headers["X-Token"] = token;
  const opts = { ...options, headers };
  if (opts.body && !file && typeof opts.body !== "string") opts.body = JSON.stringify(opts.body);
  const res = await fetch("api/" + path, opts);
  if (res.status === 401) {
    await askToken(headers["X-Token"]);
    return request(path, options);
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = new Error(data.error || res.statusText);
    if (data.needs_password) err.needsPassword = true;
    throw err;
  }
  return data;
}

let tokenAsked = null; // eine Abfrage für alle gleichzeitig abgewiesenen Anfragen

/// Fragt den Zugriffsschlüssel in einem Feld über der Seite ab (kein Browser-Fenster);
/// `rejected` = der eben abgewiesene Schlüssel, dann steht „falsch“ dabei
function askToken(rejected) {
  if (token && token !== rejected) return Promise.resolve();
  tokenAsked ||= new Promise((resolve) => {
    const gate = document.createElement("form");
    gate.className = "token-gate";
    gate.style.cssText = "position:fixed;inset:0;z-index:1000;display:grid;place-content:center;gap:10px;padding:16px;" +
      "background:var(--bg,#f5f6f5);color:var(--text,#121614);font:14px system-ui,sans-serif";
    gate.innerHTML = `<label for="token-input" style="font-weight:650">${esc(t("token_prompt"))}</label>
      <input id="token-input" type="password" autocomplete="current-password" required
        style="font:inherit;padding:8px 10px;border:1px solid var(--line-strong,#cfd4d1);border-radius:6px;min-width:min(320px,90vw)">
      ${rejected ? `<span style="color:var(--err,#b42318)">${esc(t("token_wrong"))}</span>` : ""}
      <button style="font:inherit;font-weight:600;padding:8px 14px;border:0;border-radius:6px;background:var(--accent,#0f766e);color:#fff;cursor:pointer">${esc(t("token_go"))}</button>`;
    gate.addEventListener("submit", (e) => {
      e.preventDefault();
      token = gate.querySelector("input").value.trim();
      storageSet("token", token);
      gate.remove();
      tokenAsked = null;
      resolve();
    });
    document.body.append(gate);
    gate.querySelector("input").focus();
  });
  return tokenAsked;
}

/// Uhrzeit immer 24 h als HH:MM; Kurzformen: "5" → 05:00, "530" → 05:30, "17:5" → 17:05.
/// null, wenn es keine gültige Uhrzeit ist.
function time24(s) {
  const v = String(s || "").trim();
  let h;
  let m;
  const parts = v.split(/[:.h ]/).filter(Boolean);
  if (parts.length === 2) [h, m] = parts;
  else if (/^\d{1,2}$/.test(v)) [h, m] = [v, "0"];
  else if (/^\d{3,4}$/.test(v)) [h, m] = [v.slice(0, -2), v.slice(-2)];
  else return null;
  if (!/^\d{1,2}$/.test(h) || !/^\d{1,2}$/.test(m) || Number(h) > 23 || Number(m) > 59) return null;
  return `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;
}

/// Tabelle aus CSV oder aus Excel kopiert (Tabulator); Trennzeichen wird erkannt
function parseTable(text) {
  text = text.replace(/^\uFEFF/, "");
  const first = text.split(/\r?\n/)[0] || "";
  const delim = ["\t", ";", ","].map((d) => [d, first.split(d).length]).sort((a, b) => b[1] - a[1])[0][0];
  const rows = [];
  let row = [];
  let cur = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          cur += '"';
          i++;
        } else quoted = false;
      } else cur += c;
    } else if (c === '"' && cur === "") quoted = true;
    else if (c === delim) {
      row.push(cur);
      cur = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cur);
      rows.push(row);
      row = [];
      cur = "";
    } else cur += c;
  }
  if (cur || row.length) {
    row.push(cur);
    rows.push(row);
  }
  return rows.map((r) => r.map((x) => x.trim())).filter((r) => r.some(Boolean));
}
