import { httpBase } from "../api/base";

type Level = "warn" | "error";

export interface ClientLog {
  level: Level;
  event: string;
  message: string;
  stack?: string;
  route?: string;
  request_id?: string;
  error_kind?: string;
  status?: number;
}

export type LogExtras = Partial<Pick<ClientLog, "request_id" | "error_kind" | "status">>;

const DEBOUNCE_MS = 1000;
const MAX_BATCH = 20;
const MAX_MESSAGE = 500;
const MAX_STACK = 2000;
const MAX_QUEUE = 100;

let queue: ClientLog[] = [];
let timer: ReturnType<typeof setTimeout> | null = null;

const cut = (s: string, max: number) => (s.length > max ? s.slice(0, max) : s);

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function currentRoute(): string | undefined {
  try {
    return window.location.pathname;
  } catch {
    return undefined;
  }
}

function send(batch: ClientLog[]): void {
  try {
    fetch(`${httpBase()}/logs/client`, {
      method: "POST",
      keepalive: true,
      headers: { "content-type": "application/json" },
      body: JSON.stringify(batch),
    }).catch(() => {});
  } catch {
    return;
  }
}

export function flushReports(): void {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  while (queue.length > 0) send(queue.splice(0, MAX_BATCH));
}

function enqueue(entry: ClientLog): void {
  if (queue.length >= MAX_QUEUE) queue.shift();
  queue.push(entry);
  if (timer === null) timer = setTimeout(flushReports, DEBOUNCE_MS);
}

function report(level: Level, event: string, message: string, error?: unknown, extras?: LogExtras): void {
  const entry: ClientLog = { level, event, message: cut(message, MAX_MESSAGE), route: currentRoute(), ...extras };
  if (error !== undefined) {
    entry.message = cut(`${message}: ${errorMessage(error)}`, MAX_MESSAGE);
    if (error instanceof Error && error.stack) entry.stack = cut(error.stack, MAX_STACK);
  }
  enqueue(entry);
}

export function logWarn(event: string, message: string, error?: unknown, extras?: LogExtras): void {
  if (error === undefined) console.warn(message);
  else console.warn(message, error);
  report("warn", event, message, error, extras);
}

export function logError(event: string, message: string, error?: unknown, extras?: LogExtras): void {
  if (error === undefined) console.error(message);
  else console.error(message, error);
  report("error", event, message, error, extras);
}

export function reportQuiet(level: Level, event: string, message: string, extras?: LogExtras): void {
  report(level, event, message, undefined, extras);
}

export function installGlobalHandlers(win: Window = window): () => void {
  const onError = (ev: Event) => {
    const { message, error } = ev as ErrorEvent;
    report("error", "web.error", message || errorMessage(error), undefined);
    const last = queue[queue.length - 1];
    if (last && error instanceof Error && error.stack) last.stack = cut(error.stack, MAX_STACK);
  };
  const onRejection = (ev: Event) => {
    const reason = (ev as PromiseRejectionEvent).reason;
    report("error", "web.unhandled_rejection", errorMessage(reason), undefined);
    const last = queue[queue.length - 1];
    if (last && reason instanceof Error && reason.stack) last.stack = cut(reason.stack, MAX_STACK);
  };
  win.addEventListener("error", onError);
  win.addEventListener("unhandledrejection", onRejection);
  return () => {
    win.removeEventListener("error", onError);
    win.removeEventListener("unhandledrejection", onRejection);
  };
}

export function resetReportsForTests(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  queue = [];
}
