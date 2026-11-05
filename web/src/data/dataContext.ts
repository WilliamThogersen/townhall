import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from "react";
import type { Resource, DataState } from "./state";
import type { DataStore } from "./store";
import type { ConnectionStatus } from "./types";

export const DataContext = createContext<DataStore | null>(null);

export function useDataStore(): DataStore {
  const store = useContext(DataContext);
  if (!store) throw new Error("data hooks need a DataProvider");
  return store;
}

export function useDataState<T>(select: (state: DataState) => T): T {
  const store = useDataStore();
  const snapshot = () => select(store.getState());
  return useSyncExternalStore(store.subscribe, snapshot, snapshot);
}

export function useConnection(): ConnectionStatus {
  return useDataState((state) => state.connection);
}

export function useGeneration(): number {
  return useDataState((state) => state.generation);
}

export interface ResourceView<T> {
  readonly data: T | null;
  readonly loading: boolean;
  readonly error: string | null;
}

export function useResource<T>(
  key: string | null,
  read: (state: DataState) => Resource<T> | null | undefined,
): ResourceView<T> {
  const store = useDataStore();
  const resource = useDataState((state) => (key === null ? null : (read(state) ?? null)));
  useEffect(() => {
    if (key !== null) store.ensure(key);
  }, [store, key]);
  return useMemo(
    () => ({
      data: resource?.data ?? null,
      loading: key !== null && resource?.settled !== true,
      error: resource?.error ?? null,
    }),
    [key, resource],
  );
}
