import { parseContributions, parsePlugin, parsePluginList, parsePluginUpdate, parsePreview } from "./pluginGuards";
import { request } from "./request";
import type {
  PatchPluginInput,
  Plugin,
  PluginContributions,
  PluginList,
  PluginPreview,
  PluginUpdate,
  SettingFieldValue,
} from "./types";

const path = (id: string): string => `/plugins/${encodeURIComponent(id)}`;

export const listPlugins = (signal?: AbortSignal): Promise<PluginList> =>
  request("GET", "/plugins", parsePluginList, undefined, signal);

export const previewPlugin = (id: string, signal?: AbortSignal): Promise<PluginPreview> =>
  request("GET", `/plugins/preview?plugin=${encodeURIComponent(id)}`, parsePreview, undefined, signal);

export const installPlugin = (id: string, approve: string): Promise<Plugin> =>
  request("POST", "/plugins", parsePlugin, { plugin: id, approve });

export const patchPlugin = (id: string, input: PatchPluginInput): Promise<Plugin> =>
  request("PATCH", path(id), parsePlugin, input);

export const updatePlugin = (id: string, approve?: string): Promise<PluginUpdate> =>
  request(
    "POST",
    `${path(id)}/update${approve === undefined ? "" : `?approve=${encodeURIComponent(approve)}`}`,
    parsePluginUpdate,
  );

export const deletePlugin = (id: string, purge = false): Promise<Plugin> =>
  request("DELETE", `${path(id)}${purge ? "?purge=true" : ""}`, parsePlugin);

export const getContributions = (signal?: AbortSignal): Promise<PluginContributions> =>
  request("GET", "/plugins/contributions", parseContributions, undefined, signal);

export const putPluginTheme = (plugin: string | null): Promise<PluginContributions> =>
  request("PUT", "/plugins/theme", parseContributions, { plugin });

const settingPath = (id: string, key: string): string => `${path(id)}/settings/${encodeURIComponent(key)}`;

export const putPluginSetting = (id: string, key: string, value: SettingFieldValue): Promise<PluginContributions> =>
  request("PUT", settingPath(id, key), parseContributions, { value });

export const resetPluginSetting = (id: string, key: string): Promise<PluginContributions> =>
  request("DELETE", settingPath(id, key), parseContributions);
