import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aRepo } from "./fixtures";
import { addRepo, listRepos, removeRepo } from "./repos";

afterEach(() => vi.unstubAllGlobals());

describe("repos api", () => {
  it("lists repos", async () => {
    const fn = mockFetch(200, [aRepo()]);
    expect(await listRepos()).toEqual([aRepo()]);
    expect(sent(fn)).toEqual({ url: "/api/repos", method: "GET", body: undefined });
  });

  it("adds a repo by path", async () => {
    const fn = mockFetch(201, aRepo());
    expect(await addRepo({ path: "/code/shop" })).toEqual(aRepo());
    expect(sent(fn)).toEqual({ url: "/api/repos", method: "POST", body: { path: "/code/shop" } });
  });

  it("removes a repo and returns it", async () => {
    const fn = mockFetch(200, aRepo());
    expect(await removeRepo("shop")).toEqual(aRepo());
    expect(sent(fn)).toEqual({ url: "/api/repos/shop", method: "DELETE", body: undefined });
  });

  it("asks the daemon to delete the managed clone too", async () => {
    const fn = mockFetch(200, aRepo());
    await removeRepo("shop", true);
    expect(sent(fn)).toEqual({ url: "/api/repos/shop?deleteClone=true", method: "DELETE", body: undefined });
  });

  it("refuses a repo without a base branch", async () => {
    mockFetch(200, [{ name: "shop", path: "/code/shop" }]);
    await expect(listRepos()).rejects.toMatchObject({ kind: "protocol" });
  });
});
