import { useEffect, useState } from "react";
import { listPluginUi } from "../api/plugins";
import type { PluginUiEntry } from "../api/types";
import { logWarn } from "../log/report";

export type ListUi = () => Promise<readonly PluginUiEntry[]>;

export const listEnabledUi: ListUi = () => listPluginUi();

export function useUiEntries(slot: string, listUi: ListUi = listEnabledUi): readonly PluginUiEntry[] {
  const [entries, setEntries] = useState<readonly PluginUiEntry[]>([]);

  useEffect(() => {
    let live = true;
    listUi().then(
      (found) => {
        if (live) setEntries(found.filter((e) => e.slot === slot));
      },
      (error: unknown) => logWarn("web.plugin_ui_list", "plugin ui entries not loaded", error),
    );
    return () => {
      live = false;
    };
  }, [listUi, slot]);

  return entries;
}
