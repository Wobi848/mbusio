"use strict";

const QUALITY_FILL = { good: "green", stale: "yellow", disturbed: "red", invalid: "red" };

module.exports = function (RED) {
  function MbusioInNode(config) {
    RED.nodes.createNode(this, config);
    const node = this;
    const server = RED.nodes.getNode(config.server);
    const project = Number(config.project);
    // datapoints = gewählte Nummern; meter / line = alle (auch später dazukommende)
    const scope = config.scope || "datapoints";
    const numbers = (config.datapoints || []).map(Number);
    const meter = Number(config.meter);
    const line = Number(config.line);
    const onlyGood = config.quality !== "all";
    const t = (key, ...args) => RED._(`mbusio-in.status.${key}`, ...args);

    if (!server || !project) {
      node.status({ fill: "red", shape: "ring", text: t("config") });
      return;
    }

    const wanted = (info) =>
      scope === "meter" ? info.meterId === meter : scope === "line" ? info.lineId === line : numbers.includes(info.no);
    // letzter Wert je gewähltem Datenpunkt, für den Status
    const latest = new Map();
    let missing = [];

    function showStatus() {
      const values = [...latest.values()];
      const bad = values.filter((v) => v.quality === "disturbed" || v.quality === "invalid").length;
      const stale = values.filter((v) => v.quality === "stale").length;
      if (missing.length) return node.status({ fill: "yellow", shape: "ring", text: t("missing", { no: missing.join(", ") }) });
      if (bad) return node.status({ fill: "red", shape: "dot", text: t("disturbed", { n: bad }) });
      if (stale) return node.status({ fill: "yellow", shape: "dot", text: t("stale", { n: stale }) });
      if (values.length === 1) {
        const v = values[0];
        return node.status({ fill: QUALITY_FILL[v.quality] || "grey", shape: "dot", text: `${v.value} ${v.unit}`.trim() });
      }
      node.status({ fill: "green", shape: "dot", text: t("values", { n: values.length }) });
    }

    const onState = ({ state, detail }) => {
      if (state === "connected") return showStatus();
      const fill = state === "denied" || state === "version" || state === "noproject" ? "red" : "grey";
      node.status({ fill, shape: "ring", text: t(state, { detail: detail ? `(${detail})` : "" }).trim() });
    };

    const onTree = (points) => {
      missing = scope === "datapoints" ? numbers.filter((n) => !points.has(n)) : [];
      for (const no of latest.keys()) if (!points.has(no) || !wanted(points.get(no))) latest.delete(no);
      showStatus();
    };

    const onValue = (v, info) => {
      if (!wanted(info)) return;
      latest.set(v.no, v);
      showStatus();
      if (onlyGood && v.quality !== "good") return;
      node.send({
        payload: v.value,
        topic: info.name,
        mbusio: {
          no: v.no,
          unit: v.unit,
          time: v.time,
          quality: v.quality,
          meter: info.meter,
          ident: info.ident,
          pa: info.pa,
          gateway: info.gateway,
          line: info.line,
          project: info.project,
        },
      });
    };

    const conn = server.use(project);
    conn.on("state", onState);
    conn.on("tree", onTree);
    conn.on("value", onValue);
    onState(conn.state);
    // Verbindung lief schon (anderer Node, Teil-Deploy): aktuellen Stand übernehmen, sobald der
    // Flow steht (im Konstruktor gesendet ginge verloren)
    const replay = setImmediate(() => {
      if (conn.points.size) onTree(conn.points);
      for (const v of conn.values.values()) if (conn.points.has(v.no)) onValue(v, conn.points.get(v.no));
    });

    node.on("close", () => {
      clearImmediate(replay);
      conn.off("state", onState);
      conn.off("tree", onTree);
      conn.off("value", onValue);
      server.release(project);
    });
  }

  RED.nodes.registerType("mbusio-in", MbusioInNode);
};
