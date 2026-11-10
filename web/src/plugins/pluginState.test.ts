import { describe, expect, it } from "vitest";
import { aPlugin } from "../api/fixtures";
import { pendingUpdateSha, pluginStates } from "./pluginState";

describe("pendingUpdateSha", () => {
  it("reads the sha an update is waiting on from the plugin's error", () => {
    expect(pendingUpdateSha(aPlugin({ error: "update to 2222222 needs approval" }))).toBe("2222222");
  });

  it("is null for no error and for any other error", () => {
    expect(pendingUpdateSha(aPlugin())).toBeNull();
    expect(pendingUpdateSha(aPlugin({ error: "could not fetch" }))).toBeNull();
  });
});

describe("pluginStates", () => {
  it("names installed and enabled by what the plugin is", () => {
    expect(pluginStates(aPlugin())).toEqual(["installed"]);
    expect(pluginStates(aPlugin({ enabled: true }))).toEqual(["enabled"]);
  });

  it("adds pinned and needs approval", () => {
    expect(pluginStates(aPlugin({ enabled: true, pinned: true, error: "update to 2 needs approval" }))).toEqual([
      "enabled",
      "pinned",
      "needs approval",
    ]);
  });
});
