import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const CLEAR = "\x1b[2J\x1b[3J\x1b[H";

/** Renders full-pane snapshots: clear, then rewrite. */
export function toTerminalText(snapshot: string): string {
  return CLEAR + snapshot.replace(/\r?\n/g, "\r\n");
}

export function Terminal({ snapshot }: { snapshot: string }) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<XTerm | null>(null);
  const latest = useRef(snapshot);
  latest.current = snapshot;

  useEffect(() => {
    if (!host.current) return;
    const t = new XTerm({
      disableStdin: true,
      convertEol: false,
      scrollback: 2000,
      fontSize: 13,
      theme: { background: "#0b0d10", foreground: "#d7dae0" },
    });
    const fit = new FitAddon();
    t.loadAddon(fit);
    t.open(host.current);
    term.current = t;
    const doFit = () => {
      try {
        fit.fit();
      } catch (e) {
        console.error("terminal fit failed", e);
      }
    };
    doFit();
    t.write(toTerminalText(latest.current));
    const obs = new ResizeObserver(doFit);
    obs.observe(host.current);
    return () => {
      obs.disconnect();
      t.dispose();
      term.current = null;
    };
  }, []);

  useEffect(() => {
    term.current?.write(toTerminalText(snapshot));
  }, [snapshot]);

  return <div className="terminal" ref={host} aria-label="Agent terminal" />;
}
