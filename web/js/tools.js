"use strict";

// Werkzeuge: Auslesen, Zähler einstellen, Hervorheben, Telegramm-Log

// ------------------------------------------------------------ Auslesen

function renderReading(r) {
  lastReading = r;
  const h = r.header || {};
  const meta = [
    ["meta_id", h.id],
    ["meta_manufacturer", h.manufacturer],
    ["meta_medium", h.medium && label(h.medium)],
    ["meta_version", h.version],
    ["meta_primary", r.primary_address],
    ["meta_secondary", h.secondary_address],
    ["meta_access", h.access_no],
    ["meta_status", h.status !== undefined ? "0x" + h.status.toString(16).padStart(2, "0").toUpperCase() : undefined],
    ["meta_time", r.timestamp + (r.time_unsure ? ` (${t("reading_time_unsure")})` : "")],
  ]
    .filter(([, v]) => v !== undefined && v !== null)
    .map(([k, v]) => `<div><span>${t(k)}</span><span class="mono">${esc(v)}</span></div>`)
    .join("");
  // erster Datensatz gross, meist der Hauptzählerstand
  const first = r.records[0];
  const hero = first
    ? `<div class="hero">
        <span class="hero-label">${esc(label(first.quantity))} · ${esc(label(first.function))}</span>
        <div class="hero-value"><span class="mono">${esc(first.value_text)}</span><span class="unit">${esc(first.unit)}</span></div>
        <span class="hero-sub">${esc([h.manufacturer, h.medium && label(h.medium), r.timestamp].filter(Boolean).join(" · "))}</span>
      </div>`
    : "";
  const rows = r.records
    .map((x, i) => {
      const extra = [];
      if (x.storage) extra.push(`${t("storage")} ${x.storage}`);
      if (x.tariff) extra.push(`${t("tariff")} ${x.tariff}`);
      if (x.subunit) extra.push(`${t("subunit")} ${x.subunit}`);
      const notes = x.notes.length ? ` (${x.notes.map(label).join(", ")})` : "";
      return `<tr>
        <td class="num mono dim">${i + 1}</td>
        <td>${esc(label(x.quantity) + notes)}</td>
        <td class="num mono strong">${esc(x.value_text)}</td>
        <td>${esc(x.unit)}</td>
        <td class="dim">${esc(label(x.function))}${extra.length ? " · " + extra.join(", ") : ""}</td>
        <td class="mono dim">${esc(x.dif)} | ${esc(x.vif)}</td>
      </tr>`;
    })
    .join("");
  $("reading").innerHTML = `
    <div class="${hero ? "reading-top" : ""}">${hero}<div class="meta">${meta}</div></div>
    <div class="table-wrap"><table class="reading-table">
      <thead><tr><th>#</th><th>${t("th_desc")}</th><th class="num">${t("th_value")}</th><th>${t("th_unit")}</th><th>${t("th_kind")}</th><th>DIF | VIF</th></tr></thead>
      <tbody>${rows}</tbody>
    </table></div>
    <div id="send-bar" class="send-bar"></div>`;
  renderSendBar(r);
}

/// Zähler der Zählerliste zu dieser Auslesung (über Sekundäradresse, ID oder Adresse)
function meterOfReading(r) {
  const h = r.header || {};
  return (
    meters.find((m) => h.secondary_address && m.secondary_address === h.secondary_address) ||
    meters.find((m) => h.id && (m.meter_id === h.id || m.address === h.id)) ||
    meters.find((m) => m.address.toUpperCase() === String(r.target).toUpperCase()) ||
    meters.find((m) => m.address === String(r.primary_address))
  );
}

/// Unter der Auslesung: Link zur Weitergabe (bzw. erst in die Zählerliste aufnehmen)
function renderSendBar(r) {
  const bar = $("send-bar");
  if (!bar) return;
  const m = meterOfReading(r);
  bar.innerHTML = m
    ? `<p class="hint">${esc(t("send_link_hint"))} <button type="button" class="small tonal" data-open-transfer="${m.id}">${esc(t("send_link"))}</button></p>`
    : `<p class="hint">${esc(t("send_not_listed"))} <button type="button" class="small ghost" id="send-add">${esc(t("send_add"))}</button></p>`;
}

