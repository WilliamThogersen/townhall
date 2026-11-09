import { describe, expect, it } from "vitest";
import { addProgress, canSubmit, cloneTargetOf, draftToInput, EMPTY_DRAFT, isLink, linkProblem, modeProblem, withProtocol } from "./addRepoDraft";

const draft = (over: Partial<typeof EMPTY_DRAFT>) => ({ ...EMPTY_DRAFT, ...over });

describe("addRepoDraft", () => {
  it("needs a path", () => {
    expect(canSubmit(EMPTY_DRAFT)).toBe(false);
    expect(canSubmit(draft({ path: "   " }))).toBe(false);
    expect(canSubmit(draft({ path: "~/code/shop" }))).toBe(true);
  });

  it("sends the trimmed path and leaves blank options out", () => {
    expect(draftToInput(draft({ path: " ~/code/shop ", name: " " }))).toEqual({ path: "~/code/shop" });
  });

  it("sends a name and a base branch when given", () => {
    expect(draftToInput(draft({ path: "/code/shop", name: " shop-2 ", baseBranch: "develop " }))).toEqual({
      path: "/code/shop",
      name: "shop-2",
      baseBranch: "develop",
    });
  });

  it("sends the clone folder for a link only", () => {
    const link = "https://github.com/acme/shop.git";
    expect(draftToInput(draft({ path: link, cloneDir: " ~/Developer " }), "https")).toEqual({ path: link, cloneDir: "~/Developer" });
    expect(draftToInput(draft({ path: link, cloneDir: " " }), "https")).toEqual({ path: link });
    expect(draftToInput(draft({ path: "/code/shop", cloneDir: "~/Developer" }))).toEqual({ path: "/code/shop" });
  });
});

describe("links", () => {
  it("tells links from folders", () => {
    ["https://github.com/acme/shop", "ssh://git@host/a/b", "git@gitlab.com:g/s/app.git", " HTTPS://x/y/z "].forEach((v) =>
      expect(isLink(v)).toBe(true),
    );
    ["/code/shop", "~/shop", "shop", "C:/x"].forEach((v) => expect(isLink(v)).toBe(false));
  });

  it("says why a link cannot name a repo, and nothing for a folder or a good link", () => {
    expect(linkProblem("/code/shop")).toBeNull();
    expect(linkProblem("https://github.com/acme/shop")).toBeNull();
    expect(linkProblem("git@github.com:acme/shop.git")).toBeNull();
    expect(linkProblem("https://gitlab.com/group/sub/app.git/")).toBeNull();
    ["https://github.com", "https://github.com/acme", "git@github.com:shop", "https://github.com/acme/.."].forEach((v) =>
      expect(linkProblem(v), v).toMatch(/owner\/repo/),
    );
    expect(linkProblem("https://github.com/acme/my repo")).toMatch(/blanks/);
  });

  it("blocks submitting a bad link", () => {
    expect(canSubmit(draft({ path: "https://github.com/acme" }))).toBe(false);
    expect(canSubmit(draft({ path: "https://github.com/acme/shop" }))).toBe(true);
  });

  it("previews where a link is cloned", () => {
    const at = (path: string, cloneDir = "", fallback = "") => cloneTargetOf(draft({ path, cloneDir }), fallback);
    expect(at("https://github.com/acme/shop.git", "~/Developer")).toBe("~/Developer/shop");
    expect(at("git@github.com:acme/shop", "~/Developer/")).toBe("~/Developer/shop");
    expect(at("https://github.com/acme/shop", "", "/srv/code")).toBe("/srv/code/shop");
    expect(at("https://github.com/acme/shop", "", "")).toBe("~/.townhall/repos/shop");
    expect(at("/code/shop", "~/Developer")).toBeNull();
    expect(at("https://github.com/acme", "~/Developer")).toBeNull();
  });

  it("says cloning or initialising while it works", () => {
    expect(addProgress(draft({ path: "git@github.com:acme/shop.git" }))).toBe("cloning…");
    expect(addProgress(draft({ path: "/code/shop" }))).toBe("initialising…");
  });

  it("refuses a link in folder mode and a plain path in url mode", () => {
    expect(modeProblem("folder", "~/code/shop")).toBeNull();
    expect(modeProblem("folder", "https://github.com/acme/shop")).toMatch(/choose url/);
    expect(modeProblem("url", "https://github.com/acme/shop")).toBeNull();
    expect(modeProblem("url", "~/code/shop")).toMatch(/git url/);
    expect(modeProblem("url", "")).toBeNull();
    expect(modeProblem("url", "https://github.com/acme")).toMatch(/owner\/repo/);
  });
});

