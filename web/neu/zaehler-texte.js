// Bereich Zähler: Texte (Deutsch, Englisch) zu den gemeinsamen aus js/texts.js

import { texts } from "./view.js";

texts(
  {
    zl_more: "Liste ▾",
    zl_csv: "Als CSV exportieren",
    zl_history: "Verlauf",
    zl_exchange: "Zähler getauscht …",
    zl_swap: "Tausch erkannt",
    zl_swap_title: (o, n) => `Unter dieser Adresse antwortet ein anderer Zähler: ${o} → ${n}. Zählerstände laufen weiter; bitte prüfen und bestätigen.`,
    zl_swap_detail: (time, o, n) => `Zählertausch erkannt ${time}: ID ${o} → ${n}. Zählerstände laufen weiter. Stimmt das?`,
    zl_swap_confirm: "Bestätigen",
    zl_swap_confirmed: "Zählertausch bestätigt",
    zl_exchange_addr: "Adresse des neuen Zählers",
    zl_exchange_go: "Tauschen",
    zl_paste: "Aus Excel kopierte Zellen hier einfügen, oder:",
    zl_file: "Datei wählen (CSV)",
    zl_names_scope: (n) => (n ? `für die ${n} ausgewählten Zähler` : "für alle Zähler der Linie"),
    zl_close: "Schliessen",
    zl_key_history: "Verlauf zeigen",
  },
  {
    zl_more: "List ▾",
    zl_csv: "Export as CSV",
    zl_history: "History",
    zl_exchange: "Meter replaced …",
    zl_swap: "replacement detected",
    zl_swap_title: (o, n) => `A different meter answers at this address: ${o} → ${n}. Readings continue; please check and confirm.`,
    zl_swap_detail: (time, o, n) => `Meter replacement detected ${time}: ID ${o} → ${n}. Readings continue. Is that right?`,
    zl_swap_confirm: "Confirm",
    zl_swap_confirmed: "Meter replacement confirmed",
    zl_exchange_addr: "Address of the new meter",
    zl_exchange_go: "Replace",
    zl_paste: "Paste cells copied from Excel here, or:",
    zl_file: "Choose file (CSV)",
    zl_names_scope: (n) => (n ? `for the ${n} selected meters` : "for all meters of the line"),
    zl_close: "Close",
    zl_key_history: "Show history",
  }
);
