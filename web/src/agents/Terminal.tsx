import { useEffect, useRef, useState } from "react";
import { Terminal as XTerm, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { connectAttach, type AttachConnection, type AttachState } from "./attachSocket";
import { readTerminalFont, useTerminalTheme } from "./terminalTheme";

const FONT_SIZE = 13;
const DESIGN_LINE_HEIGHT = 1.4;
const SCROLLBACK = 5000;

export const TERMINAL_ROW_PX = FONT_SIZE * DESIGN_LINE_HEIGHT;
export const TERMINAL_CELL_PX = 17;
export const TERMINAL_LINE_HEIGHT = TERMINAL_ROW_PX / TERMINAL_CELL_PX;

const encoder = new TextEncoder();

/** The bytes xterm reports for `onBinary` (a string with one byte per character). */
function binaryBytes(data: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff);
}

async function whenFontReady(fontFamily: string): Promise<void> {
  if (!("fonts" in document) || fontFamily === "") return;
  try {
    await document.fonts.load(`${FONT_SIZE}px ${fontFamily}`);
  } catch (e) {
    console.warn("terminal font not loaded", e);
  }
}

interface MountedTerminal {
  readonly term: XTerm;
  /** Fits to the host, then reports the new size to `onResize`. */
  readonly refit: () => void;
  readonly dispose: () => void;
}

function mountTerminal(
  host: HTMLElement,
  theme: ITheme,
  fontFamily: string,
  onResize: (cols: number, rows: number) => void,
): MountedTerminal {
  const term = new XTerm({
    disableStdin: false,
    cursorBlink: true,
    convertEol: false,
    scrollback: SCROLLBACK,
    fontFamily: fontFamily || undefined,
    fontSize: FONT_SIZE,
    lineHeight: TERMINAL_LINE_HEIGHT,
    theme,
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(host);
  const refit = () => {
    try {
      fit.fit();
    } catch (e) {
      console.error("terminal fit failed", e);
      return;
    }
    onResize(term.cols, term.rows);
  };
  const observer = new ResizeObserver(refit);
  observer.observe(host);
  return {
    term,
    refit,
    dispose: () => {
      observer.disconnect();
      term.dispose();
    },
  };
}

export interface TerminalProps {
  readonly name: string;
  /** While false, a lost connection is not retried. */
  readonly alive: boolean;
  readonly focused?: boolean;
  readonly onState?: (state: AttachState) => void;
}

/** A live, interactive terminal attached to the agent's tmux session. */
export function Terminal({ name, alive, focused = false, onState }: TerminalProps) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<XTerm | null>(null);
  const theme = useTerminalTheme();
  const [lost, setLost] = useState<AttachState | null>(null);
  const latest = useRef({ theme, alive, focused, onState });
  latest.current = { theme, alive, focused, onState };

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = readTerminalFont();
    let cancelled = false;
    let mounted: MountedTerminal | null = null;
    let connection: AttachConnection | null = null;
    void whenFontReady(fontFamily).then(() => {
      if (cancelled) return;
      const m = mountTerminal(el, latest.current.theme, fontFamily, (cols, rows) =>
        connection?.resize(cols, rows),
      );
      mounted = m;
      term.current = m.term;
      m.term.onData((data) => connection?.send(encoder.encode(data)));
      m.term.onBinary((data) => connection?.send(binaryBytes(data)));
      m.refit();
      if (latest.current.focused) m.term.focus();
      connection = connectAttach(name, {
        onState: (state) => {
          setLost(state.status === "open" ? null : state);
          latest.current.onState?.(state);
        },
        onOpen: () => {
          m.term.reset();
          m.refit();
          if (latest.current.focused) m.term.focus();
        },
        onOutput: (bytes) => m.term.write(bytes),
        shouldRetry: () => latest.current.alive,
      });
    });
    return () => {
      cancelled = true;
      connection?.close();
      mounted?.dispose();
      term.current = null;
    };
  }, [name]);

  useEffect(() => {
    if (term.current) term.current.options.theme = theme;
  }, [theme]);

  return (
    <div className="th-term-wrap" onMouseDown={() => term.current?.focus()}>
      <div className="th-term-host" ref={host} />
      {lost?.status === "reconnecting" && (
        <div className="th-term-state" role="status">
          disconnected, reconnecting
        </div>
      )}
    </div>
  );
}
