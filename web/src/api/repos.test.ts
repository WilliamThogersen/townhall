import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aRepo } from "./fixtures";
import { addRepo, listRepos, putRepoAccounts, removeRepo } from "./repos";

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

  it("picks a repo's accounts, null clearing a pick and a missing key leaving it", async () => {
    const picked = aRepo({ githubAccount: "work-me", gitlabAccount: null });
    const fn = mockFetch(200, picked);
    expect(await putRepoAccounts("shop", { githubAccount: "work-me", gitlabAccount: null })).toEqual(picked);
    expect(sent(fn)).toEqual({
      url: "/api/repos/shop/accounts",
      method: "PUT",
      body: { githubAccount: "work-me", gitlabAccount: null },
    });
    await putRepoAccounts("my repo", { githubAccount: null });
    expect(sent(fn, 1)).toEqual({ url: "/api/repos/my%20repo/accounts", method: "PUT", body: { githubAccount: null } });
  });

  it("reads the account picks, and has none for a repo that sends none", async () => {
    mockFetch(200, [aRepo({ githubAccount: "work-me" })]);
    expect(await listRepos()).toEqual([aRepo({ githubAccount: "work-me" })]);
    mockFetch(200, [{ name: "shop", path: "/code/shop", baseBranch: "main" }]);
    expect(await listRepos()).toEqual([aRepo()]);
    mockFetch(200, [aRepo({ githubAccount: 7 as unknown as string })]);
    await expect(listRepos()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("refuses a repo without a base branch", async () => {
    mockFetch(200, [{ name: "shop", path: "/code/shop" }]);
    await expect(listRepos()).rejects.toMatchObject({ kind: "protocol" });
  });
});
