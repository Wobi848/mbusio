// Bereich Übersicht: Texte (Deutsch, Englisch) zu den gemeinsamen aus js/texts.js

import { texts } from "./view.js";

texts(
  {
    ov_all_set: "Alles ist eingerichtet.",
    ov_ok: (n) => `${n} in Ordnung`,
    ov_faulty: (n) => `${n} gestört`,
    ov_unread: (n) => `${n} ungelesen`,
    ov_to_meters: "Zur Zählerliste",
    ov_change_plan: "Plan ändern",
    ov_to_transfer: "Zur Weitergabe",
    ov_no_output: "Kein Ausgang eingeschaltet",
    ov_device: "Gerät",
    ov_messages: "Meldungen",
  },
  {
    ov_all_set: "Everything is set up.",
    ov_ok: (n) => `${n} OK`,
    ov_faulty: (n) => `${n} faulty`,
    ov_unread: (n) => `${n} unread`,
    ov_to_meters: "Go to the meter list",
    ov_change_plan: "Change the plan",
    ov_to_transfer: "Go to transfer",
    ov_no_output: "No output switched on",
    ov_device: "Device",
    ov_messages: "Messages",
  }
);
