"use strict";

// Auswertung der Scan-Treffer, gemeinsam für die bisherige und die neue Oberfläche. Ohne
// Seitenbezug: alles, was von der Linie abhängt, kommt als Kontext
// ctx = { hits (alle Treffer des Scans), busBaud (Baudrate des Busses), mode ("primary" | "secondary": Merken über PA/SA) }

const scanLogic = {
  /// Ursache eines Treffers ohne Daten in einfachen Worten: [was ist los, was tun]
  explain(reason) {
    const [key, arg] = (reason || "").split(":");
    switch (key) {
      case "wrong_address":
        return t("why_wrong_address", arg);
      case "app_error":
        return t("why_app_error", arg, t("app_errors")[arg] || "?");
      case "collision":
      case "checksum":
      case "stop":
      case "garbled":
      case "truncated":
      case "no_reply":
      case "unexpected":
      case "decode":
      case "same_id":
        return t("why_" + key);
      default:
        return t("why_unknown");
    }
  },

  /// Auffälligkeiten: doppelte Primäradressen (je Baudrate), doppelte IDs, fremde Baudraten
  analyze(ctx) {
    const { hits, busBaud } = ctx;
    const group = (key) => {
      const m = new Map();
      hits.forEach((h, i) => {
        const k = key(h);
        if (k !== null) m.set(k, [...(m.get(k) || []), i]);
      });
      return new Map([...m].filter(([, list]) => list.length > 1));
    };
    // Kollisionen aus dem Primärscan sind schon eine Zeile für sich, die zählen nicht doppelt
    const dupPa = group((h) => (h.primary_address == null || h.status === "collision" ? null : `${h.baud}:${h.primary_address}`));
    const dupId = group((h) => (h.id && /^[0-9]{8}$/.test(h.id) ? h.id : null));
    const rowDupPa = new Set([...dupPa.values()].flat());
    // gleiche ID ist nur ein Problem, wenn auch der Hersteller gleich ist (IDs sind je Hersteller eindeutig)
    const dupIdSame = new Map([...dupId].filter(([, list]) => new Set(list.map((i) => hits[i].manufacturer)).size < list.length));
    const rowDupId = new Set([...dupId.values()].flat());
    const rowDupIdSame = new Set([...dupIdSame.values()].flat());
    const otherBaud = hits.filter((h) => h.baud && h.baud !== busBaud).length;
    return { dupPa, dupId, dupIdSame, rowDupPa, rowDupId, rowDupIdSame, otherBaud };
  },

  /// Auffälligkeiten als Sätze (bei der Linie gespeichert: Projektbaum, Bericht)
  issues(ctx, a = scanLogic.analyze(ctx)) {
    const { hits, busBaud } = ctx;
    const items = [];
    for (const [k, list] of a.dupPa) {
      const [baud, pa] = k.split(":");
      items.push(t("warn_dup_pa", pa, baud, list.map((i) => hits[i].id || "?")));
    }
    for (const [id, list] of a.dupIdSame) items.push(t("warn_dup_id", id, list.length));
    for (const h of hits.filter((x) => x.status === "collision")) {
      if (h.primary_address != null) items.push(t("warn_collision_pa", h.primary_address, h.baud));
      else if (h.id) items.push(t("warn_collision_id", h.id));
    }
    // Zähler ohne Daten einzeln, mit Ursache (auch für den Bericht verständlich)
    for (const h of hits.filter((x) => x.status === "no_data")) items.push(`${h.id || `PA ${h.primary_address}`}: ${scanLogic.explain(h.reason)[0]}`);
    if (a.otherBaud) items.push(t("warn_other_baud", a.otherBaud, busBaud));
    return items;
  },

  /// Adresse zum Ansprechen eines Treffers: volle Sekundäradresse, ausser sie ist mehrdeutig
  /// (gleiche ID und gleicher Hersteller), dann die Primäradresse
  address(h, ctx) {
    const same = ctx.hits.filter((x) => x.secondary_address && x.secondary_address === h.secondary_address);
    if (h.secondary_address && (same.length < 2 || h.primary_address == null)) return h.secondary_address;
    return String(h.primary_address);
  },

  /// Adresse zum Merken nach der Einstellung der Linie; { reason }, wenn sie nicht eindeutig ist
  rememberAddress(h, ctx) {
    const { hits, mode } = ctx;
    if (mode === "primary") {
      const pa = h.primary_address;
      if (pa == null) return { reason: t("skip_pa_unknown") };
      if (pa === 0) return { reason: t("skip_pa_zero") };
      if (hits.filter((x) => x.status === "ok" && x.baud === h.baud && x.primary_address === pa).length > 1) return { reason: t("skip_pa_dup", pa) };
      return { address: String(pa) };
    }
    // Platzhalter (F) z.B. bei alten Zählern ohne Herstellerkennung: geht, solange eindeutig
    const sa = h.secondary_address;
    if (!sa || /^F{8}/.test(sa)) return { reason: t("skip_sa_unknown") };
    if (hits.filter((x) => x.secondary_address === sa).length > 1) return { reason: t("skip_sa_dup") };
    return { address: sa };
  },

  /// Treffer, die „Übernehmen“ in die Zählerliste nimmt: lesbar und über die gewählte Adresse
  /// (SA oder PA) eindeutig ansprechbar
  rememberable(hits, ctx) {
    return hits.filter((h) => h.status === "ok" && scanLogic.rememberAddress(h, ctx).address);
  },

  /// Eintrag für die Zählerliste aus einem Treffer
  meterFromHit(h, ctx) {
    return {
      address: scanLogic.rememberAddress(h, ctx).address,
      secondary_address: h.secondary_address || null,
      primary_address: h.primary_address,
      // eigene Baudrate nur merken, wenn sie vom Bus abweicht
      baud: h.baud && h.baud !== ctx.busBaud ? h.baud : null,
      meter_id: h.id,
      manufacturer: h.manufacturer || null,
      medium: h.medium,
    };
  },

  /// Was sich automatisch beheben lässt (Schritte) und was von Hand (Texte)
  planSteps(ctx, a = scanLogic.analyze(ctx)) {
    const { hits, busBaud } = ctx;
    const steps = [];
    const manual = [];
    const name = (h) => h.id || h.secondary_address || `PA ${h.primary_address}`;
    // eindeutig ansprechbar nur über die volle Sekundäradresse, sofern sie nicht mehrfach vorkommt
    const target = (h) => (h.secondary_address && hits.filter((x) => x.secondary_address === h.secondary_address).length === 1 ? h.secondary_address : null);
    for (const h of hits) {
      if (h.status === "ok" && h.baud && h.baud !== busBaud && target(h)) steps.push({ kind: "baud", target: target(h), name: name(h), from: h.baud, to: busBaud });
    }
    for (const [k, list] of a.dupPa) {
      const pa = Number(k.split(":")[1]);
      const members = list.map((i) => hits[i]).sort((x, y) => String(x.id).localeCompare(String(y.id)));
      // PA 0 ist der Werkszustand: alle umziehen; sonst darf der erste bleiben
      for (const h of pa === 0 ? members : members.slice(1)) {
        if (!target(h)) continue;
        steps.push({ kind: "pa", target: target(h), name: name(h), from: pa, to: null, fromBaud: h.baud !== busBaud ? h.baud : null });
      }
    }
    for (const h of hits) {
      if (h.status === "collision" && h.primary_address != null) manual.push(t("plan_manual_collision", h.primary_address, h.baud));
      // Sekundärscan: gleiche ID, über die ID allein nicht trennbar
      if (h.status === "collision" && h.primary_address == null && h.id) manual.push(t("plan_manual_id_collision", h.id));
      if (h.status === "no_data") manual.push(t("plan_manual_nodata", name(h)));
    }
    for (const id of a.dupIdSame.keys()) manual.push(t("plan_manual_sameid", id));
    return { steps, manual };
  },

  /// Steht der Treffer schon in der Zählerliste? Über Sekundäradresse, ID mit Hersteller oder
  /// (Merken über PA) die Primäradresse, dann aber nur, wenn dort nicht ein anderer Zähler (andere
  /// ID, z.B. nach einem Zählertausch) steht
  listed(h, meters, ctx) {
    return meters.some(
      (m) =>
        (h.secondary_address && m.secondary_address === h.secondary_address) ||
        (h.id && m.meter_id === h.id && (!m.manufacturer || m.manufacturer === h.manufacturer)) ||
        (ctx.mode === "primary" && h.primary_address != null && m.address === String(h.primary_address) && (!m.meter_id || !h.id || m.meter_id === h.id))
    );
  },

  /// Höchstdauer eines Primärscans in Sekunden (jede Adresse antwortet nicht: volles Timeout)
  duration({ from, to, retries, bauds, conn }) {
    const n = Math.max(0, to - from + 1);
    return bauds.reduce((sum, b) => sum + n * (retries + 1) * (conn.timeout_auto ? 330 / b + 0.25 : conn.timeout_ms / 1000), 0);
  },

  /// Gleicher Zähler: ID und Hersteller; ohne ID (Kollision, keine Daten) über Baudrate und PA
  identity: (h) => (h.id ? `${h.id}:${h.manufacturer}` : `pa:${h.baud}:${h.primary_address}`),

  /// Unterschiede zum letzten vollständigen Scan gleicher Art und gleichen Bereichs (job.previous):
  /// rows: Index → { kind: "new" | "changed", text }, missing: Treffer, die diesmal fehlen
  changes(job) {
    const prev = job.previous;
    if (job.running || !prev) return null;
    const pool = new Map();
    for (const h of prev.hits) pool.set(scanLogic.identity(h), [...(pool.get(scanLogic.identity(h)) || []), h]);
    const rows = new Map();
    job.hits.forEach((h, i) => {
      const old = pool.get(scanLogic.identity(h))?.shift();
      if (!old) return rows.set(i, { kind: "new", text: t("chg_new") });
      const diff = [
        old.primary_address !== h.primary_address ? `PA ${old.primary_address ?? "–"} → ${h.primary_address ?? "–"}` : "",
        old.baud !== h.baud ? `${old.baud} → ${h.baud} Bd` : "",
        old.status !== h.status ? `${t("st_" + old.status)} → ${t("st_" + h.status)}` : "",
      ].filter(Boolean);
      if (diff.length) rows.set(i, { kind: "changed", text: t("chg_changed", diff.join(", ")) });
    });
    // eine frühere Kollision oder „keine Daten“ auf einer PA, die jetzt antwortet, fehlt nicht: sie ist aufgelöst
    const nowPa = new Set(job.hits.map((h) => `${h.baud}:${h.primary_address}`));
    const missing = [...pool.values()].flat().filter((h) => h.status === "ok" || !nowPa.has(`${h.baud}:${h.primary_address}`));
    return { rows, missing, time: prev.time };
  },
};
