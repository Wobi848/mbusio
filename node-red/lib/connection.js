"use strict";

// Verbindung zu mbusio für ein Projekt (Konzept Node-RED): Baum für Namen und Version, laufende
// Werte als Server-Sent Events, Neuversuch nach Abbruch. Alle mbusio-in-Nodes eines Servers und
// Projekts teilen sich eine Verbindung.

const EventEmitter = require("events");

/** Aufbau der mbusio-Antworten, den diese Version versteht */
const API_VERSION = 1;
const RETRY_MS = 10000;
const TIMEOUT_MS = 10000;

/** Zerlegt Server-Sent Events aus Textstücken; liefert je Stück die fertigen Ereignisse */
function sseParser() {
  let buf = "";
  return (chunk) => {
    buf += chunk.replace(/\r\n/g, "\n");
    const events = [];
    let end;
    while ((end = buf.indexOf("\n\n")) >= 0) {
      const block = buf.slice(0, end);
      buf = buf.slice(end + 2);
      let name = "message";
      const data = [];
      for (const line of block.split("\n")) {
        if (line.startsWith(":")) continue; // Kommentar, z.B. ping
        const i = line.indexOf(":");
        const key = i < 0 ? line : line.slice(0, i);
        const value = i < 0 ? "" : line.slice(i + 1).replace(/^ /, "");
        if (key === "event") name = value;
        else if (key === "data") data.push(value);
      }
      if (data.length) events.push({ name, data: data.join("\n") });
    }
    return events;
  };
}

/** Datenpunkte eines Projekts aus dem Baum: Nummer → Ort und Namen; null, wenn es das Projekt nicht gibt */
function indexTree(tree, projectId) {
  const p = (tree.projects || []).find((x) => x.id === projectId);
  if (!p) return null;
  const points = new Map();
  for (const g of p.gateways)
    for (const l of g.lines)
      for (const m of l.meters)
        for (const d of m.datapoints)
          points.set(d.no, {
            no: d.no,
            name: d.name,
            unit: d.unit,
            meter: m.name,
            meterId: m.id,
            ident: m.ident,
            pa: m.pa,
            gateway: g.name,
            line: l.name,
            lineId: l.id,
            project: p.name,
          });
  return { project: p.name, points };
}

/** Fehler mit Zustand für den Node-Status */
function failure(state, message) {
  return Object.assign(new Error(message || state), { state });
}

/**
 * Ereignisse: "state" ({ state, detail }) mit state = connecting | connected | offline | denied |
 * noproject | version; "tree" (Map Nummer → Ort); "value" (Wert, Ort)
 */
class Connection extends EventEmitter {
  constructor({ url, token, project, retryMs = RETRY_MS }) {
    super();
    this.url = String(url || "").replace(/\/+$/, "");
    this.token = token || "";
    this.project = project;
    this.retryMs = retryMs;
    this.points = new Map();
    // letzter Wert je Nummer: für Nodes, die später dazukommen, und gegen Doppelte nach Neuverbindung
    this.values = new Map();
    this.state = { state: "connecting" };
    this.abort = null;
    this.stopped = false;
  }

  start() {
    this.run();
    return this;
  }

  /** Beenden; weckt auch eine laufende Wartezeit vor dem Neuversuch */
  stop() {
    this.stopped = true;
    if (this.abort) this.abort.abort();
    this.emit("stopped");
  }

  setState(state, detail) {
    this.state = { state, detail };
    this.emit("state", this.state);
  }

  async get(path, signal) {
    const headers = this.token ? { "x-token": this.token } : {};
    let res;
    try {
      res = await fetch(this.url + path, { headers, signal });
    } catch (e) {
      throw failure("offline", e.cause?.code || e.message);
    }
    if (res.status === 401) throw failure("denied");
    if (res.status === 404) throw failure("noproject");
    if (!res.ok) throw failure("offline", `HTTP ${res.status}`);
    return res;
  }

  async loadTree() {
    const res = await this.get("/api/nodered/tree", AbortSignal.timeout(TIMEOUT_MS));
    const tree = await res.json();
    if (tree.version !== API_VERSION) throw failure("version", String(tree.version));
    const idx = indexTree(tree, this.project);
    if (!idx) throw failure("noproject");
    this.points = idx.points;
    this.emit("tree", this.points);
  }

  /** Strom lesen, bis er endet; "changed" lädt den Baum neu, bevor weitere Werte kommen */
  async readStream() {
    this.abort = new AbortController();
    const res = await this.get(`/api/nodered/stream?project=${this.project}`, this.abort.signal);
    this.setState("connected");
    const parse = sseParser();
    const decoder = new TextDecoder();
    for await (const chunk of res.body) {
      for (const e of parse(decoder.decode(chunk, { stream: true }))) {
        if (e.name === "changed") await this.loadTree();
        else if (e.name === "value") this.onValue(JSON.parse(e.data));
      }
    }
  }

  onValue(v) {
    const old = this.values.get(v.no);
    if (old && old.time === v.time && old.value === v.value && old.quality === v.quality) return;
    this.values.set(v.no, v);
    const info = this.points.get(v.no);
    // neu und noch nicht im Baum: kommt nach dem nächsten "changed" mit Namen
    if (info) this.emit("value", v, info);
  }

  async run() {
    while (!this.stopped) {
      try {
        this.setState("connecting");
        await this.loadTree();
        await this.readStream();
        if (!this.stopped) this.setState("offline", "closed");
      } catch (e) {
        if (this.stopped) break;
        this.setState(e.state || "offline", e.state ? e.message : e.cause?.code || e.message);
      }
      await new Promise((resolve) => {
        const done = () => {
          clearTimeout(t);
          this.off("stopped", done);
          resolve();
        };
        const t = setTimeout(done, this.retryMs);
        this.once("stopped", done);
      });
    }
  }
}

module.exports = { Connection, sseParser, indexTree, API_VERSION };
