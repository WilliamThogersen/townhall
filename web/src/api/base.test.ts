import { describe, expect, it } from "vitest";
import { DESKTOP_HTTP_BASE, DESKTOP_WS_BASE, httpBase, isDesktop, wsBase } from "./base";
import { streamUrl } from "./client";

const tauriWin = { __TAURI_INTERNALS__: {} };

describe("api base selection", () => {
  it("defaults to same-origin /api in a plain browser", () => {
    expect(isDesktop({})).toBe(false);
    expect(httpBase({})).toBe("/api");
    expect(wsBase({})).toBeNull();
  });

  it("uses loopback urls inside Tauri", () => {
    expect(isDesktop(tauriWin)).toBe(true);
    expect(httpBase(tauriWin)).toBe(DESKTOP_HTTP_BASE);
    expect(wsBase(tauriWin)).toBe(DESKTOP_WS_BASE);
  });

  it("builds the stream url from the selected base", () => {
    const loc = { protocol: "http:", host: "h:1" } as Location;
    expect(streamUrl("a b", loc, tauriWin)).toBe("ws://127.0.0.1:7777/api/agents/a%20b/stream");
    expect(streamUrl("a b", loc, {})).toBe("ws://h:1/api/agents/a%20b/stream");
  });
});
