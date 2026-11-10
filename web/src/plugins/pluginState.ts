import type { Plugin } from "../api/types";

export type PluginStateName = "installed" | "enabled" | "pinned" | "needs approval";

const WAITING = /^update to (\S+) needs approval$/;

export function pendingUpdateSha(plugin: Plugin): string | null {
  return plugin.error === null ? null : (WAITING.exec(plugin.error)?.[1] ?? null);
}

export function pluginStates(plugin: Plugin): readonly PluginStateName[] {
  return [
    plugin.enabled ? "enabled" : "installed",
    ...(plugin.pinned ? (["pinned"] as const) : []),
    ...(pendingUpdateSha(plugin) === null ? [] : (["needs approval"] as const)),
  ];
}
