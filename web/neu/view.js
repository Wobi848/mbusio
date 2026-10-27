// Bausteine der neuen Oberfläche: nur ändern, was sich geändert hat (Fokus, Hover und Klicks
// bleiben erhalten), Menüs, Rückfrage am Ort, Meldung mit „Rückgängig“.

import { onEscape } from "./keys.js";

/// Texte eines Bereichs (Deutsch, Englisch) zu den gemeinsamen aus js/texts.js dazu; jeder Bereich
/// hält seine Texte bei sich, damit Bereiche unabhängig gebaut werden können
export function texts(de, en) {
  Object.assign(UI.de, de);
  Object.assign(UI.en, en);
}

/// Zustand eines Ausgangs (`state` aus /api/status) als [Farbe, Wort]; gleich in Statusleiste und
/// Weitergabe
export function outputWord(st) {
  if (!st || st === "off") return ["", t("n_out_off")];
  if (st === "connected" || st === "listening") return ["ok", t("n_out_ok")];
  if (st === "connecting") return ["", t("n_out_wait")];
  return ["err", t("n_out_bad")];
}

/// Dauerbetrieb einer Linie ist möglich: Anschluss da und mindestens ein Zähler mit Plan
export const opPossible = (l) => !!(l.connection && l.plan_info?.meters);

/// Dauerbetrieb als Satz neben dem Schalter (Kopfzeile, Anlage): läuft, wartet, oder warum nicht
export function opText(l) {
  const p = l.plan_info;
  const plan = p?.meters ? (p.effective !== "off" ? planText(p.effective) : t("plan_meters_own", p.meters)) : "";
  if (l.operation) return `${t("op_running")} · ${plan}`;
  if (!l.connection) return t("op_need_conn");
  return plan ? t("op_plan_waiting", plan) : t("op_no_plan");
}

/// Eigene Stile eines Bereichs (neu/<bereich>.css) einmal laden
export function styles(area) {
  if (document.querySelector(`link[data-area="${area}"]`)) return;
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = `neu/${area}.css`;
  link.dataset.area = area;
  document.head.append(link);
}

/// Inhalt nur schreiben, wenn er anders ist als beim letzten Mal
export function html(el, markup) {
  if (el._html === markup) return;
  el._html = markup;
  el.innerHTML = markup;
}

export function text(el, value) {
  if (el.textContent !== value) el.textContent = value;
}

/// Liste mit Schlüssel: jede Zeile bleibt dasselbe Element, solange sich ihr Inhalt nicht ändert;
/// geänderte Zeilen werden einzeln ersetzt, neue eingefügt, verschwundene entfernt. So gehen
/// während eines Scans keine Klicks verloren und der Fokus bleibt (Audit Punkte 1, 2, 9).
/// `row(item)` liefert das Markup einer Zeile (ein Element), `key(item)` ihren Schlüssel.
export function list(container, items, key, row) {
  const old = new Map([...container.children].filter((c) => c.dataset.key).map((c) => [c.dataset.key, c]));
  let at = container.firstElementChild;
  for (const item of items) {
    const k = String(key(item));
    const markup = row(item);
    let el = old.get(k);
    // eine Zeile, in der gerade getippt wird, erst nach dem Verlassen neu bauen
    if (el && el._html !== markup && !el.contains(document.activeElement)) {
      const fresh = build(markup, k);
      el.replaceWith(fresh);
      if (at === el) at = fresh;
      el = fresh;
    } else if (!el) {
      el = build(markup, k);
    }
    old.delete(k);
    if (el !== at) container.insertBefore(el, at);
    else at = at.nextElementSibling;
  }
  for (const el of old.values()) el.remove();
}

function build(markup, key) {
  const t = document.createElement("template");
  t.innerHTML = markup.trim();
  const el = t.content.firstElementChild;
  el.dataset.key = key;
  el._html = markup;
  return el;
}

// ------------------------------------------------------------ Menüs

let openMenu = null;

/// Menü an einem Knopf: öffnet/schliesst, Escape und Klick daneben schliessen, Fokus geht an den
/// ersten Eintrag und beim Schliessen zurück an den Knopf. ↑↓ wechseln zwischen den Einträgen.
export function menu(button, panel, { onOpen } = {}) {
  button.addEventListener("click", (ev) => {
    ev.stopPropagation();
    if (openMenu?.panel === panel) return closeMenu();
    closeMenu();
    onOpen?.();
    panel.hidden = false;
    button.setAttribute("aria-expanded", "true");
    const release = onEscape(() => closeMenu(true));
    openMenu = { button, panel, release };
    (panel.querySelector("[autofocus], input, select, button:not(:disabled), a") || panel).focus?.();
  });
  panel.addEventListener("keydown", (ev) => {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    const items = [...panel.querySelectorAll("[role=menuitem]")];
    if (!items.length) return;
    ev.preventDefault();
    const i = items.indexOf(document.activeElement);
    items[(i + (ev.key === "ArrowDown" ? 1 : items.length - 1)) % items.length].focus();
  });
}

