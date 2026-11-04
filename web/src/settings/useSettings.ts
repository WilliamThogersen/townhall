import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from "react";
import type { Settings } from "./settings";
import { createSettingsStore, type SettingsProblem, type SettingsStore } from "./settingsStore";

export interface SettingsControls {
  readonly settings: Settings;
  readonly loading: boolean;
  readonly problem: SettingsProblem | null;
  readonly update: (patch: Partial<Settings>) => void;
  readonly retry: () => void;
  readonly dismissProblem: () => void;
}

export const SettingsStoreContext = createContext<SettingsStore>(createSettingsStore());

export function useSettings(): SettingsControls {
  const store = useContext(SettingsStoreContext);
  const state = useSyncExternalStore(store.subscribe, store.getState, store.getState);
  useEffect(() => store.ensureLoaded(), [store]);
  return useMemo(
    () => ({ ...state, update: store.update, retry: store.reload, dismissProblem: store.dismissProblem }),
    [state, store],
  );
}
