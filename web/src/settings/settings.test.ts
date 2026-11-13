import { describe, expect, it } from "vitest";
import {
  AGENTS_AT_ONCE_MAX,
  AGENTS_AT_ONCE_MIN,
  DEFAULT_SETTINGS,
  FORGE_ACCOUNTS_MAX,
  isSettingValue,
  parseSettings,
  SETTING_KEYS,
  type Settings,
} from "./settings";

const custom: Settings = {
  agentsAtOnce: 12,
  repoUnclear: "guess",
  sessionPanelOpen: false,
  notifyWhen: "off",
  minFreeDiskGb: 20,
  learnAboutMe: true,
  githubAccount: "octocat",
  forgeAccounts: [
    { forge: "github", login: "work-me", host: "github.com", label: "work" },
    { forge: "gitlab", login: "me", host: "gitlab.corp.io", label: "me" },
  ],
  reflectAfterTasks: true,
  clonesDir: "~/Developer",
  adviserPermissions: false,
  permissionTimeoutSecs: 90,
};

describe("DEFAULT_SETTINGS", () => {
  it("matches the design defaults", () => {
    expect(DEFAULT_SETTINGS).toEqual({
      agentsAtOnce: 6,
      repoUnclear: "ask",
      sessionPanelOpen: true,
      notifyWhen: "everything",
      minFreeDiskGb: 5,
      learnAboutMe: false,
      githubAccount: "",
      forgeAccounts: [],
      reflectAfterTasks: false,
      clonesDir: "",
      adviserPermissions: true,
      permissionTimeoutSecs: 60,
    });
  });

  it("checks the permission timeout like the daemon: a whole number from 5 to 600", () => {
    [5, 60, 600].forEach((value) => expect(isSettingValue("permissionTimeoutSecs", value)).toBe(true));
    [0, 4, 601, 1.5, "60", null].forEach((value) => expect(isSettingValue("permissionTimeoutSecs", value)).toBe(false));
    expect(parseSettings({ permissionTimeoutSecs: 9000 }).permissionTimeoutSecs).toBe(60);
    expect(parseSettings({ adviserPermissions: "no" }).adviserPermissions).toBe(true);
  });

  it("checks the clones dir like the daemon: empty, ~, ~/x or absolute", () => {
    ["", "~", "~/Developer", "/srv/code"].forEach((value) => expect(isSettingValue("clonesDir", value)).toBe(true));
    ["relative/dir", "  ", "~other", 7, null].forEach((value) => expect(isSettingValue("clonesDir", value)).toBe(false));
    expect(parseSettings({ clonesDir: "nope" }).clonesDir).toBe("");
  });

  it("is frozen", () => {
    expect(Object.isFrozen(DEFAULT_SETTINGS)).toBe(true);
  });

  it("keeps agents at once between 1 and 50", () => {
    expect(AGENTS_AT_ONCE_MIN).toBe(1);
    expect(AGENTS_AT_ONCE_MAX).toBe(50);
  });
});

describe("parseSettings", () => {
  it("keeps every valid field", () => {
    expect(parseSettings({ ...custom })).toEqual(custom);
  });

  it("falls back to the defaults for anything that is not an object", () => {
    [null, undefined, 3, "x", true, [1, 2]].forEach((raw) => {
      expect(parseSettings(raw)).toEqual(DEFAULT_SETTINGS);
    });
  });

  it("falls back per field for missing or wrong values", () => {
    const parsed = parseSettings({
      agentsAtOnce: 12,
      repoUnclear: 1,
      notifyWhen: null,
    });
    expect(parsed).toEqual({ ...DEFAULT_SETTINGS, agentsAtOnce: 12 });
  });

  it("rejects agents at once outside 1 to 50 or not a whole number", () => {
    [0, 51, -3, 2.5, Number.NaN, Number.POSITIVE_INFINITY, "6"].forEach((agentsAtOnce) => {
      expect(parseSettings({ agentsAtOnce }).agentsAtOnce).toBe(DEFAULT_SETTINGS.agentsAtOnce);
    });
    expect(parseSettings({ agentsAtOnce: 1 }).agentsAtOnce).toBe(1);
    expect(parseSettings({ agentsAtOnce: 50 }).agentsAtOnce).toBe(50);
  });

  it("drops unknown fields", () => {
    expect(parseSettings({ ...custom, theme: "light" })).toEqual(custom);
  });

  it("drops the keys of removed settings", () => {
    const removed = {
      sharedCargoTarget: true,
      sharedTargetMaxGb: 50,
      permissionMode: "bypassPermissions",
      workWithoutAsking: true,
      townhallModel: "opus",
    };
    expect(parseSettings({ ...custom, ...removed })).toEqual(custom);
  });

  it("returns a frozen object", () => {
    expect(Object.isFrozen(parseSettings(custom))).toBe(true);
  });
});

