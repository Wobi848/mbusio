"use strict";

// Theme vor dem ersten Zeichnen setzen, sonst blitzt kurz das falsche auf. Gesetzt wird immer
// hell oder dunkel, „auto“ folgt dem System; so stehen die dunklen Farben nur einmal im CSS.
// Im <head> laden.
const darkQuery = matchMedia("(prefers-color-scheme: dark)");
let themeChoice = "auto";
try {
  themeChoice = localStorage.getItem("theme") || "auto";
} catch {
  /* privater Modus */
}
function applyTheme() {
  const t = themeChoice === "light" || themeChoice === "dark" ? themeChoice : darkQuery.matches ? "dark" : "light";
  document.documentElement.dataset.theme = t;
}
applyTheme();
darkQuery.addEventListener("change", applyTheme);
