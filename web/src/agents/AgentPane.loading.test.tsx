import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AgentPane } from "./AgentPane";

vi.mock("./useAgentStream", () => ({
  useAgentStream: () => ({ snapshot: "", status: "connecting", error: null }),
}));

vi.mock("./Terminal", () => new Promise(() => {}));

describe("AgentPane while the terminal loads", () => {
  it("shows the shared loading line in the body", async () => {
    render(
      <AgentPane
        agent={{ name: "u-demo", kind: null, branch: "b", worktree: "/w", alive: true }}
        onClose={() => {}}
        onStopped={() => {}}
      />,
    );
    const line = await screen.findByText("loading terminal…", {}, { timeout: 2000 });
    expect(line.closest(".th-loading")).not.toBeNull();
    expect(line.closest(".th-term")).not.toBeNull();
  });
});
