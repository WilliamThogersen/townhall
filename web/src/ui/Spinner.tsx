import { useSyncExternalStore } from "react";

export const SPINNER_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"] as const;
export const SPINNER_STATIC = "⠿";
export const SPINNER_FRAME_MS = 80;

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

type Listener = () => void;

interface FrameClock {
  readonly index: number;
  readonly listeners: readonly Listener[];
  readonly timer: ReturnType<typeof setInterval> | null;
}

const IDLE_CLOCK: FrameClock = { index: 0, listeners: [], timer: null };

let clock: FrameClock = IDLE_CLOCK;

function tick(): void {
  clock = { ...clock, index: (clock.index + 1) % SPINNER_FRAMES.length };
  clock.listeners.forEach((listener) => listener());
}

function subscribeFrame(listener: Listener): () => void {
  const timer = clock.timer ?? setInterval(tick, SPINNER_FRAME_MS);
  clock = { ...clock, timer, listeners: [...clock.listeners, listener] };
  return () => {
    const listeners = clock.listeners.filter((current) => current !== listener);
    if (listeners.length > 0) {
      clock = { ...clock, listeners };
      return;
    }
    if (clock.timer !== null) clearInterval(clock.timer);
    clock = IDLE_CLOCK;
  };
}

function readFrame(): string {
  return SPINNER_FRAMES[clock.index] ?? SPINNER_STATIC;
}

function subscribeStatic(): () => void {
  return () => {};
}

function readStatic(): string {
  return SPINNER_STATIC;
}

function reducedMotionQuery(): MediaQueryList | null {
  return typeof window.matchMedia === "function" ? window.matchMedia(REDUCED_MOTION_QUERY) : null;
}

function subscribeReducedMotion(listener: Listener): () => void {
  const query = reducedMotionQuery();
  if (query === null) return () => {};
  query.addEventListener("change", listener);
  return () => query.removeEventListener("change", listener);
}

function readReducedMotion(): boolean {
  return reducedMotionQuery()?.matches ?? false;
}

function useReducedMotion(): boolean {
  return useSyncExternalStore(subscribeReducedMotion, readReducedMotion, () => false);
}

export function useSpinnerFrame(): string {
  const reduced = useReducedMotion();
  return useSyncExternalStore(
    reduced ? subscribeStatic : subscribeFrame,
    reduced ? readStatic : readFrame,
    readStatic,
  );
}

export function Spinner() {
  const frame = useSpinnerFrame();
  return (
    <span className="th-spin" aria-hidden="true">
      {frame}
    </span>
  );
}
