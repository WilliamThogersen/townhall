import { afterEach, describe, expect, it, vi } from "vitest";
import { acceptSuggestion, getPermissions } from "./permissions";

const decision = {
  id: 7,
  task: "fix-login",
  agentType: "builder",
  command: "Bash: git status",
  pattern: "Bash(git status *)",
  verdict: "allow",
  reason: "read-only",
  createdAt: "2026-11-13T10:00:00.000Z",
};
const suggestion = { agentType: "builder", tool: "Bash(git status *)", approvals: 3 };

function mockFetch(status: number, data: unknown, error: string | null = null) {
  const fn = vi.fn().mockResolvedValue(Response.json({ success: status < 400, data, error }, { status }));
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => vi.unstubAllGlobals());

describe("getPermissions", () => {
  it("reads the decisions and the suggestions", async () => {
    const fn = mockFetch(200, { decisions: [decision], suggestions: [suggestion] });
    expect(await getPermissions()).toEqual({ decisions: [decision], suggestions: [suggestion] });
    expect(fn.mock.calls[0]?.[0]).toBe("/api/permissions");
  });

  it("refuses an unknown verdict and a missing field", async () => {
    mockFetch(200, { decisions: [{ ...decision, verdict: "maybe" }], suggestions: [] });
    await expect(getPermissions()).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, { decisions: [], suggestions: [{ agentType: "builder" }] });
    await expect(getPermissions()).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("acceptSuggestion", () => {
  it("POSTs the type and the tool and reads whether it was added", async () => {
    const fn = mockFetch(200, { agentType: "builder", tool: "Bash(git status *)", added: true });
    expect(await acceptSuggestion("builder", "Bash(git status *)")).toEqual({
      agentType: "builder",
      tool: "Bash(git status *)",
      added: true,
    });
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/permissions/suggestions/accept");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({ agentType: "builder", tool: "Bash(git status *)" });
  });

  it("maps a 400 to an http error with the daemon's words", async () => {
    mockFetch(400, null, "tool must not start with -");
    await expect(acceptSuggestion("builder", "--x")).rejects.toMatchObject({
      kind: "http",
      status: 400,
      message: "tool must not start with -",
    });
  });
});
