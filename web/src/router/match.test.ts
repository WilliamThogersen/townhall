import { describe, expect, it } from "vitest";
import { matchRoute, PATHS } from "./match";

describe("matchRoute", () => {
  it.each([
    ["/", "home"],
    ["/townhall", "control"],
    ["/repos", "repos"],
    ["/agents", "agents"],
    ["/plugins", "plugins"],
    ["/settings", "settings"],
    ["/search", "search"],
    ["/c/abc", "conversation"],
  ] as const)("matches %s to %s", (path, name) => {
    expect(matchRoute(path).name).toBe(name);
  });

  it("captures and decodes the conversation id", () => {
    const route = matchRoute("/c/fix%20login");
    expect(route.params).toEqual({ id: "fix login" });
    expect(route.path).toBe("/c/fix%20login");
  });

  it("ignores one trailing slash", () => {
    expect(matchRoute("/repos/").name).toBe("repos");
    expect(matchRoute("/c/abc/").params).toEqual({ id: "abc" });
  });

  it.each(["/nope", "/c", "/c/", "/c/a/b", "/townhall/x", "/Repos"])(
    "sends %s to not found",
    (path) => {
      const route = matchRoute(path);
      expect(route.name).toBe("not-found");
      expect(route.params).toEqual({});
    },
  );

  it("keeps a malformed id verbatim instead of throwing", () => {
    expect(matchRoute("/c/%E0%A4%A").params).toEqual({ id: "%E0%A4%A" });
  });

  it("parses the query string", () => {
    expect(matchRoute("/search", "?q=fix+login&x=1").query).toEqual({ q: "fix login", x: "1" });
    expect(matchRoute("/townhall", "").query).toEqual({});
  });
});

describe("PATHS", () => {
  it("builds every route path", () => {
    expect(PATHS.home).toBe("/");
    expect(PATHS.control).toBe("/townhall");
    expect(PATHS.repos).toBe("/repos");
    expect(PATHS.settings).toBe("/settings");
    expect(PATHS.search).toBe("/search");
    expect(PATHS.conversation("a b/c")).toBe("/c/a%20b%2Fc");
  });

  it("round trips a conversation id", () => {
    expect(matchRoute(PATHS.conversation("a b/c")).params.id).toBe("a b/c");
  });
});
