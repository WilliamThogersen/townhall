import { describe, expect, it } from "vitest";
import { anAgentTypeSummary } from "../../api/fixtures";
import { defaultTypeName, filterTypes, isAdviserType, isConversationType, pipelineStep, sortTypes } from "./agentTypes";

describe("sortTypes", () => {
  const named = (...names: string[]) => names.map((name) => anAgentTypeSummary({ name }));
  const order = (types: readonly { name: string }[]) => types.map((t) => t.name);

  it("puts the default first, then the busiest, then by name", () => {
    const usage = [
      { agent: "zed", runs: 9, tokens: 1 },
      { agent: "mid", runs: 4, tokens: 1 },
      { agent: "Alpha", runs: 4, tokens: 1 },
    ];
    expect(order(sortTypes(named("alpha", "mid", "zed", "builder", "idle"), "builder", usage))).toEqual([
      "builder",
      "zed",
      "alpha",
      "mid",
      "idle",
    ]);
  });

  it("sorts by name alone without usage", () => {
    expect(order(sortTypes(named("b", "a", "c"), null, null))).toEqual(["a", "b", "c"]);
  });

  it("does not change the input", () => {
    const input = named("b", "a");
    sortTypes(input, null, null);
    expect(order(input)).toEqual(["b", "a"]);
  });
});

describe("filterTypes", () => {
  const types = [
    anAgentTypeSummary({ name: "builder", description: "writes code", model: "sonnet" }),
    anAgentTypeSummary({ name: "dev-server", description: "runs the dev server", adapter: "shell", model: null }),
  ];

  it("keeps everything for an empty or blank query", () => {
    expect(filterTypes(types, "")).toEqual(types);
    expect(filterTypes(types, "   ")).toEqual(types);
  });

  it("matches name, description, adapter and model ignoring case", () => {
    expect(filterTypes(types, "BUILD").map((t) => t.name)).toEqual(["builder"]);
    expect(filterTypes(types, "runs the").map((t) => t.name)).toEqual(["dev-server"]);
    expect(filterTypes(types, "shell").map((t) => t.name)).toEqual(["dev-server"]);
    expect(filterTypes(types, "sonnet").map((t) => t.name)).toEqual(["builder"]);
  });

  it("finds nothing for an unknown word", () => {
    expect(filterTypes(types, "zzz")).toEqual([]);
  });
});

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

  it("is never the conversation type", () => {
    expect(defaultTypeName(named("default", "zed"))).toBe("zed");
    expect(defaultTypeName(named("Default", "builder"))).toBe("builder");
    expect(defaultTypeName(named("default"))).toBeNull();
  });
});

describe("the adviser", () => {
  const named = (...names: string[]) => names.map((name) => anAgentTypeSummary({ name }));

  it("is the type named adviser, ignoring case", () => {
    expect(isAdviserType("adviser")).toBe(true);
    expect(isAdviserType("Adviser")).toBe(true);
    expect(isAdviserType("builder")).toBe(false);
  });

  it("is never the type tasks default to", () => {
    expect(defaultTypeName(named("adviser", "zed"))).toBe("zed");
    expect(defaultTypeName(named("adviser", "default"))).toBeNull();
    expect(defaultTypeName(named("adviser", "builder"))).toBe("builder");
  });
});

describe("isConversationType", () => {
  it("is the type named default, ignoring case", () => {
    expect(isConversationType("default")).toBe(true);
    expect(isConversationType("Default")).toBe(true);
    expect(isConversationType("builder")).toBe(false);
  });
});
