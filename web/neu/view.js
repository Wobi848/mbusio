// Bausteine der neuen Oberfläche: nur ändern, was sich geändert hat (Fokus, Hover und Klicks
// bleiben erhalten), Menüs, Rückfrage am Ort, Meldung mit „Rückgängig“.

import { onEscape } from "./keys.js";

/// Texte eines Bereichs (Deutsch, Englisch) zu den gemeinsamen aus js/texts.js dazu; jeder Bereich
/// hält seine Texte bei sich, damit Bereiche unabhängig gebaut werden können
export function texts(de, en) {
  Object.assign(UI.de, de);
  Object.assign(UI.en, en);
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
    if (el && el._html !== markup) {
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
