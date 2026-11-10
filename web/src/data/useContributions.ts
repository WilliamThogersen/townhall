import { useMemo } from "react";
import { useDataStore, useResource } from "./dataContext";
import { CONTRIBUTIONS_KEY } from "./resources";
import type { ContributionsState } from "./types";

const NONE = { starters: [], themes: [], theme: null, settings: [] } as const;

export function useContributions(): ContributionsState {
  const store = useDataStore();
  const { data, loading, error } = useResource(CONTRIBUTIONS_KEY, (state) => state.contributions);
  return useMemo(
    () => ({
      ...(data ?? NONE),
      loading,
      error,
      chooseTheme: store.chooseTheme,
      setSetting: store.setPluginSetting,
      resetSetting: store.resetPluginSetting,
    }),
    [data, loading, error, store],
  );
}
