import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aMarketplace, anAvailablePlugin, aPlugin, aPreview, aSettingField, someContributions } from "./fixtures";
import { addMarketplace, listMarketplaces, refreshMarketplace, removeMarketplace } from "./marketplaces";
import {
  deletePlugin,
  getContributions,
  installPlugin,
  listPlugins,
  patchPlugin,
  previewPlugin,
  putPluginSetting,
  putPluginTheme,
  resetPluginSetting,
  updatePlugin,
} from "./plugins";

afterEach(() => vi.unstubAllGlobals());

describe("marketplaces api", () => {
  it("lists marketplaces with their plugins", async () => {
    const market = aMarketplace({ plugins: [{ name: "lint", description: "d", version: null, category: null }] });
    const fn = mockFetch(200, [market]);
    expect(await listMarketplaces()).toEqual([market]);
    expect(sent(fn)).toEqual({ url: "/api/marketplaces", method: "GET", body: undefined });
  });

  it("adds, refreshes and removes, encoding the name", async () => {
    const fn = mockFetch(201, aMarketplace());
    await addMarketplace({ source: "acme/tools" });
    expect(sent(fn)).toEqual({ url: "/api/marketplaces", method: "POST", body: { source: "acme/tools" } });
    await refreshMarketplace("my tools");
    expect(sent(fn, 1)).toEqual({ url: "/api/marketplaces/my%20tools/refresh", method: "POST", body: undefined });
    await removeMarketplace("my tools");
    expect(sent(fn, 2)).toEqual({ url: "/api/marketplaces/my%20tools", method: "DELETE", body: undefined });
  });

  it("refuses a source of an unknown kind", async () => {
    mockFetch(200, [aMarketplace({ source: { source: "ftp" } as never })]);
    await expect(listMarketplaces()).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("plugins api", () => {
  it("lists available and installed plugins", async () => {
    const body = { available: [anAvailablePlugin()], installed: [aPlugin()] };
    const fn = mockFetch(200, body);
    expect(await listPlugins()).toEqual(body);
    expect(sent(fn)).toEqual({ url: "/api/plugins", method: "GET", body: undefined });
  });

  it("previews with the id in the query", async () => {
    const fn = mockFetch(200, aPreview());
    expect(await previewPlugin("lint@acme-tools")).toEqual(aPreview());
    expect(sent(fn).url).toBe("/api/plugins/preview?plugin=lint%40acme-tools");
  });

  it("installs with the approved hash", async () => {
    const fn = mockFetch(201, aPlugin());
    await installPlugin("lint@acme-tools", "sha256:aaa");
    expect(sent(fn)).toEqual({
      url: "/api/plugins",
      method: "POST",
      body: { plugin: "lint@acme-tools", approve: "sha256:aaa" },
    });
  });

  it("patches, updates and deletes by id", async () => {
    const fn = mockFetch(200, aPlugin());
    await patchPlugin("lint@acme-tools", { enabled: true });
    expect(sent(fn)).toEqual({ url: "/api/plugins/lint%40acme-tools", method: "PATCH", body: { enabled: true } });
    await deletePlugin("lint@acme-tools", true);
    expect(sent(fn, 1)).toEqual({ url: "/api/plugins/lint%40acme-tools?purge=true", method: "DELETE", body: undefined });
    await deletePlugin("lint@acme-tools");
    expect(sent(fn, 2).url).toBe("/api/plugins/lint%40acme-tools");
  });

  it("updates, passing the approved hash only when given", async () => {
    const update = { outcome: "needsApproval", sha: "2222222", added: ["api:tasks:read"], permissionsHash: "sha256:bbb", plugin: aPlugin() };
    const fn = mockFetch(200, update);
    expect(await updatePlugin("lint@acme-tools")).toEqual(update);
    expect(sent(fn).url).toBe("/api/plugins/lint%40acme-tools/update");
    await updatePlugin("lint@acme-tools", "sha256:bbb");
    expect(sent(fn, 1).url).toBe("/api/plugins/lint%40acme-tools/update?approve=sha256%3Abbb");
  });

  it("refuses an outcome it does not know", async () => {
    mockFetch(200, { outcome: "exploded", sha: null, added: [], permissionsHash: null, plugin: aPlugin() });
    await expect(updatePlugin("lint@acme-tools")).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("plugin contributions api", () => {
  const full = someContributions({
    starters: [{ plugin: "starter-prompts@official", title: "tidy", text: "tidy the readme" }],
    themes: [{ plugin: "theme-slate@official", name: "theme-slate" }],
    theme: { plugin: "theme-slate@official", css: ":root {\n  --th-bg: #000;\n}\n" },
    settings: [
      {
        plugin: "lint@acme-tools",
        name: "lint",
        fields: [
          aSettingField(),
          aSettingField({ key: "token", type: "string", default: "", min: null, max: null, secret: true, value: null, set: true }),
          aSettingField({ key: "mode", type: "enum", default: "fast", min: null, max: null, choices: ["fast", "slow"], value: "fast", description: "how" }),
        ],
      },
    ],
  });

  it("reads starters, themes, the active theme and the settings groups", async () => {
    const fn = mockFetch(200, full);
    expect(await getContributions()).toEqual(full);
    expect(sent(fn)).toEqual({ url: "/api/plugins/contributions", method: "GET", body: undefined });
  });

  it("reads a field the daemon sent without its optional keys", async () => {
    const { description, min, max, choices, ...bare } = aSettingField();
    expect([description, min, max, choices]).toEqual([null, 1, 9, null]);
    mockFetch(200, someContributions({ settings: [{ plugin: "a@b", name: "a", fields: [bare as never] }] }));
    const got = await getContributions();
    expect(got.settings[0]?.fields[0]).toMatchObject({ description: null, choices: null, min: null, max: null });
  });

  it("chooses and clears the theme with a required null", async () => {
    const fn = mockFetch(200, full);
    await putPluginTheme("theme-slate@official");
    expect(sent(fn)).toEqual({ url: "/api/plugins/theme", method: "PUT", body: { plugin: "theme-slate@official" } });
    await putPluginTheme(null);
    expect(sent(fn, 1).body).toEqual({ plugin: null });
  });

  it("saves and resets one setting, encoding the plugin id and the key", async () => {
    const fn = mockFetch(200, full);
    await putPluginSetting("lint@acme-tools", "max retries", 5);
    expect(sent(fn)).toEqual({
      url: "/api/plugins/lint%40acme-tools/settings/max%20retries",
      method: "PUT",
      body: { value: 5 },
    });
    await resetPluginSetting("lint@acme-tools", "max retries");
    expect(sent(fn, 1)).toEqual({
      url: "/api/plugins/lint%40acme-tools/settings/max%20retries",
      method: "DELETE",
      body: undefined,
    });
  });

  it("refuses a shape it does not know", async () => {
    for (const bad of [
      { ...full, starters: "x" },
      { ...full, theme: { plugin: "a" } },
      { ...full, settings: [{ plugin: "a", name: "a", fields: [{ ...aSettingField(), type: "float" }] }] },
      { ...full, settings: [{ plugin: "a", name: "a", fields: [{ ...aSettingField(), value: {} }] }] },
      { ...full, settings: [{ plugin: "a", name: "a", fields: [{ ...aSettingField(), set: "yes" }] }] },
    ]) {
      mockFetch(200, bad);
      await expect(getContributions()).rejects.toMatchObject({ kind: "protocol" });
    }
  });
});
