import { useEffect, useRef } from "react";
import { Terminal as XTerm, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { readTerminalFont, useTerminalTheme } from "./terminalTheme";

const CLEAR = "\x1b[2J\x1b[3J\x1b[H";
const FONT_SIZE = 13;
const LINE_HEIGHT = 1.4;
const SCROLLBACK = 2000;

export function toTerminalText(snapshot: string): string {
  return CLEAR + snapshot.replace(/\r?\n/g, "\r\n");
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
  readonly dispose: () => void;
}

function mountTerminal(host: HTMLElement, theme: ITheme, fontFamily: string): MountedTerminal {
  const term = new XTerm({
    disableStdin: true,
    convertEol: false,
    scrollback: SCROLLBACK,
    fontFamily: fontFamily || undefined,
    fontSize: FONT_SIZE,
    lineHeight: LINE_HEIGHT,
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
    }
  };
  refit();
  const observer = new ResizeObserver(refit);
  observer.observe(host);
  return {
    term,
    dispose: () => {
      observer.disconnect();
      term.dispose();
    },
  };
}

export function Terminal({ snapshot }: { snapshot: string }) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<XTerm | null>(null);
  const theme = useTerminalTheme();
  const latest = useRef({ snapshot, theme });
  latest.current = { snapshot, theme };

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const fontFamily = readTerminalFont();
    let cancelled = false;
    let mounted: MountedTerminal | null = null;
    void whenFontReady(fontFamily).then(() => {
      if (cancelled) return;
      mounted = mountTerminal(el, latest.current.theme, fontFamily);
      mounted.term.write(toTerminalText(latest.current.snapshot));
      term.current = mounted.term;
    });
    return () => {
      cancelled = true;
      mounted?.dispose();
      term.current = null;
    };
  }, []);

  useEffect(() => {
    term.current?.write(toTerminalText(snapshot));
  }, [snapshot]);

  useEffect(() => {
    if (term.current) term.current.options.theme = theme;
  }, [theme]);

  return <div className="th-term-host" ref={host} />;
}
