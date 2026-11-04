import { isDesktop } from "../api/base";

/** Root `data-titlebar` value when the macOS window draws its traffic lights over the page. */
export const OVERLAY_TITLEBAR = "overlay";

interface TitlebarWindow {
  readonly navigator: { readonly userAgent: string };
}

function isMac(win: TitlebarWindow): boolean {
  return win.navigator.userAgent.includes("Macintosh");
}

/**
 * The desktop window uses an overlay title bar on macOS (see tauri.conf.json), so the page
 * must leave room for the traffic lights. Marks the root so CSS can do that; returns whether it did.
 */
export function initTitlebar(win: TitlebarWindow = window, doc: Document = document): boolean {
  if (!isDesktop(win) || !isMac(win)) return false;
  doc.documentElement.dataset.titlebar = OVERLAY_TITLEBAR;
  return true;
}
