import { describe, expect, expectTypeOf, it } from "vitest";
import type {
  AddRepoInput,
  ConversationPatch,
  CreateTaskInput,
  NewMessageInput,
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
  it("is set only through the settings endpoint client", () => {
    expect(filesWith(/request\(\s*"[A-Z]+",\s*[`"']\/settings/)).toEqual(["/src/api/settings.ts"]);
    expect(filesWith(/\bputSetting\b/)).toEqual(["/src/api/settings.ts", "/src/settings/settingsStore.ts"]);
  });

  it("is chosen only on the settings screen", () => {
    // the agent library only shows a type's own mode, read-only
    expect(filesWith(/\bpermissionMode\b/)).toEqual([
      "/src/api/fixtures.ts",
      "/src/api/library.ts",
      "/src/api/types.ts",
      "/src/settings/settings.ts",
      "/src/views/agents/AgentTypeDetail.tsx",
      "/src/views/agents/AgentTypeRow.tsx",
      "/src/views/settings/PermissionRow.tsx",
    ]);
    expect(filesWith(/\bbypassPermissions\b/)).toEqual([
      "/src/api/types.ts",
      "/src/settings/settings.ts",
      "/src/views/settings/settingsCopy.ts",
    ]);
  });

  it("cannot ride along in any other request body", () => {
    expectTypeOf<Carries<AddRepoInput>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<NewMessageInput>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<ConversationPatch>>().toEqualTypeOf<false>();
    expectTypeOf<Carries<CreateTaskInput>>().toEqualTypeOf<false>();
  });
});
