// Sortierbare Spalten einer Liste (Scan-Treffer, Zähler): Klick auf den Kopf sortiert auf, ab,
// dann wieder in der ursprünglichen Reihenfolge. Leere Werte stehen immer unten. Die Wahl bleibt
// im Browser gespeichert.

/// `columns`: [[Name, Beschriftung(), Wert(Eintrag)]]; `of` holt den Eintrag aus einer Zeile
export function sorter(storageKey, columns, { title, of = (row) => row }) {
  let sort = read();

  function read() {
    try {
      const s = JSON.parse(storageGet(storageKey) || "null");
      return columns.some(([c]) => c === s?.col) ? s : null;
    } catch {
      return null; // ungültig gespeichert: ursprüngliche Reihenfolge
    }
  }

  /// Neue Liste; gleiche Werte behalten ihre Reihenfolge
  function sorted(rows) {
    const key = sort && columns.find(([c]) => c === sort.col)[2];
    if (!key) return rows;
    return rows
      .map((row, i) => ({ row, i, v: key(of(row)) }))
      .sort((a, b) => {
        const [x, y] = [a.v, b.v];
        if (x == null || y == null) return x == null && y == null ? a.i - b.i : x == null ? 1 : -1;
        const c = typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y), undefined, { numeric: true });
        return c * sort.dir || a.i - b.i;
      })
      .map((e) => e.row);
  }

  function toggle(col) {
    sort = sort?.col !== col ? { col, dir: 1 } : sort.dir > 0 ? { col, dir: -1 } : null;
    storageSet(storageKey, JSON.stringify(sort));
  }

  /// Köpfe als Knöpfe mit ▲/▼ an der sortierten Spalte
  function headHtml() {
    return columns
      .map(([c, name]) => {
        const on = sort?.col === c;
        return `<button type="button" class="sorter${on ? " on" : ""}" data-sort="${c}" title="${esc(title())}">${esc(name())}${on ? (sort.dir > 0 ? " ▲" : " ▼") : ""}</button>`;
      })
      .join("");
  }

  return { sorted, toggle, headHtml };
}
