export type AppearanceTheme = "system" | "light" | "dark" | "high-contrast";

const storageKey = "nirmata.appearance.theme";
const themes = new Set<AppearanceTheme>(["system", "light", "dark", "high-contrast"]);

export function readAppearanceTheme(): AppearanceTheme {
  try {
    const value = localStorage.getItem(storageKey) as AppearanceTheme | null;
    return value && themes.has(value) ? value : "system";
  } catch {
    return "system";
  }
}

export function applyAppearanceTheme(theme: AppearanceTheme): void {
  document.documentElement.setAttribute("data-theme", theme);
  document.querySelector("#closed-root")?.setAttribute("data-theme", theme);
  try {
    localStorage.setItem(storageKey, theme);
  } catch {
    // Storage can be unavailable in hardened webviews; the active theme still applies.
  }
}
