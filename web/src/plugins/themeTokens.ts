import tokensCss from "../styles/tokens.css?raw";
import { getThemeState, subscribeTheme } from "../theme/theme";
import type { ThemePayload } from "./bridge";

export const THEME_TOKEN_NAMES: readonly string[] = [...new Set(tokensCss.match(/--th-[\w-]+/g) ?? [])];

export function readThemePayload(root: HTMLElement = document.documentElement): ThemePayload {
  const style = getComputedStyle(root);
  const tokens: Record<string, string> = {};
  for (const name of THEME_TOKEN_NAMES) {
    const value = style.getPropertyValue(name).trim();
    if (value !== "") tokens[name] = value;
  }
  const attribute = root.getAttribute("data-theme");
  return { resolved: attribute === "light" || attribute === "dark" ? attribute : getThemeState().resolved, tokens };
}

export { subscribeTheme };