export function closeMenu(refocus = false) {
  if (!openMenu) return;
  const { button, panel, release } = openMenu;
  openMenu = null;
  panel.hidden = true;
  button.setAttribute("aria-expanded", "false");
  release();
  if (refocus) button.focus();
}

document.addEventListener("click", (ev) => {
  if (openMenu && !openMenu.panel.contains(ev.target)) closeMenu();
});

// ------------------------------------------------------------ Rückfrage am Ort

/// Rückfrage direkt am Knopf statt Browser-Fenster: Enter bestätigt, Escape bricht ab.
/// Nur für Gefährliches (Konzept §5.5). Liefert true bei „ja“.
export function ask(anchor, question, yes) {
  return new Promise((resolve) => {
    const box = document.createElement("div");
    box.className = "ask";
    box.setAttribute("role", "alertdialog");
    box.innerHTML = `<span>${esc(question)}</span><button type="button" class="danger">${esc(yes)}</button><button type="button" class="quiet">${esc(t("cancel"))}</button>`;
    const r = anchor.getBoundingClientRect();
    box.style.top = `${r.bottom + 6}px`;
    box.style.left = `${Math.max(8, Math.min(r.left, innerWidth - 360))}px`;
    document.body.append(box);
    const [ok, no] = box.querySelectorAll("button");
    const done = (v) => {
      release();
      box.remove();
      anchor.focus?.();
      resolve(v);
    };
    const release = onEscape(() => done(false));
    ok.onclick = () => done(true);
    no.onclick = () => done(false);
    ok.focus();
  });
}

// ------------------------------------------------------------ Meldung

let toastTimer = null;

/// Meldung unten; mit `undo` erscheint „Rückgängig“ (statt vorher zu fragen)
export function toast(message, { error = false, undo = null } = {}) {
  const el = document.getElementById("toast");
  clearTimeout(toastTimer);
  el.className = "toast" + (error ? " error" : "");
  el.innerHTML = `<span>${esc(message)}</span>` + (undo ? `<button type="button" class="quiet">${esc(t("n_undo"))}</button>` : "");
  el.hidden = false;
  if (undo)
    el.querySelector("button").onclick = () => {
      el.hidden = true;
      undo();
    };
  toastTimer = setTimeout(() => (el.hidden = true), error ? 10000 : undo ? 8000 : 4000);
}

// ------------------------------------------------------------ Vorschau → Übernehmen

/// Änderungssatz vom Server (Import, Namen, Adressen nach Muster): was neu ist, was sich ändert,
/// Hinweise, Fehler
export function changeSet(cs) {
  const items = (list, cls) => (list.length ? `<ul class="cs-${cls}">${list.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>` : "");
  const rows = [
    ...cs.added.map((a) => `<div class="vrow"><span><span class="chip ok">${esc(t("cs_new"))}</span> <span class="mono">${esc(a.item)}</span></span><span>${esc(a.detail)}</span><span></span></div>`),
    ...cs.changed.map((c) => `<div class="vrow"><span>${esc(c.field)} <span class="mono">${esc(c.item)}</span></span><span>${esc(c.old || "–")} → <b>${esc(c.new || "–")}</b></span><span></span></div>`),
  ];
  return `<p>${esc(t("cs_summary", cs.added.length, cs.changed.length, cs.unchanged))}</p>
    ${items(cs.errors, "err")}${cs.errors.length ? `<p class="sub">${esc(t("cs_errors_note"))}</p>` : ""}${items(cs.warnings, "warn")}
    ${!rows.length && !cs.errors.length ? `<p class="sub">${esc(t("cs_nothing"))}</p>` : ""}
    ${rows.length ? `<div class="vals">${rows.join("")}</div>` : ""}`;
}

/// Erst prüfen und zeigen, dann übernehmen, in einem Kasten (z.B. im Detailbereich).
/// `run(apply)` liefert den Änderungssatz; nach dem Übernehmen `done(cs)`.
export async function previewApply(box, { title, run, done }) {
  const show = (cs) => {
    const can = cs.added.length + cs.changed.length > 0 && !cs.errors.length;
    box.innerHTML = `<header class="dhead"><b>${esc(title)}</b></header>${changeSet(cs)}
      <div class="actions">${can ? `<button type="button" data-cs-apply>${esc(t("cs_apply"))}</button>` : ""}<button type="button" class="quiet" data-cs-close>${esc(t("cs_discard"))}</button></div>`;
    box._html = null;
    const release = onEscape(() => close());
    const close = () => {
      release();
      box.innerHTML = "";
      done?.(null);
    };
    box.querySelector("[data-cs-close]").onclick = close;
    box.querySelector("[data-cs-apply]")?.addEventListener("click", async (ev) => {
      ev.currentTarget.disabled = true;
      try {
        const r = await run(true);
        release();
        if (!r.applied) return show(r);
        box.innerHTML = "";
        done?.(r);
      } catch (e) {
        toast(e.message, { error: true });
        ev.currentTarget.disabled = false;
      }
    });
  };
  show(await run(false));
}