$("reading").addEventListener("click", (ev) => {
  const open = ev.target.closest("[data-open-transfer]");
  if (open) return guarded(open, () => openTransfer(Number(open.dataset.openTransfer)));
  const add = ev.target.closest("#send-add");
  if (add && lastReading) {
    guarded(add, async () => {
      const h = lastReading.header || {};
      // gemerkt wie die Linie: über SA oder PA
      const address = addressing() === "primary" ? String(lastReading.primary_address) : h.secondary_address || lastReading.target;
      await api("meters", { method: "POST", body: { address } });
      meters = await api("meters");
      renderMeters();
      // die Auslesung gehört jetzt zum Zähler: noch einmal speichern, damit die Weitergabe die Werte kennt
      await api("read", { method: "POST", body: { address } }).then(renderReading).catch(() => renderSendBar(lastReading));
    });
  }
});

/// `baud`: Treffer aus dem Scan über alle Baudraten mit dessen Baudrate lesen
async function readMeter(baud = null) {
  await guarded($("read"), async () => {
    const address = $("read-addr").value.trim();
    if (!address) throw new Error(t("enter_address"));
    lastReading = null;
    $("reading").innerHTML = `<p class="loading"><span class="spin"></span>${t("reading_busy")}</p>`;
    try {
      renderReading(await api("read", { method: "POST", body: { address, baud } }));
      await refreshStatus();
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    }
  });
}

$("read").onclick = () => readMeter();
$("read-addr").addEventListener("keydown", (e) => e.key === "Enter" && readMeter());

// ------------------------------------------------------------ Zähler einstellen

// ------------------------------------------------------------ Hervorheben, was zu tun ist

/// Knopf, der gerade hervorgehoben ist
let spotButton = null;

/// Kachel hervorheben, die anderen abblenden, Knopf mit Ring zeigen und in einem Satz sagen, was zu tun ist
function spotlight(tileId, buttonId, text) {
  clearSpotlight();
  $(tileId).closest(".tiles").classList.add("has-spotlight");
  $(tileId).classList.add("spotlight");
  spotButton = $(buttonId);
  spotButton.classList.add("go");
  const c = document.createElement("div");
  c.id = "callout";
  c.className = "callout";
  c.innerHTML = `<p>${esc(text)}</p><a href="#hits">${esc(t("back_to_hits"))}</a>`;
  $(tileId).querySelector("h3").after(c);
  reveal($(tileId), "center");
}

/// Nach Erfolg: grüne Bestätigung stehen lassen, Hervorhebung aufheben
function spotlightDone(text) {
  const c = $("callout");
  if (!c) return;
  c.classList.add("done");
  c.querySelector("p").textContent = text;
  document.querySelectorAll(".has-spotlight").forEach((e) => e.classList.remove("has-spotlight"));
  spotButton?.classList.remove("go");
}

function clearSpotlight() {
  $("callout")?.remove();
  document.querySelectorAll(".has-spotlight").forEach((e) => e.classList.remove("has-spotlight"));
  document.querySelectorAll(".spotlight").forEach((e) => e.classList.remove("spotlight"));
  spotButton?.classList.remove("go");
}

/// Baudrate, mit der der eingetragene Zähler läuft (null = wie der Bus)
let configBaud = null;

function setConfigTarget(address, baud = null) {
  $("set-old").value = address;
  setConfigBaud(baud);
  renderTarget();
}

/// Ziel als Zähler-Baustein: aus der Zählerliste oder dem letzten Scan, sonst nur die Adresse
function renderTarget() {
  const a = $("set-old").value.trim();
  const m = a && meters.find((x) => [x.address, x.meter_id, x.secondary_address, String(x.primary_address)].includes(a));
  const h = !m && a && lastJob?.hits.find((x) => [x.id, x.secondary_address, String(x.primary_address)].includes(a));
  const block = m
    ? meterBlock(m)
    : h
      ? { name: h.medium ? `${label(h.medium)} · ${h.manufacturer || ""}` : "", ident: h.id, pa: h.primary_address, medium: h.medium, info: h.baud ? `${h.baud} Bd` : "" }
      : null;
  $("target-meter").innerHTML = block ? ui.meter(block) : `<span class="hint">${esc(t(a ? "target_free" : "target_none"))}</span>`;
  // jetzt → neu: aktueller Wert des Ziels über den Feldern
  const pa = m ? m.primary_address : h?.primary_address;
  const baud = (m ? m.baud : h?.baud) || (block ? connBaud : null);
  $("cur-pa").textContent = pa != null ? t("target_now", `PA ${pa}`) : "";
  $("cur-baud").textContent = baud ? t("target_now", `${baud} Bd`) : "";
  $("target-list").innerHTML = meters.length
    ? meters
        .map((x) => `<button type="button" data-target="${esc(x.address)}" data-baud="${x.baud ?? ""}">${ui.meterShort({ name: x.name, ident: x.meter_id || x.address, medium: x.medium })}</button>`)
        .join("")
    : `<span class="hint menu-group">${esc(t("meters_empty_short"))}</span>`;
  updateButtons();
}

