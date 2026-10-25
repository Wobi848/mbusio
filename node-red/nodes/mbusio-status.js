"use strict";

// Viele Werte kommen kurz nacheinander (Stand nach dem Verbinden, Durchgang einer Linie): erst
// danach eine Nachricht
const SETTLE_MS = 500;

module.exports = function (RED) {
  function MbusioStatusNode(config) {
    RED.nodes.createNode(this, config);
    const node = this;
    const server = RED.nodes.getNode(config.server);
    const project = Number(config.project);
    // leer = ganzes Projekt
    const line = config.line ? Number(config.line) : null;
    const t = (key, args) => RED._(`mbusio-status.status.${key}`, args);

    if (!server || !project) {
      node.status({ fill: "red", shape: "ring", text: t("config") });
      return;
    }

    const conn = server.use(project);
    let lastSent = null;
    let timer = null;

    /** Zustand des Projekts bzw. der Linie: erreichbar, Anzahl je Qualität, Datenpunkte mit Problem */
    function summary() {
      const s = { online: conn.state.state === "connected", good: 0, stale: 0, disturbed: 0, invalid: 0, problems: [] };
      for (const v of conn.values.values()) {
        const info = conn.points.get(v.no);
        if (!info || (line && info.lineId !== line)) continue;
        s[v.quality] = (s[v.quality] || 0) + 1;
        if (v.quality !== "good")
          s.problems.push({ no: v.no, name: info.name, meter: info.meter, line: info.line, quality: v.quality, time: v.time });
      }
      return s;
    }

    function update() {
      timer = null;
      const { state, detail } = conn.state;
      // beim (Neu-)Verbinden nichts melden: erst, wenn feststeht, ob mbusio erreichbar ist
      if (state === "connecting") return node.status({ fill: "grey", shape: "ring", text: t("connecting") });
      const s = summary();
      const ok = s.online && !s.disturbed && !s.invalid;
      if (!s.online) {
        const fill = state === "denied" || state === "version" || state === "noproject" ? "red" : "grey";
        node.status({ fill, shape: "ring", text: RED._(`node-red-contrib-mbusio/mbusio-in:mbusio-in.status.${state}`, { detail: detail ? `(${detail})` : "" }).trim() });
      } else if (!ok) node.status({ fill: "red", shape: "dot", text: t("disturbed", { n: s.disturbed + s.invalid }) });
      else if (s.stale) node.status({ fill: "yellow", shape: "dot", text: t("stale", { n: s.stale }) });
      else node.status({ fill: "green", shape: "dot", text: t("ok", { n: s.good }) });
      const key = JSON.stringify(s);
      if (key === lastSent) return;
      lastSent = key;
      node.send({ payload: ok, mbusio: s });
    }

    const schedule = () => {
      if (!timer) timer = setTimeout(update, SETTLE_MS);
    };
    conn.on("state", schedule);
    conn.on("tree", schedule);
    conn.on("value", schedule);
    schedule();

    node.on("close", () => {
      clearTimeout(timer);
      conn.off("state", schedule);
      conn.off("tree", schedule);
      conn.off("value", schedule);
      server.release(project);
    });
  }

  RED.nodes.registerType("mbusio-status", MbusioStatusNode);
};
