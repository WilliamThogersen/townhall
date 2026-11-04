import { act, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal, toTerminalText } from "./Terminal";

interface FakeTerm {
  options: Record<string, unknown>;
  host: HTMLElement | null;
  writes: string[];
  disposed: boolean;
}

const mocks = vi.hoisted(() => ({
  terms: [] as FakeTerm[],
  fit: vi.fn(),
  resize: { callback: null as (() => void) | null, disconnect: vi.fn() },
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options: Record<string, unknown>;
    host: HTMLElement | null = null;
    writes: string[] = [];
    disposed = false;
    constructor(options: Record<string, unknown>) {
      this.options = { ...options };
      mocks.terms.push(this);
    }
    loadAddon() {}
    open(host: HTMLElement) {
      this.host = host;
    }
    write(text: string) {
      this.writes.push(text);
    }
    dispose() {
      this.disposed = true;
    }
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = mocks.fit;
  },
}));

const TOKENS = `
:root { --th-term-bg: #0b0b0c; --th-ansi-red: #ff6b6b; --th-font: "JetBrains Mono",monospace; }
:root[data-theme="light"] { --th-term-bg: #fbfbf8; --th-ansi-red: #c02b2b; }
`;

let style: HTMLStyleElement;

beforeEach(() => {
  mocks.terms.length = 0;
  mocks.fit.mockReset();
  mocks.resize.disconnect.mockReset();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: () => void) {
        mocks.resize.callback = callback;
      }
      observe() {}
      disconnect = mocks.resize.disconnect;
    },
  );
  style = document.createElement("style");
  style.textContent = TOKENS;
  document.head.append(style);
  document.documentElement.dataset.theme = "dark";
});

afterEach(() => {
  style.remove();
  delete document.documentElement.dataset.theme;
  Reflect.deleteProperty(document, "fonts");
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const mounted = async (): Promise<FakeTerm> => {
  await waitFor(() => expect(mocks.terms).toHaveLength(1));
  const term = mocks.terms[0];
  if (!term) throw new Error("no terminal");
  return term;
};

describe("toTerminalText", () => {
  it("clears the screen then rewrites the snapshot with CRLF line ends", () => {
    expect(toTerminalText("a\nb\r\nc")).toBe("\x1b[2J\x1b[3J\x1b[Ha\r\nb\r\nc");
  });
});

describe("Terminal", () => {
  it("opens xterm with the token theme, font and metrics", async () => {
    const { container } = render(<Terminal snapshot="hello" />);
    const term = await mounted();
    expect(term.options).toMatchObject({
      disableStdin: true,
      fontFamily: '"JetBrains Mono",monospace',
      fontSize: 13,
      lineHeight: 1.4,
      theme: { background: "#0b0b0c", red: "#ff6b6b" },
    });
    expect(term.host).toBe(container.querySelector(".th-term-host"));
    expect(term.writes).toEqual([toTerminalText("hello")]);
    expect(mocks.fit).toHaveBeenCalled();
  });

  it("rewrites the terminal when the snapshot changes", async () => {
    const { rerender } = render(<Terminal snapshot="one" />);
    const term = await mounted();
    rerender(<Terminal snapshot="two" />);
    expect(term.writes.at(-1)).toBe(toTerminalText("two"));
  });

  it("follows a theme change without recreating the terminal", async () => {
    render(<Terminal snapshot="" />);
    const term = await mounted();
    await act(async () => {
      document.documentElement.dataset.theme = "light";
    });
    expect(term.options.theme).toMatchObject({ background: "#fbfbf8", red: "#c02b2b" });
    expect(mocks.terms).toHaveLength(1);
  });

  it("refits on resize and logs a failed fit", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<Terminal snapshot="" />);
    await mounted();
    mocks.fit.mockImplementation(() => {
      throw new Error("no size");
    });
    mocks.resize.callback?.();
    expect(error).toHaveBeenCalledWith("terminal fit failed", expect.any(Error));
  });

  it("disposes xterm and the resize observer on unmount", async () => {
    const { unmount } = render(<Terminal snapshot="" />);
    const term = await mounted();
    unmount();
    expect(term.disposed).toBe(true);
    expect(mocks.resize.disconnect).toHaveBeenCalled();
  });

  it("waits for the font before measuring", async () => {
    const load = vi.fn().mockResolvedValue([]);
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    render(<Terminal snapshot="" />);
    await mounted();
    expect(load).toHaveBeenCalledWith('13px "JetBrains Mono",monospace');
  });

  it("still opens when the font fails to load", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const load = vi.fn().mockRejectedValue(new Error("offline"));
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    render(<Terminal snapshot="" />);
    await mounted();
    expect(warn).toHaveBeenCalledWith("terminal font not loaded", expect.any(Error));
  });

  it("does not open when unmounted before the font is ready", async () => {
    let release: () => void = () => {};
    const load = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    const { unmount } = render(<Terminal snapshot="" />);
    unmount();
    await act(async () => release());
    expect(mocks.terms).toHaveLength(0);
  });
});