$("target-list").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-target]");
  if (b) setConfigTarget(b.dataset.target, Number(b.dataset.baud) || null);
});

function setConfigBaud(baud) {
  configBaud = baud && baud !== connBaud ? baud : null;
  const note = $("config-baud-note");
  note.hidden = !configBaud;
  note.textContent = configBaud ? t("config_baud_note", configBaud) : "";
}

// Wer selbst eine andere Adresse eintippt, meint einen Zähler auf der Bus-Baudrate
$("set-old").addEventListener("input", () => {
  setConfigBaud(null);
  clearSpotlight();
  renderTarget();
});

/// Treffer der letzten Suche zu einer eingegebenen Adresse
function hitFor(address) {
  const key = address.toUpperCase();
  return lastJob?.hits.find((h) => h.id === key || h.secondary_address === key || String(h.primary_address) === key) ?? null;
}

function configTarget() {
  const address = $("set-old").value.trim();
  if (!address) throw new Error(t("enter_address"));
  return address;
}

/// Zähler der Liste mit derselben Adresse (gleiche Schreibweise)
function listedMeter(address) {
  return meters.find((m) => m.address.toUpperCase() === address.toUpperCase());
}

$("set-addr").onclick = () =>
  guarded($("set-addr"), async () => {
    const address = $("set-old").value.trim();
    const newAddr = $("set-new").value;
    if (!address || newAddr === "") throw new Error(t("enter_both"));
    if (!confirm(t("confirm_set", address, newAddr))) return;
    await doSetAddress(address, Number(newAddr), configBaud);
    toast(t("address_set", newAddr));
    spotlightDone(t("spot_done_pa", address, newAddr));
  });

/// Primäradresse setzen und überall nachführen: Trefferliste, Zählerliste, Änderungsprotokoll
async function doSetAddress(address, newAddr, fromBaud) {
  const hit = hitFor(address);
  await api("set-address", { method: "POST", body: { address, new_address: newAddr, from_baud: fromBaud } });
  // Trefferliste nachführen, damit die Warnung "PA doppelt" ohne neuen Scan verschwindet
  if (hit) {
    hit.primary_address = newAddr;
    renderJob(lastJob);
  }
  // steht der Zähler mit seiner Primäradresse in der Liste, die neue eintragen
  const m = listedMeter(String(address));
  if (m && /^\d+$/.test(m.address)) {
    await api("meters/" + m.id, { method: "PUT", body: { ...m, address: String(newAddr) } });
    await loadMeters();
  }
}

/// Baudrate umstellen und überall nachführen; true, wenn die Zählerliste angepasst wurde
async function doSwitchBaud(address, baud, fromBaud) {
  const hit = hitFor(address);
  await api("switch-baud", { method: "POST", body: { address, baud, from_baud: fromBaud } });
  if (hit) {
    hit.baud = baud;
    renderJob(lastJob);
  }
  const m = listedMeter(address);
  if (!m) return false;
  // eigene Baudrate nur speichern, wenn sie vom Bus abweicht
  await api("meters/" + m.id, { method: "PUT", body: { ...m, baud: baud === connBaud ? null : baud } });
  await loadMeters();
  return true;
}



$("switch-baud").onclick = () =>
  guarded($("switch-baud"), async () => {
    const address = configTarget();
    const baud = Number($("new-baud").value);
    if (!confirm(t("confirm_switch", address, baud))) return;
    const listed = await doSwitchBaud(address, baud, configBaud);
    setConfigBaud(baud);
    spotlightDone(t("spot_done_baud", address, baud));
    toast(t(listed ? "baud_switched_list" : "baud_switched", baud));
  });

