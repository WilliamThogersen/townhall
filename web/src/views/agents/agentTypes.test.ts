import { describe, expect, it } from "vitest";
import { anAgentTypeSummary } from "../../api/fixtures";
import { defaultTypeName, pipelineStep } from "./agentTypes";

describe("pipelineStep", () => {
  it.each([
    ["planner", "plan"],
    ["coder", "code"],
    ["tester", "test"],
    ["reviewer", "review"],
    ["shipper", "ship"],
    ["Planner", "plan"],
  ])("%s belongs to %s", (name, step) => {
    expect(pipelineStep(name)).toBe(step);
  });

  it.each(["builder", "planner-2", ""])("gives %s no step", (name) => {
    expect(pipelineStep(name)).toBeNull();
  });
});

describe("defaultTypeName", () => {
  const named = (...names: string[]) => names.map((name) => anAgentTypeSummary({ name }));

  it("is builder when it exists, ignoring case", () => {
    expect(defaultTypeName(named("alpha", "Builder"))).toBe("Builder");
  });

  it("is the first type by name without a builder", () => {
    expect(defaultTypeName(named("zed", "alpha"))).toBe("alpha");
  });

  it("is null without types", () => {
    expect(defaultTypeName([])).toBeNull();
  });
});
