// Bereich Anlage: Zustand der Anlage (Raster oben) und der Linien (Chips), dazu kleine Helfer für den
// Projektbaum. Reine Bausteine: bekommen, was sie zeigen, als Parameter.

import { outputWord, opPossible } from "./view.js";

const README = "https://github.com/Wobi848/mbusio";

/// Bytes lesbar (kB, MB, GB)
const size = (b) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${Math.round(b / 1e6)} MB` : `${Math.max(1, Math.round(b / 1e3))} kB`);

/// Wie lange der Platz beim heutigen Plan reicht, als Satzteil in passender Einheit
function lasts(days) {
  if (days >= 36500) return t("an_disk_century");
  if (days >= 730) return t("an_disk_lasts", t("an_disk_years", Math.floor(days / 365)));
  if (days >= 60) return t("an_disk_lasts", t("an_disk_months", Math.floor(days / 30)));
  return t("an_disk_lasts", t("an_disk_days", days));
}

/// Speicher: frei, wie lange er reicht (unter 90 Tagen als Hinweis); Zuwachs im Tooltip
function diskHtml(d, chip) {
  const pct = Math.round(d.free_pct);
  if (d.level !== "ok") return chip("warn", t(d.level === "full" ? "disk_full" : "disk_low", pct));
  const title = d.per_day ? t("an_disk_title", size(d.per_day), size(d.db_bytes)) : "";
  const free = esc(t("ps_disk_free", pct));
  if (d.days_left == null) return free;
  return d.days_left < 90 ? `${free} ${chip("warn", lasts(d.days_left), title)}` : `<span title="${esc(title)}">${free} · ${esc(lasts(d.days_left))}</span>`;
}

export const allLines = (p) => p.gateways.flatMap((g) => g.lines);
export const sum = (lines, f) => lines.reduce((n, l) => n + (f(l) || 0), 0);

export const planned = (l) => !!l.plan_info?.meters;

/// Schalter Dauerbetrieb: ohne Anschluss oder Plan gesperrt, der Grund steht daneben (opText)
export const opSwitch = (l, big = false) =>
  `<input type="checkbox" class="switch${big ? " big" : ""}" data-op="${l.id}"${l.operation ? " checked" : ""}${
    opPossible(l) ? "" : " disabled"
  } aria-label="${esc(`${t("op_chip")} ${l.name}`)}">`;


/// Zustand der Linie: Probleme als Wort und Farbe zuerst, sonst letzter bzw. nächster Termin
/// Letzte oder nächste Auslesung der Linie als kurzer Text (leer, wenn es keine gibt)
export function lineInfo(l) {
  const p = l.plan_info || {};
  const r = p.result;
  return r?.time && !r.offline && !r.line_down
    ? t("plan_last", r.ok, r.total)
    : l.operation && p.next && !l.held
      ? t("plan_next", slotText(p.next))
      : l.last_read
        ? t("an_read_at", slotText(l.last_read))
        : "";
}

export function lineState(l) {
  const p = l.plan_info || {};
  const r = p.result;
  const chips = [];
  if (l.held) chips.push(["", t("plan_held"), t("plan_held_title")]);
  else if (l.service && !l.service.connected && l.service.error) chips.push(["warn", t("plan_offline"), l.service.error]);
  if (r?.line_down) chips.push(["warn", t("an_line_down"), r.error || t("plan_line_down")]);
  if (p.disturbed) chips.push(["warn", t("plan_disturbed", p.disturbed)]);
  if (l.datapoints?.invalid) chips.push(["warn", t("ln_dp_invalid", l.datapoints.invalid), t("ln_dp_invalid_title")]);
  if (l.issues?.count) chips.push(["warn", t("ln_issues", l.issues.count), l.issues.items.join("\n")]);
  if (p.paused) chips.push(["", t("plan_paused", p.paused)]);
  if (p.load_warn) chips.push(["warn", t("plan_load", Math.round(p.load * 100))]);
  const info = lineInfo(l);
  return chips.map(([cls, txt, title]) => `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(txt)}</span>`).join(" ") + (info ? ` <span class="sub">${esc(info)}</span>` : "");
}

/// Zustand der Anlage als Raster (leer, wenn keine Linie im Dauerbetrieb ist); jeder Teil führt zur
/// Stelle, an der man es behebt
export function plantHtml(p, s) {
  const lines = allLines(p).filter((l) => l.operation);
  if (!lines.length || !s) return "";
  const cell = (title, body) => `<div class="cell"><span class="eyebrow">${esc(title)}</span><span class="body">${body}</span></div>`;
  const chip = (cls, txt, title = "") => `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(txt)}</span>`;
  const cells = [];
  if (!s.background) {
    const win = s.os === "windows";
    cells.push(
      cell(
        t("op_chip"),
        `${chip("warn", t("ps_fg"), t(win ? "ps_fg_win" : "ps_fg_linux"))} <a href="${README}#${win ? "windows-als-dienst" : "raspberry-pi-als-dienst"}" target="_blank" rel="noopener">${esc(
          t("an_svc_how")
        )}</a>`
      )
    );
  }
  const planLines = lines.filter(planned);
  const next = planLines.map((l) => l.plan_info.next).filter(Boolean).sort()[0];
  const disturbed = sum(planLines, (l) => l.plan_info.disturbed);
  const paused = sum(planLines, (l) => l.plan_info.paused);
  const down = planLines.filter((l) => l.plan_info.result?.line_down).length;
  cells.push(
    cell(
      t("plan"),
      `${esc(t("ps_lines", planLines.length))}${next ? ` · ${esc(t("plan_next", slotText(next)))}` : ""}` +
        (disturbed ? " " + chip("warn", t("plan_disturbed", disturbed)) : "") +
        (paused ? " " + chip("", t("plan_paused", paused)) : "") +
        (down ? " " + chip("warn", t("ps_lines_down", down)) : "")
    )
  );
  if (s.time)
    cells.push(
      cell(
        t("ps_time"),
        s.time.confirmed
          ? esc(t("ps_time_ok", s.time.source))
          : `${chip("warn", t("time_unsure"))} <button type="button" class="tonal" data-take-time>${esc(t("ps_time_take"))}</button>`
      )
    );
  // Speicherschutz ist eine Einstellung des Geräts: im Menü Einstellungen
  if (s.disk) cells.push(cell(t("ps_disk"), `${diskHtml(s.disk, chip)} <button type="button" class="quiet" data-open-settings>${esc(t("st_protect"))} …</button>`));
  for (const o of (s.outputs || []).filter((o) => o.on)) {
    const [cls, word] = outputWord(s[o.key]?.state);
    cells.push(cell(o.label, `${chip(cls, word, s[o.key]?.error || "")} <a href="#/weitergabe">${esc(t("ps_open_transfer"))}</a>`));
  }
  return cells.join("");
}