$("detect-baud").onclick = () =>
  guarded($("detect-baud"), async () => {
    const address = configTarget();
    const out = $("detect-result");
    out.textContent = t("detect_busy");
    try {
      const r = await api("detect-baud", { method: "POST", body: { address } });
      out.textContent = r.bauds.length ? t("detect_found", r.bauds.join(", ")) : t("detect_none");
      if (r.bauds.length && !r.bauds.includes(connBaud)) {
        // läuft woanders: Befehle mit seiner Baudrate senden, Umstellen auf den Bus vorschlagen
        setConfigBaud(r.bauds[0]);
        $("new-baud").value = connBaud;
      } else if (r.bauds.length) {
        setConfigBaud(null);
      }
    } catch (e) {
      out.textContent = "";
      throw e;
    }
  });

$("app-reset").onclick = () =>
  guarded($("app-reset"), async () => {
    const address = configTarget();
    if (!confirm(t("confirm_reset", address))) return;
    await api("reset", { method: "POST", body: { address, from_baud: configBaud } });
    toast(t("reset_done"));
  });

// ------------------------------------------------------------ Log

async function pollLog() {
  if (!$("log-box").open) return;
  const entries = await api("log?since=" + logSeq);
  if (!entries.length) return;
  logSeq = entries[entries.length - 1].seq;
  const pre = $("log");
  pre.insertAdjacentHTML(
    "beforeend",
    entries
      .map((e) => {
        // TX/RX-Zeilen anklicken: Telegramm analysieren
        const bytes = e.dir === "TX" || e.dir === "RX" ? ` data-hex="${esc(e.text)}" title="${esc(t("log_click"))}"` : "";
        return `<span class="log-${e.dir}"${bytes}>${e.time} ${e.dir.padEnd(4)} ${esc(e.text)}</span>\n`;
      })
      .join("")
  );
  while (pre.childElementCount > 1000) pre.firstElementChild.remove();
  if ($("log-follow").checked) pre.scrollTop = pre.scrollHeight;
}

$("raw-send").onclick = () =>
  guarded($("raw-send"), async () => {
    const hex = $("raw-hex").value.trim();
    if (!hex) return;
    const r = await api("raw", { method: "POST", body: { hex } });
    const lines = [t("raw_" + r.kind) + (r.hex && r.kind !== "ack" ? ": " + r.hex : "")];
    if (r.decoded) {
      const h = r.decoded.header;
      if (h) lines.push(`ID ${h.id} · ${h.manufacturer} · ${label(h.medium)}`);
      for (const x of r.decoded.records) lines.push(`${label(x.quantity)}: ${x.value_text} ${x.unit}`.trim());
    }
    const out = $("raw-result");
    out.textContent = lines.join("\n");
    out.hidden = false;
  });
$("raw-hex").addEventListener("keydown", (e) => e.key === "Enter" && $("analyze").click());

$("analyze").onclick = () => guarded($("analyze"), () => analyzeHex($("raw-hex").value.trim()));

$("log").addEventListener("click", (ev) => {
  const hex = ev.target.closest("[data-hex]")?.dataset.hex;
  if (hex) analyzeHex(hex).catch((e) => toast(e.message, true));
});

/// Telegramm erklären lassen und unter dem Log anzeigen (sendet nichts)
async function analyzeHex(hex) {
  if (!hex) return;
  const box = $("log-box");
  if (!box.open) box.open = true;
  $("raw-hex").value = hex;
  const a = await api("analyze", { method: "POST", body: { hex } });
  const el = $("analysis");
  el.hidden = false;
  el.innerHTML = `
    <h4>${esc(a.summary)}</h4>
    ${a.problems.length
      ? `<ul class="problems">${a.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul>`
      : `<p class="clean">${esc(t("an_clean"))}</p>`}
    <div class="table-wrap"><table>
      <thead><tr><th>${esc(t("an_bytes"))}</th><th>${esc(t("an_field"))}</th><th>${esc(t("an_meaning"))}</th></tr></thead>
      <tbody>${a.fields
        .map((f) => `<tr${f.bad ? ' class="bad"' : ""}><td class="bytes">${esc(f.bytes || "–")}</td><td>${esc(f.name)}</td><td>${esc(f.value)}</td></tr>`)
        .join("")}</tbody>
    </table></div>`;
  reveal(el, "start");
}

$("clear-log").onclick = async () => {
  await api("log", { method: "DELETE" }).catch(() => {});
  $("log").innerHTML = "";
};
