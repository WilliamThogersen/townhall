import { useEffect, useState } from "react";
import type { ITheme } from "@xterm/xterm";

const TOKEN_FOR_KEY: readonly (readonly [keyof ITheme, string])[] = [
  ["background", "term-bg"],
  ["foreground", "term-fg"],
  ["cursor", "term-cursor"],
  ["selectionBackground", "term-selection"],
  ["black", "ansi-black"],
  ["red", "ansi-red"],
  ["green", "ansi-green"],
  ["yellow", "ansi-yellow"],
  ["blue", "ansi-blue"],
  ["magenta", "ansi-magenta"],
  ["cyan", "ansi-cyan"],
  ["white", "ansi-white"],
  ["brightBlack", "ansi-bright-black"],
  ["brightRed", "ansi-bright-red"],
  ["brightGreen", "ansi-bright-green"],
  ["brightYellow", "ansi-bright-yellow"],
  ["brightBlue", "ansi-bright-blue"],
  ["brightMagenta", "ansi-bright-magenta"],
  ["brightCyan", "ansi-bright-cyan"],
  ["brightWhite", "ansi-bright-white"],
];

const rootStyle = (): CSSStyleDeclaration => getComputedStyle(document.documentElement);

const readToken = (css: CSSStyleDeclaration, name: string): string =>
  css.getPropertyValue(`--th-${name}`).trim();

export function readTerminalTheme(): ITheme {
  const css = rootStyle();
  const entries = TOKEN_FOR_KEY.map(([key, token]) => [key, readToken(css, token)] as const);
  return Object.fromEntries(entries.filter(([, value]) => value !== "")) as ITheme;
}

export function readTerminalFont(): string {
  return readToken(rootStyle(), "font");
}

export function useTerminalTheme(): ITheme {
  const [theme, setTheme] = useState<ITheme>(readTerminalTheme);

  useEffect(() => {
    const observer = new MutationObserver(() => setTheme(readTerminalTheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return theme;
}
