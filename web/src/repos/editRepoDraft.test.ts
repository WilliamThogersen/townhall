import { describe, expect, it } from "vitest";
import { aRepo } from "../api/fixtures";
import { canSave, draftOf, patchOf, type EditRepoDraft } from "./editRepoDraft";

const repo = aRepo({ name: "shop", path: "/code/shop", baseBranch: "main" });
const edit = (over: Partial<EditRepoDraft>): EditRepoDraft => ({ ...draftOf(repo), ...over });

describe("editRepoDraft", () => {
  it("starts from the repo as it is, with nothing to save", () => {
    expect(draftOf(repo)).toEqual({ name: "shop", baseBranch: "main", path: "/code/shop", moveFolder: false });
    expect(patchOf(repo, draftOf(repo))).toBeNull();
    expect(canSave(repo, draftOf(repo))).toBe(false);
  });

  it("sends only what changed, trimmed", () => {
    expect(patchOf(repo, edit({ name: " store " }))).toEqual({ name: "store" });
    expect(patchOf(repo, edit({ baseBranch: "develop" }))).toEqual({ baseBranch: "develop" });
    expect(patchOf(repo, edit({ path: " /code/new " }))).toEqual({ path: "/code/new" });
    expect(patchOf(repo, edit({ name: "store", baseBranch: "develop" }))).toEqual({ name: "store", baseBranch: "develop" });
  });

  it("sends moveFolder only together with a changed path", () => {
    expect(patchOf(repo, edit({ path: "/code/new", moveFolder: true }))).toEqual({ path: "/code/new", moveFolder: true });
    expect(patchOf(repo, edit({ moveFolder: true }))).toBeNull();
  });

  it("does not save a blank field", () => {
    expect(canSave(repo, edit({ name: "  " }))).toBe(false);
    expect(canSave(repo, edit({ baseBranch: "" }))).toBe(false);
    expect(canSave(repo, edit({ path: " " }))).toBe(false);
    expect(canSave(repo, edit({ name: "store" }))).toBe(true);
  });

  it("does not save a name the daemon would refuse", () => {
    expect(canSave(repo, edit({ name: "a b" }))).toBe(false);
    expect(canSave(repo, edit({ name: "-x" }))).toBe(false);
    expect(canSave(repo, edit({ name: "my.shop_2" }))).toBe(true);
  });
});
