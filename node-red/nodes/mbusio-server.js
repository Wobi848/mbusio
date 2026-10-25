"use strict";

const { Connection } = require("../lib/connection");

module.exports = function (RED) {
  function MbusioServerNode(config) {
    RED.nodes.createNode(this, config);
    const node = this;
    node.url = String(config.url || "").replace(/\/+$/, "");
    // Projekt → { conn, users }: eine Verbindung je Projekt, geteilt von allen mbusio-in
    const shared = new Map();

    /** Verbindung zu einem Projekt holen (wird gestartet, wenn sie neu ist) */
    node.use = (project) => {
      let s = shared.get(project);
      if (!s) {
        s = { conn: new Connection({ url: node.url, token: node.credentials.token, project }), users: 0 };
        s.conn.setMaxListeners(0); // ein Satz Listener je mbusio-in
        shared.set(project, s);
        s.conn.start();
      }
      s.users++;
      return s.conn;
    };

    /** Verbindung zurückgeben; die letzte beendet sie */
    node.release = (project) => {
      const s = shared.get(project);
      if (s && --s.users <= 0) {
        s.conn.stop();
        shared.delete(project);
      }
    };

    node.on("close", () => {
      for (const s of shared.values()) s.conn.stop();
      shared.clear();
    });
  }

  RED.nodes.registerType("mbusio-server", MbusioServerNode, {
    credentials: { token: { type: "password" } },
  });

  // Auswahl im Editor: Baum von mbusio holen. Läuft über Node-RED, damit der Zugriffsschlüssel
  // auf dem Server bleibt und der Browser nicht selbst mbusio erreichen muss.
  RED.httpAdmin.post("/mbusio/tree", RED.auth.needsPermission("flows.write"), async (req, res) => {
    const body = req.body || {};
    const deployed = body.id ? RED.nodes.getNode(body.id) : null;
    const url = String(body.url || deployed?.url || "").replace(/\/+$/, "");
    const token = body.token || deployed?.credentials?.token || "";
    if (!/^https?:\/\/[^/]+/.test(url)) return res.status(400).json({ error: "url" });
    try {
      const r = await fetch(url + "/api/nodered/tree", {
        headers: token ? { "x-token": token } : {},
        signal: AbortSignal.timeout(5000),
      });
      res.status(r.status).json(await r.json());
    } catch (e) {
      res.status(502).json({ error: e.cause?.code || e.message });
    }
  });
};
