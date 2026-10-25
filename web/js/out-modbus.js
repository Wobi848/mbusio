"use strict";

// Ausgang Modbus TCP im Reiter Weitergabe: Einstellungen, Zustand, Register je Datenpunkt.
// Was für alle Ausgänge gleich ist, steht in transfer.js (OUTPUT_UI).

let modbusConfig = null;
let modbusStatus = null;
let modbusProjects = [];

function modbusFields() {
  return [
    { name: "enabled", label: t("modbus_enabled"), type: "check", wide: true },
    { name: "bind", label: t("modbus_bind"), cls: "mono", placeholder: "0.0.0.0", hint: t("modbus_bind_hint") },
    { name: "port", label: t("modbus_port"), type: "number", cls: "mono", hint: t("modbus_port_hint") },
    { name: "project", label: t("modbus_project"), type: "select", num: true, options: [[0, t("modbus_project_active")], ...modbusProjects.map((p) => [p.id, p.name])] },
    { name: "swap_words", label: t("modbus_words"), type: "select", options: [["false", t("modbus_words_abcd")], ["true", t("modbus_words_cdab")]] },
  ];
}

OUTPUT_UI.modbus = {
  async load() {
    const mb = await api("modbus");
    modbusConfig = mb.config;
    modbusStatus = mb.status;
    modbusProjects = mb.projects;
  },

  fromStatus(s) {
    modbusStatus = s.modbus;
  },

  state() {
    const s = modbusStatus;
    if (!s || s.state === "off") return { chip: ui.chip("muted", t("modbus_off")), text: t("modbus_state_off") };
    if (s.state === "error") return { chip: ui.chip("warn", t("modbus_error")), text: t("modbus_state_error", s.listen, s.error) };
    return { chip: ui.chip("ok", t("modbus_listening")), text: t("modbus_state_listening", s.listen, s.clients, s.requests) };
  },

  form() {
    return `${ui.form(modbusFields(), { ...modbusConfig, swap_words: String(!!modbusConfig?.swap_words) })}
      <div class="row"><button type="submit">${esc(t("save"))}</button></div>`;
  },

  async save(f) {
    const v = ui.formValues(f, modbusFields());
    await api("modbus", {
      method: "PUT",
      body: { ...v, bind: v.bind || "0.0.0.0", port: v.port || 502, project: v.project || 0, swap_words: v.swap_words === "true" },
    });
    toast(t("modbus_saved"));
  },

  /// Startregister und die 10 Register des Blocks
  fullAddress: (address) => t("modbus_block", address, Number(address) + 9),

  problem: () => (modbusStatus?.state === "error" ? t("modbus_busbar", modbusStatus.error) : null),
};