describe("withProtocol", () => {
  const ssh = (link: string) => withProtocol(link, "ssh");
  const https = (link: string) => withProtocol(link, "https");

  it("turns any github link into ssh", () => {
    expect(ssh("https://github.com/acme/shop")).toBe("git@github.com:acme/shop.git");
    expect(ssh("https://github.com/acme/shop.git")).toBe("git@github.com:acme/shop.git");
    expect(ssh("https://github.com/acme/shop/")).toBe("git@github.com:acme/shop.git");
    expect(ssh("git@github.com:acme/shop.git")).toBe("git@github.com:acme/shop.git");
    expect(ssh("git@github.com:acme/shop")).toBe("git@github.com:acme/shop.git");
    expect(ssh("ssh://git@github.com/acme/shop.git")).toBe("git@github.com:acme/shop.git");
  });

  it("turns any github link into https", () => {
    expect(https("git@github.com:acme/shop.git")).toBe("https://github.com/acme/shop.git");
    expect(https("git@github.com:acme/shop")).toBe("https://github.com/acme/shop.git");
    expect(https("ssh://git@github.com/acme/shop.git")).toBe("https://github.com/acme/shop.git");
    expect(https("https://github.com/acme/shop")).toBe("https://github.com/acme/shop.git");
    expect(https("http://github.com/acme/shop/")).toBe("https://github.com/acme/shop.git");
  });

  it("keeps nested gitlab groups", () => {
    expect(ssh("https://gitlab.example.com/konpap/vines.git")).toBe("git@gitlab.example.com:konpap/vines.git");
    expect(ssh("https://gitlab.com/group/sub/deep/app")).toBe("git@gitlab.com:group/sub/deep/app.git");
    expect(https("git@gitlab.com:group/sub/app.git")).toBe("https://gitlab.com/group/sub/app.git");
    expect(https("ssh://git@gitlab.com/group/sub/app/")).toBe("https://gitlab.com/group/sub/app.git");
  });

  it("drops credentials and trims blanks", () => {
    expect(ssh("  https://me:tok@gitlab.com/g/app.git ")).toBe("git@gitlab.com:g/app.git");
    expect(https("ssh://me@gitlab.com/g/app.git")).toBe("https://gitlab.com/g/app.git");
  });

  it("leaves links with a port, bad links and folders as they are", () => {
    expect(ssh("ssh://git@host:2222/g/app.git")).toBe("ssh://git@host:2222/g/app.git");
    expect(https("https://host:8443/g/app")).toBe("https://host:8443/g/app");
    expect(ssh("~/code/shop")).toBe("~/code/shop");
    expect(ssh("https://github.com/acme")).toBe("https://github.com/acme");
  });

  it("sends the converted link and previews the same one", () => {
    const link = "https://gitlab.example.com/konpap/vines.git";
    expect(draftToInput(draft({ path: link }), "ssh")).toEqual({ path: "git@gitlab.example.com:konpap/vines.git" });
    expect(draftToInput(draft({ path: link }), "https")).toEqual({ path: link });
    expect(draftToInput(draft({ path: "/code/shop" }), "ssh")).toEqual({ path: "/code/shop" });
  });
});
