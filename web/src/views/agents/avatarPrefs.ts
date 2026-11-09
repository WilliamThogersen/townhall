import { useSyncExternalStore } from "react";
import { logWarn } from "../../log/report";
import { defaultAvatar, parseAvatar, type Avatar } from "./avatar";

export const AVATAR_PREFS_KEY = "th-agent-avatars";

export type AvatarPrefs = Readonly<Record<string, Avatar>>;

const keyOf = (name: string) => name.trim().toLowerCase();

const listeners = new Set<() => void>();
const defaults = new Map<string, Avatar>();

let cachedRaw: string | null | undefined;
let cachedPrefs: AvatarPrefs = {};

function readRaw(): string | null {
  try {
    return window.localStorage.getItem(AVATAR_PREFS_KEY);
  } catch {
    return null;
  }
}

function parsePrefs(raw: string): AvatarPrefs {
  let json: unknown;
  try {
    json = JSON.parse(raw);
  } catch (error) {
    logWarn("web.avatar_prefs_invalid", "warn: saved agent avatars are not valid JSON, using the defaults", error);
    return {};
  }
  if (typeof json !== "object" || json === null || Array.isArray(json)) return {};
  const valid = Object.entries(json).flatMap(([name, value]) => {
    const avatar = parseAvatar(value);
    return avatar === null ? [] : [[name, avatar] as const];
  });
  return Object.fromEntries(valid);
}

/** Saved avatars by lowercased agent type name. Stable between calls while the stored text is unchanged. */
export function readAvatarPrefs(): AvatarPrefs {
  const raw = readRaw();
  if (raw !== cachedRaw) {
    cachedRaw = raw;
    cachedPrefs = raw === null ? {} : parsePrefs(raw);
  }
  return cachedPrefs;
}

function defaultFor(name: string): Avatar {
  const key = keyOf(name);
  const known = defaults.get(key);
  if (known !== undefined) return known;
  const made = defaultAvatar(key);
  defaults.set(key, made);
  return made;
}

export function avatarFor(name: string): Avatar {
  return readAvatarPrefs()[keyOf(name)] ?? defaultFor(name);
}

function write(next: AvatarPrefs): boolean {
  try {
    window.localStorage.setItem(AVATAR_PREFS_KEY, JSON.stringify(next));
  } catch (error) {
    logWarn("web.avatar_prefs_not_saved", "warn: agent avatar not saved, browser storage refused the write", error);
    return false;
  }
  listeners.forEach((listener) => listener());
  return true;
}

export function saveAvatar(name: string, avatar: Avatar): boolean {
  return write({ ...readAvatarPrefs(), [keyOf(name)]: avatar });
}

export function clearAvatar(name: string): boolean {
  const { [keyOf(name)]: _gone, ...rest } = readAvatarPrefs();
  return write(rest);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  const onStorage = (event: StorageEvent) => {
    if (event.key === AVATAR_PREFS_KEY || event.key === null) listener();
  };
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", onStorage);
  };
}

export function useAvatar(name: string): Avatar {
  return useSyncExternalStore(subscribe, () => avatarFor(name));
}
