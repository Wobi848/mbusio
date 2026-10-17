"use strict";

// MQTT-Einstellungen und Zustand

// ------------------------------------------------------------ MQTT

let mqttConfig = null;
const mqttBase = () => (mqttConfig?.base || "mbusio").replace(/\/+$/, "");

async function loadMqtt() {
  const r = await api("mqtt");
  mqttConfig = r.config;
  mqttStatus = r.status;
  const c = r.config;
  $("mq-enabled").checked = c.enabled;
  $("mq-host").value = c.host;
  $("mq-port").value = c.port;
  $("mq-tls").checked = c.tls;
  $("mq-insecure").checked = c.insecure;
  $("mq-user").value = c.username;
  $("mq-pass").value = "";
  $("mq-pass").placeholder = r.has_password ? t("mqtt_pass_saved") : "";
  $("mq-client").value = c.client_id;
  $("mq-base").value = c.base;
  $("mq-time").checked = c.time_sync;
  $("mq-ca").value = c.ca;
  updateMqttForm();
  renderMqttState();
}

function updateMqttForm() {
  const base = ($("mq-base").value.trim() || "mbusio").replace(/\/+$/, "");
  $("mq-time-label").textContent = t("mqtt_time", base + "/time");
  $("mq-insecure").closest("label").hidden = !$("mq-tls").checked;
  $("mqtt-form").querySelector(".mqtt-ca").hidden = !$("mq-tls").checked;
  const sample = datapoints.find((d) => d.mqtt_topic)?.mqtt_topic || "projekt/gateway-1-linie-1/whg-1/energy";
  $("mqtt-example").textContent = t("mqtt_example", `${base}/${sample}`);
}

function renderMqttState() {
  const s = mqttStatus;
  if (!s) return;
  const chip = $("mqtt-chip");
  chip.className = "chip " + ({ connected: "on", error: "st-collision", connecting: "st-no_data" }[s.state] || "off");
  chip.textContent = t("mqtt_" + (s.state || "off"));
  $("mqtt-state").textContent =
    s.state === "connected"
      ? t("mqtt_state_connected", s.broker, s.since, s.sent, s.queued)
      : s.state === "connecting"
        ? t("mqtt_state_connecting", s.broker)
        : s.state === "error"
          ? t("mqtt_state_error", s.broker, s.error, s.queued)
          : t("mqtt_state_off");
  renderBusbar();
}

for (const id of ["mq-tls", "mq-base"]) $(id).addEventListener("input", updateMqttForm);
$("mq-tls").addEventListener("change", () => {
  // üblicher Port mitwechseln, solange man keinen eigenen hat
  const p = $("mq-port").value.trim();
  if (!p || p === "1883" || p === "8883") $("mq-port").value = $("mq-tls").checked ? "8883" : "1883";
  updateMqttForm();
});

$("mqtt-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("mqtt-form").querySelector("[type=submit]"), async () => {
    const pass = $("mq-pass").value;
    await api("mqtt", {
      method: "PUT",
      body: {
        config: {
          enabled: $("mq-enabled").checked,
          host: $("mq-host").value.trim(),
          port: Number($("mq-port").value) || ($("mq-tls").checked ? 8883 : 1883),
          tls: $("mq-tls").checked,
          insecure: $("mq-insecure").checked,
          ca: $("mq-ca").value,
          username: $("mq-user").value.trim(),
          password: "",
          client_id: $("mq-client").value.trim(),
          base: $("mq-base").value.trim() || "mbusio",
          time_sync: $("mq-time").checked,
        },
        // leer = gespeichertes behalten; Benutzer gelöscht = Passwort auch
        password: pass ? pass : $("mq-user").value.trim() ? null : "",
      },
    });
    toast(t("mqtt_saved"));
    await loadMqtt();
    setTimeout(() => refreshStatus().catch(() => {}), 1500);
  });
};
