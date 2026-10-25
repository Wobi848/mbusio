"use strict";

// Zählerliste vom Kunden (CSV oder aus Excel kopiert): Überschrift erkennen, Spalten zuordnen.
// Gemeinsam für die bisherige und die neue Oberfläche; zerlegt wird mit parseTable (base.js).

const listLogic = {
  FIELDS: ["id", "name", "location", "note", "pa"],
  GUESS: {
    id: /^(id|ident|z(ä|ae)hler.?(nr|nummer)|serien.?(nr|nummer)|fab(rikations)?.?(nr|nummer)|meter.?(id|no)|serial|nummer|nr\.?)$/i,
    name: /^(name|bezeichnung|benennung|description)$/i,
    location: /^(ort|lage|standort|einbauort|wohnung|whg|raum|location|place)$/i,
    note: /^(notiz|bemerkung|kommentar|note|remark|comment)$/i,
    pa: /^(pa|prim(ä|ae)r.?adr(esse)?|primary.?address|m-?bus.?adr(esse)?)$/i,
  },

  /// Zeilen → { rows, header (erste Zeile ist Überschrift), cols, map (Feld → Spalte, geraten) }
  guess(rows) {
    if (!rows.length) return null;
    const looksId = (v) => /^\D{0,4}\d{5,8}$/.test(v.replace(/\s/g, ""));
    const header = !rows[0].some(looksId) && rows.length > 1;
    const cols = Math.max(...rows.map((r) => r.length));
    const map = {};
    if (header) rows[0].forEach((h, i) => listLogic.FIELDS.forEach((f) => map[f] === undefined && listLogic.GUESS[f].test(h.trim()) && (map[f] = i)));
    // ohne Überschrift: Spalte mit den meisten ID-artigen Werten ist die ID
    if (map.id === undefined) {
      let best = -1;
      let score = 0;
      for (let c = 0; c < cols; c++) {
        const n = rows.slice(header ? 1 : 0).filter((r) => looksId(r[c] || "")).length;
        if (n > score) [best, score] = [c, n];
      }
      if (best >= 0) map.id = best;
    }
    return { rows, header, cols, map };
  },

  /// Zeilen für POST /api/meters/import-list: [{ id, name, location, note, pa }]
  rows(d) {
    return d.rows.slice(d.header ? 1 : 0).map((r) => Object.fromEntries(listLogic.FIELDS.map((f) => [f, d.map[f] !== undefined ? r[d.map[f]] || "" : ""])));
  },
};
