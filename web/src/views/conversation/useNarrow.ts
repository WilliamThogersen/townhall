import { useSyncExternalStore } from "react";

export const NARROW_QUERY = "(max-width: 1199px)";

function query(): MediaQueryList | null {
  return typeof window.matchMedia === "function" ? window.matchMedia(NARROW_QUERY) : null;
}

function subscribe(listener: () => void): () => void {
  const list = query();
  if (list === null) return () => undefined;
  list.addEventListener("change", listener);
  return () => list.removeEventListener("change", listener);
}

function read(): boolean {
  return query()?.matches ?? false;
}

export function useNarrow(): boolean {
  return useSyncExternalStore(subscribe, read, () => false);
}
