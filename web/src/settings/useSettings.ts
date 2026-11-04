import { useSyncExternalStore } from "react";
import type { Settings } from "./settings";
import { getSettings, subscribeSettings, updateSettings } from "./settingsStore";

export interface SettingsControls {
  readonly settings: Settings;
  readonly update: (patch: Partial<Settings>) => void;
}

export function useSettings(): SettingsControls {
  const settings = useSyncExternalStore(subscribeSettings, getSettings, getSettings);
  return { settings, update: updateSettings };
}
