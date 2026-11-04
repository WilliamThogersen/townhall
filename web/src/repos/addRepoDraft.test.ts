import { describe, expect, it } from "vitest";
import { canSubmit, draftToInput, EMPTY_DRAFT } from "./addRepoDraft";

describe("addRepoDraft", () => {
  it("needs a path", () => {
    expect(canSubmit(EMPTY_DRAFT)).toBe(false);
    expect(canSubmit({ ...EMPTY_DRAFT, path: "   " })).toBe(false);
    expect(canSubmit({ ...EMPTY_DRAFT, path: "~/code/shop" })).toBe(true);
  });

  it("sends the trimmed path and leaves blank options out", () => {
    expect(draftToInput({ path: " ~/code/shop ", name: " ", baseBranch: "" })).toEqual({ path: "~/code/shop" });
  });

  it("sends a name and a base branch when given", () => {
    expect(draftToInput({ path: "/code/shop", name: " shop-2 ", baseBranch: "develop " })).toEqual({
      path: "/code/shop",
      name: "shop-2",
      baseBranch: "develop",
    });
  });
});
