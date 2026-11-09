import { describe, expect, expectTypeOf, it } from "vitest";
import type {
  AddRepoInput,
  ConversationPatch,
  CreateTaskInput,
  NewMessageInput,
  Settings,
} from "../api/types";

const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}"], {
  query: "?raw",
  import: "default",
  eager: true,
});

function filesWith(pattern: RegExp): readonly string[] {
  return Object.entries(sources)
    .filter(([, text]) => pattern.test(text))
    .map(([path]) => path)
    .sort();
}

type Carries<T> = "permissionMode" extends keyof T ? true : false;

describe("skip all", () => {
  it("is not a setting any more: only an agent type carries it", () => {
    expect(filesWith(/\bworkWithoutAsking\b/)).toEqual([]);
    expect(filesWith(/\/settings\/permissionMode/)).toEqual([]);
    expectTypeOf<Carries<Settings>>().toEqualTypeOf<false>();
    expectTypeOf<keyof Settings>().not.toEqualTypeOf<"permissionMode">();
  });

  it("is only read from an agent type, never chosen on the settings screen", () => {
    // the agent library shows a type's own mode, read-only; the loop stage reads it to tell planners apart
    expect(filesWith(/\bpermissionMode\b/)).toEqual([
      "/src/api/fixtures.ts",
      "/src/api/library.ts",
      "/src/api/types.ts",
      "/src/tasks/stage.ts",
      "/src/views/agents/AgentTypeDetail.tsx",
      "/src/views/agents/AgentTypeRow.tsx",
    ]);
    expect(filesWith(/\bbypassPermissions\b/)).toEqual([
      "/src/api/library.ts",
      "/src/api/types.ts",
      "/src/views/agents/AgentTypeDetail.tsx",
    ]);
  });

  it("cannot ride along in any other request body", () => {
    expectTypeOf<Carries<AddRepoInput>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<NewMessageInput>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<ConversationPatch>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<CreateTaskInput>>().toEqualTypeOf<false>();
  });
});
