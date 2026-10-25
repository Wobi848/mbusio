"use strict";

// Symbole und Zähler-Kennung, gemeinsam für die bisherige und die neue Oberfläche (und den Bericht)

/// Symbole (Strich-SVG, Farbe vom Text)
const svg = (d, size = 15, stroke = 2) =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON_EDIT = svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>');
const ICON_DEL = svg('<path d="M6 6l12 12M18 6L6 18"/>');
const ICON_UP = svg('<path d="M6 15l6-6 6 6"/>');
const ICON_DOWN = svg('<path d="M6 9l6 6 6-6"/>');
const ICON_CHART = svg('<path d="M4 19h16M5 15l4-5 4 3 6-7"/>');
const ICON_ALERT = svg('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16.5v.01"/>');
const ICON_CLOCK = svg('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>');
const ICON_CHECK = svg('<path d="M5 12l5 5L20 7"/>', 11, 3.2);
const ICON_NEXT = svg('<path d="M5 12h14M13 6l6 6-6 6"/>', 14);

/// Medium-Schlüssel vom Server → Art des Symbols
const MEDIUM_KIND = {
  medium_heat_outlet: "heat",
  medium_heat_inlet: "heat",
  medium_heat_cooling: "heat",
  medium_steam: "heat",
  medium_hca: "heat",
  medium_cooling_inlet: "cold",
  medium_water: "water",
  medium_cold_water: "water",
  medium_dual_water: "water",
  medium_warm_water: "warm",
  medium_hot_water: "warm",
  medium_electricity: "power",
  medium_gas: "gas",
};
const MEDIUM_ICON = {
  heat: '<path d="M12 3c1 3 5 5 5 10a5 5 0 0 1-10 0c0-3 2-4 2-7 1.5 1 3 2.5 3 4.5"/>',
  cold: '<path d="M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9"/>',
  water: '<path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"/>',
  warm: '<path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"/><path d="M9 15c1-1 2 1 3 0s2 1 3 0"/>',
  power: '<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>',
  gas: '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
  other: '<rect x="5" y="5" width="14" height="14" rx="3"/><path d="M9 12h6"/>',
};

/// Medium-Symbol (abgerundetes Quadrat, Farbe je Medium, Stil in css/meter.css); small = 22 px statt 34 px
function mediumIcon(medium, small = false) {
  const kind = MEDIUM_KIND[medium] || "other";
  const size = small ? 12 : 18;
  return `<span class="mid m-${kind}${small ? " s" : ""}" aria-hidden="true"><svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${
    small ? 2.2 : 2
  }" stroke-linecap="round" stroke-linejoin="round">${MEDIUM_ICON[kind]}</svg></span>`;
}

/// Zähler der Liste für den Zähler-Baustein: ID (über PA gemerkt: die ID aus der Auslesung), PA,
/// Hersteller, eigene Baudrate
function meterBlock(m) {
  const viaPa = /^\d+$/.test(m.address);
  return {
    name: m.name,
    ident: viaPa ? m.meter_id : m.meter_id || m.address,
    pa: viaPa ? m.address : m.primary_address,
    man: m.manufacturer,
    medium: m.medium,
    info: m.baud ? `${m.baud} Bd` : "",
  };
}
