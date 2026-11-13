import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PermissionSuggestions } from "./PermissionSuggestions";

const decision = (id: number, verdict: string, command: string, reason: string) => ({
  id,
  task: "fix-login",
  agentType: "builder",
  command,
  pattern: "Bash(git status *)",
  verdict,
  reason,
  createdAt: "2026-11-13T10:00:00.000Z",
});

const suggestion = { agentType: "builder", tool: "Bash(git status *)", approvals: 3 };

interface Setup {
  readonly decisions?: readonly unknown[];
  readonly suggestions?: readonly unknown[];
  readonly acceptStatus?: number;
}

function setup({ decisions = [], suggestions = [], acceptStatus = 200 }: Setup = {}) {
  const calls: { method: string; url: string; body: unknown }[] = [];
  let open = suggestions;
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const body = typeof init?.body === "string" ? (JSON.parse(init.body) as unknown) : undefined;
    calls.push({ method, url, body });
    if (method === "POST") {
      if (acceptStatus !== 200) {
        return Response.json({ success: false, data: null, error: "no such agent type" }, { status: acceptStatus });
      }
      open = [];
      return Response.json({
        success: true,
        data: { agentType: "builder", tool: "Bash(git status *)", added: true },
        error: null,
      });
    }
    return Response.json({ success: true, data: { decisions, suggestions: open }, error: null });
  });
  vi.stubGlobal("fetch", fetch);
  render(<PermissionSuggestions />);
  return { calls };
}

afterEach(() => vi.unstubAllGlobals());

describe("PermissionSuggestions", () => {
  it("shows nothing before the adviser has decided anything", async () => {
    const { calls } = setup();
    await waitFor(() => expect(calls).toHaveLength(1));
    expect(screen.queryByText("permission suggestions")).toBeNull();
    expect(screen.queryByText("recent permission decisions")).toBeNull();
  });

  it("offers a pattern the adviser allowed three times and adds it only when asked", async () => {
    const { calls } = setup({ suggestions: [suggestion], decisions: [decision(1, "allow", "Bash: git status", "read-only")] });
    const row = await screen.findByRole("group", { name: "Suggestion Bash(git status *) for builder" });
    expect(within(row).getByText("Bash(git status *)")).toBeVisible();
    expect(within(row).getByText(/allowed 3 times/)).toBeVisible();
    expect(calls.filter((c) => c.method === "POST")).toHaveLength(0);
    await userEvent.click(within(row).getByRole("button", { name: "add to builder's tools" }));
    expect(calls.filter((c) => c.method === "POST")).toEqual([
      { method: "POST", url: "/api/permissions/suggestions/accept", body: { agentType: "builder", tool: "Bash(git status *)" } },
    ]);
    await waitFor(() => expect(screen.queryByRole("group", { name: /^Suggestion/ })).toBeNull());
  });

  it("says why a suggestion could not be added and keeps it", async () => {
    setup({ suggestions: [suggestion], acceptStatus: 404 });
    await userEvent.click(await screen.findByRole("button", { name: "add to builder's tools" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no such agent type");
    expect(screen.getByRole("button", { name: "add to builder's tools" })).toBeEnabled();
  });

  it("lists the latest decisions with their verdict and reason", async () => {
    setup({
      decisions: [
        decision(2, "hard-deny", "Bash: git push --force origin main", "always yours to decide: it force-pushes"),
        decision(1, "allow", "Bash: git status", "read-only"),
      ],
    });
    const log = await screen.findByRole("list", { name: "Recent permission decisions" });
    const items = within(log).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("hard-deny");
    expect(items[0]).toHaveTextContent("Bash: git push --force origin main");
    expect(items[0]).toHaveTextContent("always yours to decide: it force-pushes");
    expect(items[1]).toHaveTextContent("allow");
  });

  it("says when the decisions cannot be read", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({ success: false, data: null, error: "held" }, { status: 503 })));
    render(<PermissionSuggestions />);
    expect(await screen.findByRole("alert")).toHaveTextContent("held");
  });
});
