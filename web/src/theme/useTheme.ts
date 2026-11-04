import { useSyncExternalStore } from "react";
import {
  getThemeState,
  saveThemeChoice,
  subscribeTheme,
  type ResolvedTheme,
  type ThemeChoice,
} from "./theme";

export interface ThemeControls {
  readonly choice: ThemeChoice;
  readonly resolved: ResolvedTheme;
  readonly setChoice: (choice: ThemeChoice) => void;
}

export function useTheme(): ThemeControls {
  const { choice, resolved } = useSyncExternalStore(subscribeTheme, getThemeState, getThemeState);
  return { choice, resolved, setChoice: saveThemeChoice };
}
