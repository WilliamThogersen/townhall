import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readTerminalFont, readTerminalTheme, useTerminalTheme } from "./terminalTheme";

const TOKENS = `
:root {
  --th-term-bg: #0b0b0c;
  --th-term-fg: #e8e6e1;
  --th-term-cursor: #ffb238;
  --th-term-selection: rgba(255,178,56,0.28);
  --th-ansi-black: #26262a;
  --th-ansi-red: #ff6b6b;
  --th-ansi-green: #5fd38d;
  --th-ansi-yellow: #ffb238;
  --th-ansi-blue: #6cb6ff;
  --th-ansi-magenta: #d79bff;
  --th-ansi-cyan: #5fd7d7;
  --th-ansi-white: #c8c6c0;
  --th-ansi-bright-black: #6a6a72;
  --th-ansi-bright-red: #ff9090;
  --th-ansi-bright-green: #8ae6ad;
  --th-ansi-bright-yellow: #ffc870;
  --th-ansi-bright-blue: #9acdff;
  --th-ansi-bright-magenta: #e6bdff;
  --th-ansi-bright-cyan: #8ae6e6;
  --th-ansi-bright-white: #f6f4ef;
  --th-font: "JetBrains Mono",monospace;
}
:root[data-theme="light"] {
  --th-term-bg: #fbfbf8;
  --th-ansi-white: #6b6a64;
}
`;

let style: HTMLStyleElement;

beforeEach(() => {
  style = document.createElement("style");
  style.textContent = TOKENS;
  document.head.append(style);
  document.documentElement.dataset.theme = "dark";
});

afterEach(() => {
  style.remove();
  delete document.documentElement.dataset.theme;
  vi.restoreAllMocks();
});

const flushMutations = () => act(() => Promise.resolve());

describe("readTerminalTheme", () => {
  it("maps every token to its xterm theme key", () => {
    expect(readTerminalTheme()).toEqual({
      background: "#0b0b0c",
      foreground: "#e8e6e1",
      cursor: "#ffb238",
      selectionBackground: "rgba(255,178,56,0.28)",
      black: "#26262a",
      red: "#ff6b6b",
      green: "#5fd38d",
      yellow: "#ffb238",
      blue: "#6cb6ff",
      magenta: "#d79bff",
      cyan: "#5fd7d7",
      white: "#c8c6c0",
      brightBlack: "#6a6a72",
      brightRed: "#ff9090",
      brightGreen: "#8ae6ad",
      brightYellow: "#ffc870",
      brightBlue: "#9acdff",
      brightMagenta: "#e6bdff",
      brightCyan: "#8ae6e6",
      brightWhite: "#f6f4ef",
    });
  });

  it("follows the light overrides", () => {
    document.documentElement.dataset.theme = "light";
    const theme = readTerminalTheme();
    expect(theme.background).toBe("#fbfbf8");
    expect(theme.white).toBe("#6b6a64");
    expect(theme.foreground).toBe("#e8e6e1");
  });

  it("leaves out tokens that are not defined", () => {
    style.remove();
    expect(readTerminalTheme()).toEqual({});
  });
});

describe("readTerminalFont", () => {
  it("reads the font token", () => {
    expect(readTerminalFont()).toBe('"JetBrains Mono",monospace');
  });
});

describe("useTerminalTheme", () => {
  it("returns the theme read at mount", () => {
    const { result } = renderHook(() => useTerminalTheme());
    expect(result.current.background).toBe("#0b0b0c");
  });

  it("re-reads the tokens when data-theme changes", async () => {
    const { result } = renderHook(() => useTerminalTheme());
    document.documentElement.dataset.theme = "light";
    await flushMutations();
    expect(result.current.background).toBe("#fbfbf8");
    document.documentElement.dataset.theme = "dark";
    await flushMutations();
    expect(result.current.background).toBe("#0b0b0c");
  });

  it("ignores other attribute changes", async () => {
    const { result } = renderHook(() => useTerminalTheme());
    const first = result.current;
    document.documentElement.setAttribute("lang", "en");
    await flushMutations();
    expect(result.current).toBe(first);
    document.documentElement.removeAttribute("lang");
  });

  it("stops observing on unmount", async () => {
    const disconnect = vi.spyOn(MutationObserver.prototype, "disconnect");
    const { result, unmount } = renderHook(() => useTerminalTheme());
    unmount();
    expect(disconnect).toHaveBeenCalledTimes(1);
    document.documentElement.dataset.theme = "light";
    await flushMutations();
    expect(result.current.background).toBe("#0b0b0c");
  });
});
