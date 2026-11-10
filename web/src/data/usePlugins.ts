import { useMemo } from "react";
import { useDataStore, useResource } from "./dataContext";
import { PLUGINS_KEY } from "./resources";
import type { PluginsState } from "./types";

const NONE = { marketplaces: [], available: [], installed: [] } as const;

export function usePlugins(): PluginsState {
  const store = useDataStore();
  const { data, loading, error } = useResource(PLUGINS_KEY, (state) => state.plugins);
  return useMemo(
    () => ({
      ...(data ?? NONE),
      loading,
      error,
      addMarketplace: store.addMarketplace,
      refreshMarketplace: store.refreshMarketplace,
      removeMarketplace: store.removeMarketplace,
      preview: store.previewPlugin,
      install: store.installPlugin,
      patch: store.patchPlugin,
      update: store.updatePlugin,
      uninstall: store.deletePlugin,
    }),
    [data, loading, error, store],
  );
}
