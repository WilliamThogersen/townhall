import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { listPluginUi, mintHostToken } from "./plugins";

afterEach(() => vi.unstubAllGlobals());

const entry = { plugin: "panel@acme", slot: "page", title: "Panel", entry: "ui/panel.html", height: null };

describe("listPluginUi", () => {
  it("reads the ui entries of the enabled plugins", async () => {
    const fn = mockFetch(200, [entry, { ...entry, slot: "sidebar.foot", height: 40 }]);
    const found = await listPluginUi();
    expect(sent(fn)).toEqual({ url: "/api/plugins/ui", method: "GET", body: undefined });
    expect(found).toEqual([entry, { ...entry, slot: "sidebar.foot", height: 40 }]);
  });

  it("refuses an entry with no entry file", async () => {
    mockFetch(200, [{ ...entry, entry: 4 }]);
    await expect(listPluginUi()).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("mintHostToken", () => {
  it("posts to the plugin's host-token route, encoding the id", async () => {
    const answer = { plugin: "panel@acme", version: "1.0.0", token: "ab".repeat(32), scopes: ["tasks:read"] };
    const fn = mockFetch(200, answer);
    expect(await mintHostToken("panel@acme")).toEqual(answer);
    expect(sent(fn)).toEqual({ url: "/api/plugins/panel%40acme/host-token", method: "POST", body: undefined });
  });

  it("accepts a plugin with no version", async () => {
    mockFetch(200, { plugin: "p@m", version: null, token: "t", scopes: [] });
    expect(await mintHostToken("p@m")).toMatchObject({ version: null });
  });

  it("surfaces a refusal", async () => {
    mockFetch(404, null, "enabled plugin `p@m` not found");
    await expect(mintHostToken("p@m")).rejects.toMatchObject({ kind: "http", status: 404 });
  });
});
