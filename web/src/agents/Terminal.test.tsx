import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal, TERMINAL_CELL_PX, TERMINAL_LINE_HEIGHT, TERMINAL_ROW_PX } from "./Terminal";

interface FakeTerm {
  options: Record<string, unknown>;
  host: HTMLElement | null;
  writes: Uint8Array[];
  cols: number;
  rows: number;
  focus: ReturnType<typeof vi.fn>;
  reset: ReturnType<typeof vi.fn>;
  data: ((d: string) => void) | null;
  binary: ((d: string) => void) | null;
  disposed: boolean;
}

class FakeSocket {
  static readonly OPEN = 1;
  static all: FakeSocket[] = [];
  readyState = 0;
  binaryType = "";
  sent: (string | Uint8Array)[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: ((e: { reason: string }) => void) | null = null;
  closed = false;
  constructor(readonly url: string) {
    FakeSocket.all.push(this);
  }
  send(data: string | Uint8Array) {
    this.sent.push(data);
  }
  close() {
    this.closed = true;
  }
  open() {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.();
  }
  lose(reason = "") {
    this.readyState = 3;
    this.onclose?.({ reason });
  }
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
    writes: Uint8Array[] = [];
    cols = 140;
    rows = 40;
    focus = vi.fn();
    reset = vi.fn();
    data: ((d: string) => void) | null = null;
    binary: ((d: string) => void) | null = null;
    disposed = false;
    constructor(options: Record<string, unknown>) {
      this.options = { ...options };
      mocks.terms.push(this);
    }
    loadAddon() {}
    open(host: HTMLElement) {
      this.host = host;
    }
    onData(cb: (d: string) => void) {
      this.data = cb;
    }
    onBinary(cb: (d: string) => void) {
      this.binary = cb;
    }
    write(bytes: Uint8Array) {
      this.writes.push(bytes);
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
  FakeSocket.all = [];
  vi.stubGlobal("WebSocket", FakeSocket);
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

describe("terminal row height", () => {
  it("gives 18.2px rows, 13px text at 1.4 like the preview", () => {
    expect(TERMINAL_ROW_PX).toBeCloseTo(13 * 1.4, 5);
  });

  it("scales xterm's measured 17px cell to the row height", () => {
    expect(TERMINAL_CELL_PX).toBe(17);
    expect(TERMINAL_LINE_HEIGHT).toBeCloseTo(1.0706, 4);
    expect(TERMINAL_CELL_PX * TERMINAL_LINE_HEIGHT).toBeCloseTo(18.2, 5);
  });
});

const socket = (): FakeSocket => {
  const ws = FakeSocket.all.at(-1);
  if (!ws) throw new Error("no socket");
  return ws;
};
const text = (bytes: string | Uint8Array | undefined) =>
  typeof bytes === "string" ? bytes : new TextDecoder().decode(bytes);
const live = async (props: Partial<Parameters<typeof Terminal>[0]> = {}) => {
  const view = render(<Terminal name="u-demo" alive {...props} />);
  const term = await mounted();
  await waitFor(() => expect(FakeSocket.all).toHaveLength(1));
  return { ...view, term };
};

describe("Terminal", () => {
  it("opens a live xterm with the token theme, font and metrics", async () => {
    const { container, term } = await live();
    expect(term.options).toMatchObject({
      disableStdin: false,
      fontFamily: '"JetBrains Mono",monospace',
      fontSize: 13,
      lineHeight: TERMINAL_LINE_HEIGHT,
      theme: { background: "#0b0b0c", red: "#ff6b6b" },
    });
    expect(term.host).toBe(container.querySelector(".th-term-host"));
    expect(mocks.fit).toHaveBeenCalled();
  });

  it("attaches to the agent and reports its size once the socket opens", async () => {
    await live();
    expect(socket().url).toMatch(/\/api\/agents\/u-demo\/attach$/);
    expect(socket().binaryType).toBe("arraybuffer");
    socket().open();
    expect(socket().sent.map(text)).toEqual(['{"type":"resize","cols":140,"rows":40}']);
  });

  it("writes binary output into the terminal", async () => {
    const { term } = await live();
    socket().open();
    socket().onmessage?.({ data: new Uint8Array([27, 91, 49, 109, 104, 105]).buffer });
    expect(term.writes).toEqual([new Uint8Array([27, 91, 49, 109, 104, 105])]);
  });

  it("sends typed input as binary frames", async () => {
    const { term } = await live();
    socket().open();
    socket().sent.length = 0;
    term.data?.("ls\r");
    term.data?.("\u00e5");
    expect(socket().sent.every((s) => typeof s !== "string")).toBe(true);
    expect(socket().sent.map(text)).toEqual(["ls\r", "\u00e5"]);
    term.binary?.("\x1b[M\xff");
    expect(Array.from(socket().sent.at(-1) as Uint8Array)).toEqual([27, 91, 77, 255]);
  });

  it("sends a resize frame after every fit", async () => {
    const { term } = await live();
    socket().open();
    socket().sent.length = 0;
    term.cols = 100;
    term.rows = 30;
    mocks.resize.callback?.();
    expect(socket().sent.map(text)).toEqual(['{"type":"resize","cols":100,"rows":30}']);
  });

  it("does not send input before the socket is open", async () => {
    const { term } = await live();
    term.data?.("x");
    expect(socket().sent).toEqual([]);
  });

  it("clears the screen on each open so a reconnect starts clean", async () => {
    const { term } = await live();
    socket().open();
    expect(term.reset).toHaveBeenCalledTimes(1);
  });

  it("focuses on click and when the pane opens with focus", async () => {
    const { container, term } = await live({ focused: true });
    expect(term.focus).toHaveBeenCalled();
    term.focus.mockClear();
    fireEvent.mouseDown(container.querySelector(".th-term-host") as HTMLElement);
    expect(term.focus).toHaveBeenCalledTimes(1);
  });

  it("does not steal focus when the pane is not focused", async () => {
    const { term } = await live();
    socket().open();
    expect(term.focus).not.toHaveBeenCalled();
  });

  it("tells the pane about each connection state", async () => {
    const onState = vi.fn();
    await live({ onState });
    expect(onState).toHaveBeenLastCalledWith({ status: "connecting", error: null });
    socket().open();
    expect(onState).toHaveBeenLastCalledWith({ status: "open", error: null });
  });

  it("shows a state line and reconnects with backoff while the agent is alive", async () => {
    try {
      await live();
      vi.useFakeTimers();
      socket().open();
      act(() => socket().lose());
      expect(screen.getByRole("status")).toHaveTextContent("disconnected, reconnecting");
      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });
      expect(FakeSocket.all).toHaveLength(2);
      act(() => socket().open());
      expect(screen.queryByRole("status")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("stops for good when the daemon says the session ended", async () => {
    const onState = vi.fn();
    try {
      await live({ onState });
      vi.useFakeTimers();
      socket().open();
      act(() => socket().lose("session ended"));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(20_000);
      });
      expect(FakeSocket.all).toHaveLength(1);
      expect(onState).toHaveBeenLastCalledWith({ status: "closed", error: "session not found" });
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not reconnect once the agent is no longer alive", async () => {
    try {
      const { rerender } = await live();
      vi.useFakeTimers();
      socket().open();
      rerender(<Terminal name="u-demo" alive={false} />);
      act(() => socket().lose());
      await act(async () => {
        await vi.advanceTimersByTimeAsync(20_000);
      });
      expect(FakeSocket.all).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("follows a theme change without recreating the terminal", async () => {
    render(<Terminal name="u-demo" alive />);
    const term = await mounted();
    await act(async () => {
      document.documentElement.dataset.theme = "light";
    });
    expect(term.options.theme).toMatchObject({ background: "#fbfbf8", red: "#c02b2b" });
    expect(mocks.terms).toHaveLength(1);
  });

  it("refits on resize and logs a failed fit", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    await live();
    socket().open();
    socket().sent.length = 0;
    mocks.fit.mockImplementation(() => {
      throw new Error("no size");
    });
    mocks.resize.callback?.();
    expect(error).toHaveBeenCalledWith("terminal fit failed", expect.any(Error));
    expect(socket().sent).toEqual([]);
  });

  it("closes only its socket and disposes xterm and the observer on unmount", async () => {
    const { unmount, term } = await live();
    socket().open();
    const ws = socket();
    unmount();
    expect(ws.closed).toBe(true);
    expect(term.disposed).toBe(true);
    expect(mocks.resize.disconnect).toHaveBeenCalled();
  });

  it("waits for the font before measuring", async () => {
    const load = vi.fn().mockResolvedValue([]);
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    render(<Terminal name="u-demo" alive />);
    await mounted();
    expect(load).toHaveBeenCalledWith('13px "JetBrains Mono",monospace');
  });

  it("still opens when the font fails to load", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const load = vi.fn().mockRejectedValue(new Error("offline"));
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    render(<Terminal name="u-demo" alive />);
    await mounted();
    expect(warn).toHaveBeenCalledWith("terminal font not loaded", expect.any(Error));
  });

  it("does not open when unmounted before the font is ready", async () => {
    let release: () => void = () => {};
    const load = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    Object.defineProperty(document, "fonts", { value: { load }, configurable: true });
    const { unmount } = render(<Terminal name="u-demo" alive />);
    unmount();
    await act(async () => release());
    expect(mocks.terms).toHaveLength(0);
  });
});
