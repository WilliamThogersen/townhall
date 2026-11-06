import { describe, expect, it } from "vitest";
import { anArtifact } from "../../../api/fixtures";
import { isTextProof, proofGroups } from "./proofModel";

describe("proofGroups", () => {
  it("groups by author role in the order of the loop and drops empty roles", () => {
    const groups = proofGroups([
      anArtifact({ id: "1", authorRole: "reviewer" }),
      anArtifact({ id: "2", authorRole: "coder" }),
      anArtifact({ id: "3", authorRole: "reviewer" }),
      anArtifact({ id: "4", authorRole: "other" }),
    ]);
    expect(groups.map((g) => [g.role, g.label, g.items.map((a) => a.id)])).toEqual([
      ["coder", "coder", ["2"]],
      ["reviewer", "reviewer", ["1", "3"]],
      ["other", "other", ["4"]],
    ]);
  });

  it("keeps the upload order inside a group", () => {
    const groups = proofGroups([
      anArtifact({ id: "b", authorRole: "tester" }),
      anArtifact({ id: "a", authorRole: "tester" }),
    ]);
    expect(groups[0]?.items.map((a) => a.id)).toEqual(["b", "a"]);
  });

  it("has no groups without artifacts", () => {
    expect(proofGroups([])).toEqual([]);
  });
});

describe("isTextProof", () => {
  it("is true for logs, notes and reports and false for screenshots", () => {
    expect(isTextProof(anArtifact({ kind: "log" }))).toBe(true);
    expect(isTextProof(anArtifact({ kind: "note" }))).toBe(true);
    expect(isTextProof(anArtifact({ kind: "report" }))).toBe(true);
    expect(isTextProof(anArtifact({ kind: "screenshot" }))).toBe(false);
  });
});
