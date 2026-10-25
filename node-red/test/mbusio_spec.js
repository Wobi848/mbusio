const http = require("http");
const helper = require("node-red-node-test-helper");
const serverNode = require("../nodes/mbusio-server.js");
const inNode = require("../nodes/mbusio-in.js");
const statusNode = require("../nodes/mbusio-status.js");
const { sseParser } = require("../lib/connection");
helper.init(require.resolve("node-red"));

// Nachgebautes mbusio: Baum und Strom, vom Test gesteuert
function fakeMbusio({ token = "", version = 1 } = {}) {
  const fake = { streams: [], tokens: [], version };
  fake.tree = {
    projects: [
      {
        id: 1,
        name: "Musterstrasse 5",
        gateways: [
          {
            id: 1,
            name: "Keller",
            lines: [
              {
                id: 7,
                name: "Linie 1",
                operation: true,
                meters: [
                  {
                    id: 12,
                    name: "Whg 1",
                    ident: "20395066",
                    pa: 3,
                    datapoints: [
                      { no: 1, name: "Whg 1 · Energie", unit: "kWh" },
                      { no: 2, name: "Whg 1 · Leistung", unit: "W" },
                    ],
                  },
                  { id: 13, name: "Whg 2", ident: "20395067", pa: 4, datapoints: [{ no: 3, name: "Whg 2 · Energie", unit: "kWh" }] },
                ],
              },
            ],
          },
        ],
      },
    ],
  };
  fake.server = http.createServer((req, res) => {
    fake.tokens.push(req.headers["x-token"]);
    if (token && req.headers["x-token"] !== token) {
      res.writeHead(401).end();
      return;
    }
    if (req.url === "/api/nodered/tree") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ version: fake.version, projects: fake.tree.projects }));
      return;
    }
    if (req.url.startsWith("/api/nodered/stream?project=1")) {
      res.writeHead(200, { "content-type": "text/event-stream" });
      res.write(": ping\n\n");
      fake.streams.push(res);
      return;
    }
    res.writeHead(404).end();
  });
  fake.send = (name, data) => {
    for (const s of fake.streams) s.write(`event: ${name}\ndata: ${JSON.stringify(data)}\n\n`);
  };
  fake.drop = () => {
    for (const s of fake.streams) s.destroy();
    fake.streams = [];
  };
  return new Promise((resolve) => fake.server.listen(0, "127.0.0.1", () => resolve(fake)));
}

const value = (no, v, quality = "good", time = "2026-10-25T10:00:00Z") => ({ no, value: v, unit: "kWh", time, quality });

const waitFor = async (cond, ms = 2000) => {
  const end = Date.now() + ms;
  while (!cond()) {
    if (Date.now() > end) throw new Error("timeout");
    await new Promise((r) => setTimeout(r, 10));
  }
};

