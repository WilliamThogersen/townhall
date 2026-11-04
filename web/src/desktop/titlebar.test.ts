import { afterEach, describe, expect, it } from "vitest";
import { initTitlebar, OVERLAY_TITLEBAR } from "./titlebar";

const MAC_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

const desktopWindow = (userAgent: string) => ({ __TAURI_INTERNALS__: {}, navigator: { userAgent } });
const browserWindow = (userAgent: string) => ({ navigator: { userAgent } });

afterEach(() => {
  delete document.documentElement.dataset.titlebar;
});

describe("initTitlebar", () => {
  it("marks the root for the overlay title bar in the macOS desktop app", () => {
    expect(initTitlebar(desktopWindow(MAC_UA), document)).toBe(true);
    expect(document.documentElement.dataset.titlebar).toBe(OVERLAY_TITLEBAR);
  });

  it("leaves the root alone in a browser on macOS", () => {
    expect(initTitlebar(browserWindow(MAC_UA), document)).toBe(false);
    expect(document.documentElement.dataset.titlebar).toBeUndefined();
  });

  it("leaves the root alone in the desktop app off macOS", () => {
    expect(initTitlebar(desktopWindow(WINDOWS_UA), document)).toBe(false);
    expect(document.documentElement.dataset.titlebar).toBeUndefined();
  });
});
