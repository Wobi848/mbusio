// Neue Oberfläche: Start, Texte, Einstellungen, Bereiche (Laschen, Arbeitsfläche, Tasten); Kopfzeile
// in app-kopf.js, Statusleiste in app-statusleiste.js (Konzept neue Oberfläche).
// Gemeinsam mit dem Bericht: js/texts.js (t, UI) und js/base.js (request, esc, …).

import { state, set, watch, activeLine, AREA_URL } from "./store.js";
import { text, menu, closeMenu, toast, ask, askValue } from "./view.js";
import { bind, keyList, onEscape } from "./keys.js";
import { pollStatus, loadTree, reload, report } from "./data.js";
import { startKopf } from "./app-kopf.js";
import { startStatusleiste } from "./app-statusleiste.js";
import { startMeldungen } from "./app-meldungen.js";

const $ = (id) => document.getElementById(id);
const AREAS = ["anlage", "scan", "zaehler", "weitergabe", "werkzeuge", "simulator"];
const AREA_TAB = { scan: "tab_scan", zaehler: "tab_meters", weitergabe: "tab_transfer", anlage: "tab_project", werkzeuge: "tab_tools", simulator: "tab_sim" };
const STATUS_EVERY = 5000;
const TREE_EVERY = 15000;

// ------------------------------------------------------------ Texte

/// Feste Texte im Gerüst (data-t, data-title) in der gewählten Sprache
function applyTexts() {
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll("[data-t]")) el.textContent = t(el.dataset.t);
  for (const el of document.querySelectorAll("[data-title]")) {
    el.title = t(el.dataset.title);
    el.setAttribute("aria-label", el.title);
  }
}

// ------------------------------------------------------------ Einstellungen

menu($("settings"), $("settings-menu"), {
  onOpen: () => {
    $("lang").value = lang;
    $("theme").value = themeChoice;
    loadStorage();
  },
});

/// Speicherschutz (Gerät): beim Öffnen laden, jede Änderung gleich speichern
async function loadStorage() {
  const box = $("storage");
  try {
    const s = await request("storage");
    box.elements.protect.checked = s.protect;
    box.elements.first_days.value = s.first_days;
    box.elements.second_days.value = s.second_days;
  } catch (e) {
    report(e);
  }
}
$("storage").addEventListener("change", async () => {
  const f = $("storage").elements;
  try {
    await request("storage", { method: "PUT", body: { protect: f.protect.checked, first_days: Number(f.first_days.value), second_days: Number(f.second_days.value) } });
    toast(t("st_saved"));
  } catch (e) {
    report(e);
    loadStorage();
  }
});
// Sicherung des Geräts (Konzept Sicherung): herunterladen; einspielen erst nach Rückfrage mit dem
// Inhalt, dann startet mbusio neu und die Seite verbindet sich von selbst wieder
$("backup-get").href = apiHref("backup");
$("backup-pick").addEventListener("click", () => $("backup-file").click());
$("backup-file").addEventListener("change", async (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  try {
    let password = "";
    let summary;
    for (;;) {
      try {
        summary = await request("restore", { method: "POST", body: file, headers: password ? { "X-Backup-Password": encodeURIComponent(password) } : {} });
        break;
      } catch (e) {
        if (!e.needsPassword) throw e;
        // beim zweiten Mal steht dabei, warum (Passwort falsch)
        password = await askValue($("backup-pick"), password ? e.message : t("bk_password"), t("bk_unlock"), "password");
        if (password == null) return;
      }
    }
    if (!(await ask($("backup-pick"), t("bk_q", summary), t("bk_apply")))) return await request("restore", { method: "DELETE" });
    await request("restore/apply", { method: "POST" });
    closeMenu();
    toast(t("bk_restarting"));
    // neu laden, sobald das neu gestartete mbusio antwortet: alles kommt aus der Sicherung
    await new Promise((r) => setTimeout(r, 2000));
    for (let i = 0; i < 60; i++) {
      if (await request("status").then(() => true, () => false)) return location.reload();
      await new Promise((r) => setTimeout(r, 1000));
    }
  } catch (e) {
    report(e);
  }
});
$("lang").addEventListener("change", (ev) => {
  lang = ev.target.value;
  storageSet("lang", lang);
  location.reload(); // Sprache betrifft auch Texte vom Server; einmaliger Wechsel, kein Arbeitsablauf
});
$("theme").addEventListener("change", (ev) => {
  themeChoice = ev.target.value;
  storageSet("theme", themeChoice);
  applyTheme();
});
// Erklärungen zu Bereichen und Werkzeugen (.explain); aus, bis man sie einschaltet
$("help-toggle").addEventListener("change", (ev) => {
  document.body.classList.toggle("show-help", ev.target.checked);
  storageSet("help", ev.target.checked ? "1" : "0");
});
$("help-toggle").checked = storageGet("help") === "1";
document.body.classList.toggle("show-help", $("help-toggle").checked);
$("keys-help").addEventListener("click", () => {
  closeMenu();
  showKeys();
});

