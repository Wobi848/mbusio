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

let lang = storageGet("lang") || (navigator.language || "de").slice(0, 2);
if (!UI[lang]) lang = "de";
let labels = {}; // Messdaten-Begriffe vom Server
let token = storageGet("token") || "";
/// nach Eingabe eines neuen Zugriffsschlüssels (App: Download-Links nachführen)
let onToken = () => {};

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
      onToken();
      return request(path, { ...options, retried: true });
    }
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}
