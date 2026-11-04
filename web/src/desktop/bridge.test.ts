import { afterEach, describe, expect, it, vi } from "vitest";
import { pickFolder } from "./bridge";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => {
  vi.unstubAllGlobals();
  invoke.mockReset();
});

describe("pickFolder", () => {
  it("refuses outside the desktop app", async () => {
    await expect(pickFolder()).rejects.toThrow("desktop commands are only available in the app");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("asks the app for a folder", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    invoke.mockResolvedValue("/Users/me/code/shop");
    await expect(pickFolder()).resolves.toBe("/Users/me/code/shop");
    expect(invoke).toHaveBeenCalledWith("pick_folder");
  });
});
