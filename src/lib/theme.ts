import type { Theme } from "../generated/Theme";

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
let chosen: Theme = "system";

function paint() {
  const dark = chosen === "dark" || (chosen === "system" && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

darkQuery.addEventListener("change", paint);

/** Apply a theme. "system" follows the OS, including later changes. */
export function applyTheme(theme: Theme) {
  chosen = theme;
  paint();
}
