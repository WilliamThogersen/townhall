/** Where the daemon API lives. Same-origin in dev and browsers, fixed loopback in the desktop shell. */

export const DEFAULT_HTTP_BASE = "/api";
export const DESKTOP_HTTP_BASE = "http://127.0.0.1:7777/api";
export const DESKTOP_WS_BASE = "ws://127.0.0.1:7777/api";

/** True when running inside the Tauri webview (it injects __TAURI_INTERNALS__). */
export function isDesktop(win: object = window): boolean {
  return "__TAURI_INTERNALS__" in win;
}

export function httpBase(win: object = window): string {
  return isDesktop(win) ? DESKTOP_HTTP_BASE : DEFAULT_HTTP_BASE;
}

/** WebSocket base, or null when it must be derived from the page location. */
export function wsBase(win: object = window): string | null {
  return isDesktop(win) ? DESKTOP_WS_BASE : null;
}
