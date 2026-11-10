import { useEffect } from "react";
import { useContributions } from "../data";

export const PLUGIN_THEME_ATTRIBUTE = "data-th-plugin-theme";

export function applyPluginTheme(css: string | null): void {
  const current = document.head.querySelector(`style[${PLUGIN_THEME_ATTRIBUTE}]`);
  if (css === null) {
    current?.remove();
    return;
  }
  const style = current ?? document.createElement("style");
  if (style.textContent !== css) style.textContent = css;
  if (current === null) style.setAttribute(PLUGIN_THEME_ATTRIBUTE, "");
  if (document.head.lastElementChild !== style) document.head.append(style);
}

export function PluginTheme() {
  const { theme } = useContributions();
  const css = theme?.css ?? null;
  useEffect(() => {
    applyPluginTheme(css);
    return () => applyPluginTheme(null);
  }, [css]);
  return null;
}
