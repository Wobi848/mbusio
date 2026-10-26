// Bereich Werkzeuge: Darstellung einer Telegramm-Analyse und der Antwort auf gesendete Rohbytes,
// je als [Kopf, Inhalt]. Reine Bausteine; gesendet wird in werkzeuge.js.

/// Analyse vom Server: Zusammenfassung, Probleme, Felder Byte für Byte
export function analysisHtml(a) {
  return [
    `<b>${esc(a.summary)}</b>`,
    (a.problems.length ? a.problems.map((p) => `<p class="chip err wide">${esc(p)}</p>`).join("") : `<p class="chip ok wide">${esc(t("an_clean"))}</p>`) +
      `<table class="atable"><thead><tr><th>${esc(t("an_bytes"))}</th><th>${esc(t("an_field"))}</th><th>${esc(t("an_meaning"))}</th></tr></thead><tbody>${a.fields
        .map((f) => `<tr${f.bad ? ' class="bad"' : ""}><td class="mono">${esc(f.bytes || "–")}</td><td>${esc(f.name)}</td><td>${esc(f.value)}</td></tr>`)
        .join("")}</tbody></table>`,
  ];
}

/// Antwort auf Rohbytes: Art (keine, ACK, Kollision, Telegramm), Telegramm zum Anklicken, Werte
export function rawReplyHtml(r) {
  const lines = [];
  if (r.decoded?.header) lines.push(`ID ${r.decoded.header.id} · ${r.decoded.header.manufacturer} · ${label(r.decoded.header.medium)}`);
  for (const x of r.decoded?.records || []) lines.push(`${label(x.quantity)}: ${x.value_text} ${x.unit}`.trim());
  return [
    `<b>${esc(t("raw_" + r.kind))}</b>`,
    (r.hex && r.kind !== "ack" ? `<button type="button" class="frame mono" data-hex="${esc(r.hex)}" title="${esc(t("log_click"))}">${esc(r.hex)}</button>` : "") +
      lines.map((l) => `<div>${esc(l)}</div>`).join(""),
  ];
}