describe("SETTING_KEYS", () => {
  it("lists every setting once", () => {
    expect([...SETTING_KEYS].sort()).toEqual(Object.keys(DEFAULT_SETTINGS).sort());
    expect(new Set(SETTING_KEYS).size).toBe(SETTING_KEYS.length);
  });

  it("is frozen", () => {
    expect(Object.isFrozen(SETTING_KEYS)).toBe(true);
  });
});

describe("isSettingValue", () => {
  it("accepts the values the daemon accepts", () => {
    SETTING_KEYS.forEach((key) => {
      expect(isSettingValue(key, custom[key])).toBe(true);
      expect(isSettingValue(key, DEFAULT_SETTINGS[key])).toBe(true);
    });
    expect(isSettingValue("agentsAtOnce", 1)).toBe(true);
    expect(isSettingValue("agentsAtOnce", 50)).toBe(true);
  });

  it("refuses wrong types and values outside the choices", () => {
    expect(isSettingValue("sessionPanelOpen", 1)).toBe(false);
    expect(isSettingValue("repoUnclear", "always")).toBe(false);
    expect(isSettingValue("notifyWhen", null)).toBe(false);
    expect(isSettingValue("minFreeDiskGb", 0)).toBe(true);
    expect(isSettingValue("minFreeDiskGb", -1)).toBe(false);
    expect(isSettingValue("minFreeDiskGb", 1.5)).toBe(false);
    expect(isSettingValue("githubAccount", "WilliamThogersen")).toBe(true);
    expect(isSettingValue("githubAccount", "")).toBe(true);
    expect(isSettingValue("githubAccount", "two words")).toBe(false);
    expect(isSettingValue("githubAccount", 7)).toBe(false);
    [0, 51, 2.5, "6", Number.NaN].forEach((value) => {
      expect(isSettingValue("agentsAtOnce", value)).toBe(false);
    });
  });

  it("accepts a list of complete forge accounts and refuses anything else", () => {
    const one = { forge: "github", login: "me", host: "github.com", label: "me" };
    expect(isSettingValue("forgeAccounts", [])).toBe(true);
    expect(isSettingValue("forgeAccounts", [one])).toBe(true);
    expect(isSettingValue("forgeAccounts", "me")).toBe(false);
    expect(isSettingValue("forgeAccounts", [{ ...one, forge: "bitbucket" }])).toBe(false);
    expect(isSettingValue("forgeAccounts", [{ ...one, login: "" }])).toBe(false);
    expect(isSettingValue("forgeAccounts", [{ ...one, host: undefined }])).toBe(false);
    expect(isSettingValue("forgeAccounts", [{ forge: "github", login: "me" }])).toBe(false);
    expect(isSettingValue("forgeAccounts", [{ ...one, token: "ghp_x" }])).toBe(false);
    expect(isSettingValue("forgeAccounts", [one, null])).toBe(false);
    expect(isSettingValue("forgeAccounts", Array.from({ length: FORGE_ACCOUNTS_MAX + 1 }, () => one))).toBe(false);
  });

  it("falls back to no accounts when the stored list is invalid", () => {
    expect(parseSettings({ forgeAccounts: "me" }).forgeAccounts).toEqual([]);
    expect(parseSettings({ ...custom }).forgeAccounts).toEqual(custom.forgeAccounts);
  });
});
