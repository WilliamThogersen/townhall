import { describe, expect, it } from "vitest";
import { aRepo } from "../../api/fixtures";
import { PREVIEW_STARTERS, starterPrompts } from "./starterPrompts";

describe("starterPrompts", () => {
  it("uses the preview's starters when no repo is known", () => {
    expect(starterPrompts(null)).toEqual(PREVIEW_STARTERS);
    expect(starterPrompts([])).toEqual(PREVIEW_STARTERS);
    expect(PREVIEW_STARTERS).toEqual([
      "townhall needs a settings page",
      "fix the failing tests on shop main",
      "find slow queries in search",
    ]);
  });

  it("names the known repos and their base branch", () => {
    const repos = [
      aRepo({ name: "townhall", baseBranch: "main" }),
      aRepo({ name: "shop", baseBranch: "develop" }),
      aRepo({ name: "data-hub" }),
    ];
    expect(starterPrompts(repos)).toEqual([
      "fix the failing tests on townhall main",
      "find dead code in shop",
      "tidy the readme in data-hub",
    ]);
  });

  it("reuses repos when fewer than three are known", () => {
    const repos = [aRepo({ name: "shop", baseBranch: "develop" })];
    expect(starterPrompts(repos)).toEqual([
      "fix the failing tests on shop develop",
      "find dead code in shop",
      "tidy the readme in shop",
    ]);
  });
});
