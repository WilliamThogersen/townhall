import { logWarn } from "../../log/report";

export const LEGACY_AVATAR_PREFS_KEY = "th-agent-avatars";

export function dropLegacyAvatarPrefs(): void {
  try {
    window.localStorage.removeItem(LEGACY_AVATAR_PREFS_KEY);
  } catch (error) {
    logWarn("web.avatar_prefs_not_dropped", "warn: old browser-only agent avatars could not be removed", error);
  }
}
