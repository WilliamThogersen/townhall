import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { connectEvents as realConnectEvents, type ConnectEvents } from "../api/events";
import { listPluginUi } from "../api/plugins";
import type { PluginUiEntry } from "../api/types";
import { logWarn } from "../log/report";
import { navigate as realNavigate } from "../router";
import { useToasts } from "../ui/Toasts";
import { createBridge, DEFAULT_HEIGHT_RANGE, type HostSession, type ThemePayload, type Tone } from "./bridge";
import { forgetHostSession, hostSession } from "./hostSession";
import { PluginFrame } from "./PluginFrame";
import { readThemePayload, subscribeTheme } from "./themeTokens";

export const HELLO_TIMEOUT_MS = 5000;

const SLOT_HEIGHTS: Readonly<Record<string, readonly [number, number]>> = {
  "sidebar.foot": [24, 120],
  "session.section": [24, 400],
};

const TONE_GLYPHS: Readonly<Record<Tone, string>> = { info: "i", success: "✓", warn: "!", error: "×" };

export interface SlotServices {
  readonly listUi: () => Promise<readonly PluginUiEntry[]>;
  readonly session: (plugin: string) => Promise<HostSession>;
  readonly fetch: typeof fetch;
  readonly connectEvents: ConnectEvents;
  readonly theme: () => ThemePayload;
  readonly subscribeTheme: (listener: () => void) => () => void;
  readonly navigate: (path: string) => void;
}

const REAL: SlotServices = {
  listUi: () => listPluginUi(),
  session: (plugin) => hostSession(plugin),
  fetch: (input, init) => fetch(input, init),
  connectEvents: realConnectEvents,
  theme: () => readThemePayload(),
  subscribeTheme,
  navigate: (path) => realNavigate(path),
};

export interface PluginSlotProps {
  readonly name: string;
  readonly context?: Readonly<Record<string, unknown>>;
  readonly services?: Partial<SlotServices>;
}

export function PluginSlot({ name, context, services }: PluginSlotProps) {
  const [entries, setEntries] = useState<readonly PluginUiEntry[]>([]);
  const all = useMemo<SlotServices>(() => ({ ...REAL, ...services }), [services]);
  const { listUi } = all;

  useEffect(() => {
    let live = true;
    listUi().then(
      (found) => {
        if (live) setEntries(found.filter((e) => e.slot === name));
      },
      (error: unknown) => logWarn("web.plugin_ui_list", "plugin ui entries not loaded", error),
    );
    return () => {
      live = false;
    };
  }, [listUi, name]);

  if (entries.length === 0) return null;
  return (
    <>
      {entries.map((entry) => (
        <SlotFrame key={`${entry.plugin}/${entry.entry}`} slot={name} entry={entry} context={context} services={all} />
      ))}
    </>
  );
}

type Phase =
  | { readonly kind: "loading" }
  | { readonly kind: "ready" }
  | { readonly kind: "paused" }
  | { readonly kind: "failed"; readonly why: string };

interface SlotFrameProps {
  readonly slot: string;
  readonly entry: PluginUiEntry;
  readonly context: Readonly<Record<string, unknown>> | undefined;
  readonly services: SlotServices;
}

function SlotFrame({ slot, entry, context, services }: SlotFrameProps) {
  const toasts = useToasts();
  const frame = useRef<HTMLIFrameElement>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);
  const [height, setHeight] = useState<number | undefined>(entry.height ?? undefined);
  const contextKey = JSON.stringify(context ?? {});
  const { push } = toasts;

  const showToast = useCallback(
    (text: string, tone: Tone) => push({ glyph: TONE_GLYPHS[tone], title: entry.plugin, text }),
    [push, entry.plugin],
  );

  useEffect(() => {
    const timer = window.setTimeout(
      () => setPhase({ kind: "failed", why: "no hello within 5 s" }),
      HELLO_TIMEOUT_MS,
    );
    const bridge = createBridge({
      slot,
      context: JSON.parse(contextKey) as Record<string, unknown>,
      frame: () => frame.current?.contentWindow ?? null,
      session: () => services.session(entry.plugin),
      fetch: services.fetch,
      connectEvents: services.connectEvents,
      theme: services.theme,
      subscribeTheme: services.subscribeTheme,
      heightRange: SLOT_HEIGHTS[slot] ?? DEFAULT_HEIGHT_RANGE,
      ui: {
        resize: setHeight,
        toast: showToast,
        navigate: services.navigate,
        ready: () => {
          window.clearTimeout(timer);
          setPhase({ kind: "ready" });
        },
        paused: () => {
          window.clearTimeout(timer);
          setPhase({ kind: "paused" });
        },
        failed: (why) => {
          window.clearTimeout(timer);
          setPhase({ kind: "failed", why });
        },
      },
    });
    const onMessage = (event: MessageEvent) => bridge.handle({ source: event.source, data: event.data });
    window.addEventListener("message", onMessage);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("message", onMessage);
      bridge.dispose();
    };
  }, [slot, entry.plugin, contextKey, services, showToast, attempt]);

  const again = (forget: boolean) => {
    if (forget) forgetHostSession(entry.plugin);
    setPhase({ kind: "loading" });
    setAttempt((n) => n + 1);
  };

  if (phase.kind === "paused") {
    return (
      <Notice onClick={() => again(false)} action="Resume">
        plugin {entry.plugin} was paused: it sent too many messages
      </Notice>
    );
  }
  if (phase.kind === "failed") {
    return (
      <Notice onClick={() => again(true)} action="Retry">
        plugin {entry.plugin} failed to load: {phase.why}
      </Notice>
    );
  }
  return (
    <PluginFrame
      key={attempt}
      ref={frame}
      plugin={entry.plugin}
      entry={entry.entry}
      title={entry.title}
      height={height}
    />
  );
}

function Notice({ children, action, onClick }: { children: ReactNode; action: string; onClick: () => void }) {
  return (
    <div role="alert" className="th-plugin-notice">
      {children}{" "}
      <button type="button" className="th-btn th-btn--ghost th-btn--sm" onClick={onClick}>
        {action}
      </button>
    </div>
  );
}
