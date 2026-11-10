import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aMarketplace, anAvailablePlugin, aPlugin, aPreview } from "./fixtures";
import { addMarketplace, listMarketplaces, refreshMarketplace, removeMarketplace } from "./marketplaces";
import { deletePlugin, installPlugin, listPlugins, patchPlugin, previewPlugin, updatePlugin } from "./plugins";

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
