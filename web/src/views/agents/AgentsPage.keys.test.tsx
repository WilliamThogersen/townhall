import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { anAgentTypeSummary } from "../../api/fixtures";
import * as conversations from "../../api/conversations";
import * as library from "../../api/library";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { AgentsPage } from "./AgentsPage";

function Bar() {
  const hints = useCurrentKeyHints();
  return <p data-testid="bar">{hints.map((h) => `${h.keys} ${h.label}`).join(" | ")}</p>;
}

beforeEach(() => {
  vi.spyOn(conversations, "listConversations").mockResolvedValue([]);
  vi.spyOn(library, "listAgentUsage").mockResolvedValue([]);
  vi.spyOn(library, "listAgentTypes").mockResolvedValue([anAgentTypeSummary({ name: "builder" })]);
});

afterEach(() => vi.restoreAllMocks());

describe("agents key hints", () => {
  it("shows the keys of the agent list", async () => {
    render(
      <KeyHintsProvider>
        <Bar />
        <AgentsPage />
      </KeyHintsProvider>,
    );
    await screen.findByRole("button", { name: /builder/ });
    expect(screen.getByTestId("bar")).toHaveTextContent("↑↓ agent types | esc clear filter");
  });
});
