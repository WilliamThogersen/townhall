import { isFields, isSettingValue, parseSettings, SETTING_KEYS } from "../settings/settings";
import { ApiError } from "./errors";
import { request } from "./request";
import type { PutSettingInput, SettingKey, Settings } from "./types";

function fail(what: string): never {
  throw new ApiError("protocol", `unexpected response shape: ${what}`);
}

export function parseSettingsData(value: unknown): Settings {
  if (!isFields(value)) return fail("settings is not an object");
  const invalid = SETTING_KEYS.find((key) => !isSettingValue(key, value[key]));
  if (invalid !== undefined) return fail(`settings.${invalid}`);
  return parseSettings(value);
}

export const getSettings = (signal?: AbortSignal): Promise<Settings> =>
  request("GET", "/settings", parseSettingsData, undefined, signal);

export function putSetting<K extends SettingKey>(key: K, value: Settings[K]): Promise<Settings> {
  const body: PutSettingInput<K> = { value };
  return request("PUT", `/settings/${encodeURIComponent(key)}`, parseSettingsData, body);
}