describe("mbusio nodes", function () {
  let fake;
  before(() => helper.startServer());
  after(() => helper.stopServer());
  afterEach(async () => {
    await helper.unload();
    if (fake) {
      fake.drop();
      fake.server.close();
      fake = null;
    }
  });

  async function load(inConfig, opts = {}) {
    fake = await fakeMbusio(opts);
    const url = `http://127.0.0.1:${fake.server.address().port}`;
    const flow = [
      { id: "s1", type: "mbusio-server", url },
      Object.assign({ id: "n1", type: "mbusio-in", server: "s1", project: "1", scope: "datapoints", datapoints: [1], wires: [["out"]] }, inConfig),
      { id: "out", type: "helper" },
    ];
    await helper.load([serverNode, inNode, statusNode], flow, { s1: { token: opts.token || "" } });
    const msgs = [];
    helper.getNode("out").on("input", (m) => msgs.push(m));
    await waitFor(() => fake.streams.length === 1);
    return { msgs, n1: helper.getNode("n1") };
  }

  it("parses server-sent events split across chunks", function () {
    const parse = sseParser();
    parse(": ping\n\nevent: val").should.eql([]);
    parse('ue\ndata: {"no":1}\n\nevent: changed\ndata: {}\n\n').should.eql([
      { name: "value", data: '{"no":1}' },
      { name: "changed", data: "{}" },
    ]);
  });

  it("sends selected datapoints with names and quality", async function () {
    const { msgs } = await load();
    fake.send("value", value(1, 123456));
    fake.send("value", value(2, 999)); // nicht gewählt
    await waitFor(() => msgs.length === 1);
    msgs[0].payload.should.equal(123456);
    msgs[0].topic.should.equal("Whg 1 · Energie");
    msgs[0].mbusio.should.containDeep({ no: 1, unit: "kWh", quality: "good", meter: "Whg 1", ident: "20395066", pa: 3, gateway: "Keller", line: "Linie 1", project: "Musterstrasse 5" });
  });

  it("holds back values that are not good unless all are wanted", async function () {
    const { msgs, n1 } = await load();
    fake.send("value", value(1, 1, "disturbed"));
    await waitFor(() => n1.status.args.some((a) => a[0].fill === "red"));
    msgs.should.have.length(0);
    await helper.unload();
    fake.drop();
    fake.server.close();
    const all = await load({ quality: "all" });
    fake.send("value", value(1, 1, "disturbed"));
    await waitFor(() => all.msgs.length === 1);
    all.msgs[0].mbusio.quality.should.equal("disturbed");
  });

  it("takes all datapoints of a meter, also new ones", async function () {
    const { msgs } = await load({ scope: "meter", meter: "13", datapoints: [] });
    fake.send("value", value(1, 1));
    fake.send("value", value(3, 3));
    await waitFor(() => msgs.length === 1);
    msgs[0].mbusio.no.should.equal(3);
    // neuer Datenpunkt an diesem Zähler: nach "changed" mit Namen dabei
    fake.tree.projects[0].gateways[0].lines[0].meters[1].datapoints.push({ no: 4, name: "Whg 2 · Volumen", unit: "m³" });
    fake.send("changed", {});
    fake.send("value", value(4, 4));
    await waitFor(() => msgs.length === 2);
    msgs[1].topic.should.equal("Whg 2 · Volumen");
  });

  it("takes all datapoints of a line", async function () {
    const { msgs } = await load({ scope: "line", line: "7", datapoints: [] });
    for (const no of [1, 2, 3]) fake.send("value", value(no, no));
    await waitFor(() => msgs.length === 3);
    msgs.map((m) => m.mbusio.meter).should.eql(["Whg 1", "Whg 1", "Whg 2"]);
  });

  it("does not repeat unchanged values after reconnecting", async function () {
    const { msgs } = await load();
    fake.send("value", value(1, 5));
    await waitFor(() => msgs.length === 1);
    // Verbindung bricht ab; die Wartezeit vor dem Neuversuch abkürzen
    const conn = helper.getNode("s1").use(1);
    helper.getNode("s1").release(1);
    conn.retryMs = 20;
    fake.drop();
    await waitFor(() => fake.streams.length === 1, 15000);
    fake.send("value", value(1, 5)); // Stand nach dem Verbinden: schon bekannt
    fake.send("value", value(1, 6, "good", "2026-10-25T10:15:00Z"));
    await waitFor(() => msgs.length === 2);
    msgs[1].payload.should.equal(6);
  });

  it("sends the access key and reports a wrong one", async function () {
    fake = await fakeMbusio({ token: "geheim" });
    const url = `http://127.0.0.1:${fake.server.address().port}`;
    await helper.load(
      [serverNode, inNode],
      [
        { id: "s1", type: "mbusio-server", url },
        { id: "n1", type: "mbusio-in", server: "s1", project: "1", datapoints: [1], wires: [[]] },
      ],
      { s1: { token: "falsch" } }
    );
    const n1 = helper.getNode("n1");
    await waitFor(() => n1.status.args.some((a) => a[0].text === "mbusio-in.status.denied"));
    fake.tokens.should.containEql("falsch");
  });

  it("reports a datapoint that is missing in mbusio and an unsuitable version", async function () {
    const { n1 } = await load({ datapoints: [1, 9] });
    await waitFor(() => n1.status.args.some((a) => a[0].text === "mbusio-in.status.missing"));
    await helper.unload();
    fake.drop();
    fake.server.close();
    fake = await fakeMbusio({ version: 2 });
    const url = `http://127.0.0.1:${fake.server.address().port}`;
    await helper.load(
      [serverNode, inNode],
      [
        { id: "s1", type: "mbusio-server", url },
        { id: "n1", type: "mbusio-in", server: "s1", project: "1", datapoints: [1], wires: [[]] },
      ]
    );
    const n = helper.getNode("n1");
    await waitFor(() => n.status.args.some((a) => a[0].text === "mbusio-in.status.version"));
  });

  // mbusio-status anstelle von mbusio-in laden
  const loadStatus = (config) => load(Object.assign({ type: "mbusio-status", line: "" }, config));

  it("status: one message once all is known, payload true when all is fine", async function () {
    const { msgs } = await loadStatus();
    for (const no of [1, 2, 3]) fake.send("value", value(no, no));
    await waitFor(() => msgs.length === 1);
    msgs[0].payload.should.equal(true);
    msgs[0].mbusio.should.containDeep({ online: true, good: 3, stale: 0, disturbed: 0, invalid: 0, problems: [] });
    // gleicher Zustand: keine weitere Nachricht
    fake.send("value", value(1, 1, "good", "2026-10-25T10:15:00Z"));
    await new Promise((r) => setTimeout(r, 700));
    msgs.should.have.length(1);
  });

  it("status: disturbed values make payload false and are listed; stale only listed", async function () {
    const { msgs } = await loadStatus();
    fake.send("value", value(1, 1));
    fake.send("value", value(2, 2, "stale"));
    await waitFor(() => msgs.length === 1);
    msgs[0].payload.should.equal(true);
    msgs[0].mbusio.problems.should.containDeep([{ no: 2, quality: "stale", meter: "Whg 1" }]);
    fake.send("value", value(1, 1, "disturbed"));
    await waitFor(() => msgs.length === 2);
    msgs[1].payload.should.equal(false);
    msgs[1].mbusio.disturbed.should.equal(1);
    msgs[1].mbusio.problems.map((x) => x.no).should.eql([1, 2]);
  });

  it("status: only the chosen line, and false when mbusio is gone", async function () {
    const { msgs } = await loadStatus({ line: "99" });
    fake.send("value", value(1, 1, "disturbed")); // andere Linie
    await new Promise((r) => setTimeout(r, 700));
    msgs.should.have.length(1);
    msgs[0].payload.should.equal(true);
    msgs[0].mbusio.disturbed.should.equal(0);
    fake.drop();
    fake.server.close();
    await waitFor(() => msgs.length === 2, 3000);
    msgs[1].payload.should.equal(false);
    msgs[1].mbusio.online.should.equal(false);
  });

  it("gives the editor the tree through Node-RED", async function () {
    await load();
    const url = `http://127.0.0.1:${fake.server.address().port}`;
    const res = await helper.request().post("/mbusio/tree").send({ id: "s1", url }).expect(200);
    res.body.projects[0].name.should.equal("Musterstrasse 5");
    await helper.request().post("/mbusio/tree").send({ url: "ftp://x" }).expect(400);
  });
});
