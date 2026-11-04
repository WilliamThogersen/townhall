import { toMessage } from "../api/errors";
import { isFields, isSettingValue, SETTING_KEYS, type SettingKey, type Settings } from "./settings";

export const BROWSER_SETTINGS_KEY = "th-settings";

export interface StoredSetting {
  readonly key: SettingKey;
  readonly value: Settings[SettingKey];
}

export type PutSetting = (key: SettingKey, value: Settings[SettingKey]) => Promise<Settings>;

export interface ImportDeps {
  readonly storage: Storage | null;
  readonly put: PutSetting;
  readonly onSaved: (settings: Settings) => void;
}

export interface ImportOutcome {
  readonly sent: number;
  readonly failed: number;
  readonly message: string | null;
}

interface Failure {
  readonly setting: StoredSetting;
  readonly message: string;
}

const NOTHING: ImportOutcome = Object.freeze({ sent: 0, failed: 0, message: null });

function warn(what: string, error: unknown): void {
  console.warn(`warn: ${what}`, error);
}

function parseJson(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function readStoredSettings(raw: string): readonly StoredSetting[] {
  const fields = parseJson(raw);
  if (!isFields(fields)) return [];
  return SETTING_KEYS.flatMap((key) => {
    const value = fields[key];
    return isSettingValue(key, value) ? [{ key, value }] : [];
  });
}

function readRaw(storage: Storage): string | null {
  try {
    return storage.getItem(BROWSER_SETTINGS_KEY);
  } catch (error) {
    warn("settings saved in this browser cannot be read", error);
    return null;
  }
}

function keepOnly(storage: Storage, left: readonly StoredSetting[]): void {
  try {
    if (left.length === 0) {
      storage.removeItem(BROWSER_SETTINGS_KEY);
      return;
    }
    const fields = Object.fromEntries(left.map(({ key, value }) => [key, value]));
    storage.setItem(BROWSER_SETTINGS_KEY, JSON.stringify(fields));
  } catch (error) {
    warn("settings saved in this browser not updated", error);
  }
}

async function sendEach(settings: readonly StoredSetting[], deps: ImportDeps): Promise<readonly Failure[]> {
  let failures: readonly Failure[] = [];
  for (const setting of settings) {
    try {
      const saved = await deps.put(setting.key, setting.value);
      deps.onSaved(saved);
    } catch (error) {
      failures = [...failures, { setting, message: toMessage(error) }];
    }
  }
  return failures;
}

export async function importBrowserSettings(daemon: Settings, deps: ImportDeps): Promise<ImportOutcome> {
  const { storage } = deps;
  const raw = storage === null ? null : readRaw(storage);
  if (storage === null || raw === null) return NOTHING;
  const changed = readStoredSettings(raw).filter(({ key, value }) => daemon[key] !== value);
  const failures = await sendEach(changed, deps);
  keepOnly(
    storage,
    failures.map((failure) => failure.setting),
  );
  return { sent: changed.length, failed: failures.length, message: failures[0]?.message ?? null };
}

export function forgetStoredSetting(storage: Storage | null, key: SettingKey): void {
  const raw = storage === null ? null : readRaw(storage);
  if (storage === null || raw === null) return;
  keepOnly(
    storage,
    readStoredSettings(raw).filter((setting) => setting.key !== key),
  );
}