/// Übersicht der Tasten, die gerade wirken (?); Escape oder Klick daneben schliesst
function showKeys() {
  document.querySelector(".keys-help")?.remove();
  const box = document.createElement("div");
  box.className = "keys-help";
  box.setAttribute("role", "dialog");
  box.setAttribute("aria-label", t("n_keys"));
  const fixed = [
    ["↑ ↓", t("n_key_rows")],
    [t("n_key_space"), t("n_key_select")],
    ["Ctrl+A", t("n_key_all")],
    ["Enter", t("n_key_enter")],
    ["Esc", t("n_key_esc")],
  ];
  box.innerHTML = `<b>${esc(t("n_keys"))}</b><dl>${[...keyList().map((k) => [k.key.toUpperCase(), k.label]), ...fixed]
    .map(([k, l]) => `<dt><kbd>${esc(k)}</kbd></dt><dd>${esc(l)}</dd>`)
    .join("")}</dl>`;
  document.body.append(box);
  const close = () => {
    release();
    box.remove();
    document.removeEventListener("click", outside, true);
  };
  const outside = (ev) => !box.contains(ev.target) && close();
  const release = onEscape(close);
  setTimeout(() => document.addEventListener("click", outside, true), 0);
}

// ------------------------------------------------------------ Bereiche

/// Frühere deutsche Adressen: Lesezeichen führen weiter an die richtige Stelle
const OLD_URLS = [[/^#\/anlage\b/, "#/site"], [/^#\/zaehler\b/, "#/meters"], [/^#\/weitergabe\b/, "#/transfer"], [/^#\/werkzeuge\b/, "#/tools"], [/\/abrechnung\b/, "/billing"], [/\bprojekt=/, "project="], [/\bziel=/, "target="]];

/// Adresse ↔ Bereich: #/meters, #/scan …; Zurück-Taste und Lesezeichen funktionieren. Läuft vor den
/// Bereichen, die Teile der Adresse selbst lesen.
function readHash() {
  const hash = OLD_URLS.reduce((h, [old, now]) => h.replace(old, now), location.hash);
  if (hash !== location.hash) history.replaceState(null, "", hash);
  const url = hash.replace(/^#\/?/, "").split(/[/?]/)[0];
  set({ area: AREAS.find((a) => AREA_URL[a] === url) || "zaehler" });
}
addEventListener("hashchange", readHash);

function renderTabs() {
  for (const a of document.querySelectorAll(".tabs [data-area]")) {
    a.title = `${t(AREA_TAB[a.dataset.area])} (${AREAS.indexOf(a.dataset.area) + 1})`;
    if (a.dataset.area !== state.area) a.removeAttribute("aria-current");
    else if (!a.hasAttribute("aria-current")) {
      a.setAttribute("aria-current", "page");
      // schmal laufen die Reiter seitlich: den gewählten zeigen
      a.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }
  const l = activeLine();
  text($("n-meters"), l?.meters ? String(l.meters) : "");
  text($("n-dp"), l?.datapoints?.count ? String(l.datapoints.count) : "");
  $("tab-sim").hidden = !(l?.connection || "").startsWith("SIMULATOR") && state.area !== "simulator";
}

/// Arbeitsfläche: jeder Bereich hat eine eigene Fläche, die beim Wechsel bleibt (Scroll, Auswahl,
/// Filter). Ein Bereich ist die Datei app/<bereich>.js mit `mount(fläche)`, geladen beim ersten
/// Öffnen.
const panes = {};
function renderWork() {
  for (const a of AREAS) {
    if (!panes[a] && a === state.area) {
      panes[a] = document.createElement("section");
      panes[a].className = "pane";
      panes[a].dataset.area = a;
      $("work").append(panes[a]);
      // Laden fehlgeschlagen (z.B. WLAN weg): Meldung statt leerer Fläche; beim nächsten Öffnen neu
      import(`./${a}.js`)
        .then((m) => m.mount(panes[a]))
        .catch((e) => {
          panes[a].remove();
          delete panes[a];
          report(e);
        });
    }
    if (panes[a]) panes[a].hidden = a !== state.area;
  }
}

AREAS.forEach((a, i) =>
  bind(String(i + 1), () => (location.hash = `#/${AREA_URL[a]}`), t("n_key_area", t(AREA_TAB[a])))
);
// das erste sichtbare Suchfeld (im Protokoll der Weitergabe ist die Liste darunter versteckt)
bind("/", () => [...document.querySelectorAll("#work .pane:not([hidden]) [data-search]")].find((el) => el.offsetParent)?.focus(), t("n_key_search"));
bind("?", showKeys, t("n_key_help"));

// ------------------------------------------------------------ Start

async function start() {
  applyTexts();
  labels = await request("labels").catch(() => ({}));
  startKopf();
  watch(["tree", "status", "area"], renderTabs);
  watch(["area"], renderWork);
  startStatusleiste();
  startMeldungen();
  const fromAddress = location.hash.length > 2;
  readHash();
  await reload().catch(report);
  // ohne Adresse dort anfangen, wo es weitergeht: noch keine Zähler → Scan, sonst Zähler (§3.4)
  if (!fromAddress) location.hash = activeLine()?.meters ? "#/meters" : "#/scan";
  setInterval(pollStatus, STATUS_EVERY);
  setInterval(() => loadTree().catch(() => {}), TREE_EVERY);
}

start();
