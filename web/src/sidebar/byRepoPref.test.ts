import { afterEach, describe, expect, it, vi } from "vitest";
import { BY_REPO_KEY, loadByRepo, saveByRepo } from "./byRepoPref";

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("by repo preference", () => {
  it("is off until saved on, and remembers both ways", () => {
    expect(loadByRepo()).toBe(false);
    saveByRepo(true);
    expect(localStorage.getItem(BY_REPO_KEY)).toBe("true");
    expect(loadByRepo()).toBe(true);
    saveByRepo(false);
    expect(loadByRepo()).toBe(false);
  });

  it("reads anything else as off", () => {
    localStorage.setItem(BY_REPO_KEY, "yes");
    expect(loadByRepo()).toBe(false);
  });

  it("works without storage", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(loadByRepo()).toBe(false);
    expect(() => saveByRepo(true)).not.toThrow();
  });
});
