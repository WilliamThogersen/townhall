type Listener = () => void;

export interface NavigateOptions {
  readonly replace?: boolean;
}

const listeners = new Set<Listener>();

function notify(): void {
  for (const listener of [...listeners]) listener();
}

export function currentUrl(): string {
  return `${window.location.pathname}${window.location.search}`;
}

export function subscribe(listener: Listener): () => void {
  if (listeners.size === 0) window.addEventListener("popstate", notify);
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) window.removeEventListener("popstate", notify);
  };
}

export function navigate(to: string, options: NavigateOptions = {}): void {
  const next = new URL(to, window.location.href);
  const now = window.location;
  if (next.pathname === now.pathname && next.search === now.search && next.hash === now.hash) {
    return;
  }
  if (options.replace) window.history.replaceState(null, "", next);
  else window.history.pushState(null, "", next);
  notify();
}
