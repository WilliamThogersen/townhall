import { describe, expect, it } from "vitest";
import { conversationCount, repoName, shortRepoPath, userHomeOf } from "./repoPath";

const HOME = "/Users/me";

describe("shortRepoPath", () => {
  it("shows home as ~ with the first folder, … and the last two", () => {
    expect(shortRepoPath("/Users/me/Developer/wexo/work/clients/shop", HOME)).toBe("~/Developer/…/clients/shop");
    expect(shortRepoPath("/Users/me/Developer/a/b/c/hive", HOME)).toBe("~/Developer/…/c/hive");
  });

  it("keeps short paths whole", () => {
    expect(shortRepoPath("/Users/me/Developer/townhall", HOME)).toBe("~/Developer/townhall");
    expect(shortRepoPath("/Users/me/Developer/a/b/c", HOME)).toBe("~/Developer/a/b/c");
    expect(shortRepoPath("/Users/me/repo", HOME)).toBe("~/repo");
  });

  it("shows home itself as ~", () => {
    expect(shortRepoPath("/Users/me", HOME)).toBe("~");
  });

  it("shortens paths outside home from the root", () => {
    expect(shortRepoPath("/private/tmp/a/scratchpad/demo-repo", HOME)).toBe("/private/…/scratchpad/demo-repo");
    expect(shortRepoPath("/srv/git/shop", HOME)).toBe("/srv/git/shop");
  });

  it("does not treat a sibling with the same prefix as home", () => {
    expect(shortRepoPath("/Users/meg/Developer/x", HOME)).toBe("/Users/meg/Developer/x");
  });

  it("ignores trailing slashes on the path and on home", () => {
    expect(shortRepoPath("/Users/me/Developer/wexo/work/clients/shop/", "/Users/me/")).toBe("~/Developer/…/clients/shop");
    expect(shortRepoPath("/Users/me/Developer/townhall/", HOME)).toBe("~/Developer/townhall");
  });

  it("works without a known home", () => {
    expect(shortRepoPath("/Users/me/Developer/wexo/work/clients/shop", null)).toBe("/Users/…/clients/shop");
  });

  it("keeps the root whole", () => {
    expect(shortRepoPath("/", HOME)).toBe("/");
  });
});

describe("repoName", () => {
  it("is the last folder", () => {
    expect(repoName("/Users/me/Developer/townhall")).toBe("townhall");
  });

  it("ignores a trailing slash", () => {
    expect(repoName("/Users/me/Developer/shop/")).toBe("shop");
  });

  it("keeps a path with no folders as it is", () => {
    expect(repoName("/")).toBe("/");
    expect(repoName("shop")).toBe("shop");
  });
});

describe("userHomeOf", () => {
  it("is the parent of the townhall home", () => {
    expect(userHomeOf("/Users/me/.townhall")).toBe("/Users/me");
    expect(userHomeOf("/Users/me/.townhall/")).toBe("/Users/me");
  });

  it("is unknown when the townhall home is somewhere else", () => {
    expect(userHomeOf("/var/lib/townhall")).toBeNull();
    expect(userHomeOf("/.townhall")).toBeNull();
  });
});

describe("conversationCount", () => {
  it("counts conversations in words", () => {
    expect(conversationCount(0)).toBe("no conversations");
    expect(conversationCount(1)).toBe("1 conversation");
    expect(conversationCount(2)).toBe("2 conversations");
  });
});
