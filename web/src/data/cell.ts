import type { DataState } from "./state";

export type Update = (change: (state: DataState) => DataState) => void;

export interface Cell {
  readonly get: () => DataState;
  readonly update: Update;
  readonly subscribe: (listener: () => void) => () => void;
}

export function createCell(initial: DataState): Cell {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => state,
    update: (change) => {
      const next = change(state);
      if (next === state) return;
      state = next;
      listeners.forEach((listener) => listener());
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
