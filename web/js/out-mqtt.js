"use strict";

// Ausgang MQTT im Reiter Weitergabe: Einstellungen, Zustand, volles Topic.
// Was für alle Ausgänge gleich ist, steht in transfer.js (OUTPUT_UI).

let mqttConfig = null;
let mqttHasPassword = false;

const mqttBase = () => (mqttConfig?.base || "mbusio").replace(/\/+$/, "");

/// Felder der MQTT-Einstellungen (Baustein Formular)
function mqttFields() {
  const base = mqttBase();
  return [
    { name: "enabled", label: t("mqtt_enabled"), type: "check", wide: true },
    { name: "host", label: t("mqtt_host"), cls: "mono", placeholder: "192.168.1.10", wide: true },
    { name: "port", label: t("mqtt_port"), type: "number", cls: "mono" },
    { name: "tls", label: "TLS", type: "check" },
    { name: "insecure", label: t("mqtt_insecure"), type: "check", wide: true },
    { name: "username", label: t("mqtt_user") },
    { name: "password", label: t("mqtt_password"), type: "password", placeholder: mqttHasPassword ? t("mqtt_pass_saved") : "" },
    { name: "client_id", label: t("mqtt_client"), cls: "mono", placeholder: "mbusio-…" },
    { name: "base", label: t("mqtt_base"), cls: "mono", placeholder: "mbusio" },
    { name: "topic", label: t("mqtt_topic"), cls: "mono", wide: true, placeholder: "{projekt}/{gateway}-{linie}/{zaehler}/{wert}", hint: t("mqtt_topic_hint") },
    { name: "time_sync", label: t("mqtt_time", base + "/time"), type: "check", wide: true },
    { name: "ca", label: t("mqtt_ca"), type: "textarea", cls: "mono", placeholder: "-----BEGIN CERTIFICATE-----", wide: true },
    { name: "ha", label: t("mqtt_ha"), type: "check", wide: true },
    { name: "ha_prefix", label: t("mqtt_ha_prefix"), cls: "mono", placeholder: "homeassistant", hint: t("mqtt_ha_hint") },
  ];
}

OUTPUT_UI.mqtt = {
  async load() {
    const mq = await api("mqtt");
    mqttConfig = mq.config;
    mqttHasPassword = mq.has_password;
    mqttStatus = mq.status;
  },

  fromStatus(s) {
    mqttStatus = s.mqtt;
  },

  state() {
    const s = mqttStatus;
    if (!s) return { chip: ui.chip("muted", t("mqtt_off")), text: "" };
    const kind = { connected: "ok", error: "warn", connecting: "muted" }[s.state] || "muted";
    const text =
      s.state === "connected"
        ? t("mqtt_state_connected", s.broker, s.since, s.sent, s.queued)
        : s.state === "connecting"
          ? t("mqtt_state_connecting", s.broker)
          : s.state === "error"
            ? t("mqtt_state_error", s.broker, s.error, s.queued)
            : t("mqtt_state_off");
    return { chip: ui.chip(kind, t("mqtt_" + (s.state || "off"))), text };
  },

  form() {
    return `${ui.form(mqttFields(), { ...mqttConfig, password: "" })}
      <p class="hint mono" id="mqtt-example"></p>
      <div class="row"><button type="submit">${esc(t("save"))}</button></div>`;
  },

  /// TLS-Felder nur mit TLS, Präfix nur mit Home Assistant; Beispiel-Topic aktuell halten
  update(f) {
    const tls = f.querySelector("[name=tls]").checked;
    for (const n of ["insecure", "ca"]) f.querySelector(`[data-field="${n}"]`).hidden = !tls;
    f.querySelector('[data-field="ha_prefix"]').hidden = !f.querySelector("[name=ha]").checked;
    const base = (f.querySelector("[name=base]").value.trim() || "mbusio").replace(/\/+$/, "");
    const sample = allDatapoints().find((d) => d.outputs.mqtt?.address)?.outputs.mqtt.address || "projekt/gateway-1-linie-1/whg-1/energy";
    $("mqtt-example").textContent = t("mqtt_example", `${base}/${sample}`);
  },

  change(f, ev) {
    if (ev.target.name === "tls") {
      // üblicher Port mitwechseln, solange man keinen eigenen hat
      const port = f.querySelector("[name=port]");
      if (["", "1883", "8883"].includes(port.value.trim())) port.value = ev.target.checked ? "8883" : "1883";
    }
    this.update(f);
  },

  async save(f) {
    const v = ui.formValues(f, mqttFields());
    await api("mqtt", {
      method: "PUT",
      body: {
        config: { ...v, port: v.port || (v.tls ? 8883 : 1883), base: v.base || "mbusio", ha_prefix: v.ha_prefix || "homeassistant", password: "" },
        // leer = gespeichertes behalten; Benutzer gelöscht = Passwort auch
        password: v.password ? v.password : v.username ? null : "",
      },
    });
    toast(t("mqtt_saved"));
  },

  fullAddress: (address) => `${mqttBase()}/${address}`,

  /// Für die Statusleiste: Broker getrennt (mit wartenden Nachrichten), sonst nichts
  problem: () => (mqttStatus?.state === "error" ? t("mqtt_busbar", mqttStatus.queued) : null),
};
