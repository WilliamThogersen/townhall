import { clampAgentsAtOnce, parseSettings, type Settings } from "./settings";

export const SETTINGS_STORAGE_KEY = "th-settings";

let current: Settings | null = null;
let listeners: ReadonlySet<() => void> = new Set();

function readStoredSettings(): Settings {
  try {
    const raw = window.localStorage.getItem(SETTINGS_STORAGE_KEY);
    return parseSettings(raw === null ? null : JSON.parse(raw));
  } catch (error) {
    console.warn("settings not read, using the defaults", error);
    return parseSettings(null);
  }
}

function storeSettings(settings: Settings): void {
  try {
    window.localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch (error) {
    console.warn("settings not saved, keeping them for this session", error);
  }
}

function sameSettings(a: Settings, b: Settings): boolean {
  return (Object.keys(a) as (keyof Settings)[]).every((key) => a[key] === b[key]);
}

export function getSettings(): Settings {
  if (current === null) current = readStoredSettings();
  return current;
}

export function subscribeSettings(listener: () => void): () => void {
  listeners = new Set([...listeners, listener]);
  return () => {
    listeners = new Set([...listeners].filter((known) => known !== listener));
  };
}

export function updateSettings(patch: Partial<Settings>): void {
  const before = getSettings();
  const merged = { ...before, ...patch };
  const next = parseSettings({ ...merged, agentsAtOnce: clampAgentsAtOnce(merged.agentsAtOnce) });
  if (sameSettings(before, next)) return;
  current = next;
  storeSettings(next);
  listeners.forEach((listener) => listener());
}
