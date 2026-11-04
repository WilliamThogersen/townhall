import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
import { AgentsProvider } from "../../agents/AgentsProvider";
import * as client from "../../api/client";
import type { Agent } from "../../api/types";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { ControlCenterPage } from "./ControlCenterPage";

vi.mock("../../agents/AgentPane", () => ({
  AgentPane: ({ agent, onClose, onStopped }: { agent: Agent; onClose: () => void; onStopped: () => void }) => (
    <article aria-label={`${agent.name} terminal`}>
      <button type="button" onClick={onClose}>
        close
      </button>
      <button type="button" onClick={onStopped}>
        stopped
      </button>
    </article>
  ),
}));

const mk = (name: string, alive = true): Agent => ({
  name,
  kind: "shell",
  branch: `townhall/${name}`,
  worktree: `/w/${name}`,
  alive,
});

function HintsProbe() {
  const hints = useCurrentKeyHints();
  return <output aria-label="hints">{hints.map((h) => `${h.keys} ${h.label}`).join(" | ")}</output>;
}

function show() {
  const user = userEvent.setup();
  const view = render(
    <AgentsProvider intervalMs={60_000}>
      <KeyHintsProvider>
        <ControlCenterPage />
        <HintsProbe />
      </KeyHintsProvider>
    </AgentsProvider>,
  );
  return { user, ...view };
}

const row = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name} `) });
const findRow = (name: string) => screen.findByRole("button", { name: new RegExp(`^${name} `) });
const pane = (name: string) => screen.queryByRole("article", { name: `${name} terminal` });
const hints = () => screen.getByRole("status", { hidden: true, name: "hints" });
const url = () => `${window.location.pathname}${window.location.search}`;

let list: MockInstance<typeof client.listAgents>;

beforeEach(() => {
  window.history.replaceState(null, "", "/townhall");
  list = vi.spyOn(client, "listAgents").mockResolvedValue([mk("b"), mk("a", false), mk("c")]);
});

afterEach(() => vi.restoreAllMocks());

describe("ControlCenterPage list", () => {
  it("lists agents from the context, alive first then by name, with stats", async () => {
    const { container } = show();
    await findRow("b");
    const names = within(screen.getByRole("list", { name: "agents" }))
      .getAllByRole("button")
      .map((b) => b.dataset.agent);
    expect(names).toEqual(["b", "c", "a"]);
    const stats = [...container.querySelectorAll(".th-trace-stat")].map((s) => s.textContent);
    expect(stats).toEqual(["agents3", "running2", "ended1"]);
    expect(screen.getByRole("heading", { level: 1, name: "control center" })).toBeInTheDocument();
    expect(screen.getByText("3 agents")).toBeInTheDocument();
    expect(container.querySelector(".th-control")).not.toHaveAttribute("data-pane");
  });

  it("shows loading agents… until the first list arrives", async () => {
    list.mockReturnValue(new Promise<readonly Agent[]>(() => undefined));
    show();
    expect(await screen.findByText("loading agents…", {}, { timeout: 1000 })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "agents" })).toBeNull();
  });

  it("shows the empty state without stats or row hints", async () => {
    list.mockResolvedValue([]);
    const { container } = show();
    expect(await screen.findByRole("heading", { name: "no agents yet" })).toBeInTheDocument();
    expect(screen.getByText("start one with new agent…")).toBeInTheDocument();
    expect(container.querySelector(".th-trace-stats")).toBeNull();
    expect(screen.getByText("0 agents")).toBeInTheDocument();
    expect(hints()).toHaveTextContent(/^$/);
  });
});

describe("ControlCenterPage pane", () => {
  it("opens one pane at a time and keeps it in the url", async () => {
    const { user, container } = show();
    await user.click(await findRow("b"));
    expect(pane("b")).toBeInTheDocument();
    expect(url()).toBe("/townhall?agent=b");
    expect(row("b")).toHaveAttribute("aria-current", "true");
    expect(container.querySelector(".th-control")).toHaveAttribute("data-pane", "true");
    expect(container.querySelector(".th-control")).toHaveClass("th-convo-layout");
    expect(hints()).toHaveTextContent("↑↓ rows | ⏎ open agent | esc close pane");
    row("c").focus();
    await user.keyboard("{Enter}");
    expect(pane("c")).toBeInTheDocument();
    expect(pane("b")).toBeNull();
    expect(screen.getAllByRole("article")).toHaveLength(1);
    expect(url()).toBe("/townhall?agent=c");
  });

  it("closes the pane and returns focus to its row", async () => {
    const { user } = show();
    await user.click(await findRow("c"));
    await user.click(screen.getByRole("button", { name: "close" }));
    expect(pane("c")).toBeNull();
    expect(url()).toBe("/townhall");
    expect(row("c")).toHaveFocus();
    expect(hints()).toHaveTextContent("↑↓ rows | ⏎ open agent");
  });

  it("closes the pane on esc, but not from a text field", async () => {
    window.history.replaceState(null, "", "/townhall?agent=b");
    const { user } = show();
    await findRow("b");
    await user.click(screen.getByRole("button", { name: "new agent…" }));
    fireEvent.keyDown(screen.getByLabelText("name"), { key: "Escape" });
    expect(pane("b")).toBeInTheDocument();
    act(() => (document.activeElement as HTMLElement).blur());
    await user.keyboard("{Escape}");
    expect(pane("b")).toBeNull();
    expect(row("b")).toHaveFocus();
  });

  it("opens the agent named in the url on load and ignores an unknown one", async () => {
    window.history.replaceState(null, "", "/townhall?agent=a");
    show();
    await findRow("a");
    expect(pane("a")).toBeInTheDocument();
    act(() => window.history.replaceState(null, "", "/townhall?agent=zz"));
    act(() => window.dispatchEvent(new PopStateEvent("popstate")));
    expect(screen.queryByRole("article")).toBeNull();
  });

  it("refreshes and closes when the agent is stopped", async () => {
    const { user } = show();
    await user.click(await findRow("b"));
    list.mockResolvedValue([mk("a", false), mk("c")]);
    const calls = list.mock.calls.length;
    await user.click(screen.getByRole("button", { name: "stopped" }));
    expect(pane("b")).toBeNull();
    expect(url()).toBe("/townhall");
    await waitFor(() => expect(list.mock.calls.length).toBe(calls + 1));
    await waitFor(() => expect(row("c")).toHaveFocus());
  });

  it("moves focus to the rows when an arrow key is pressed on the page", async () => {
    show();
    await findRow("b");
    fireEvent.keyDown(document.body, { key: "ArrowDown" });
    expect(row("b")).toHaveFocus();
  });
});

describe("ControlCenterPage new agent", () => {
  it("unfolds the form, folds it after a create and opens the new agent", async () => {
    vi.spyOn(client, "createAgent").mockResolvedValue(mk("d"));
    const { user } = show();
    await findRow("b");
    const toggle = screen.getByRole("button", { name: "new agent…" });
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    list.mockResolvedValue([mk("b"), mk("a", false), mk("c"), mk("d")]);
    await user.type(screen.getByLabelText("name"), "d");
    await user.click(screen.getByRole("button", { name: "start" }));
    expect(await screen.findByRole("article", { name: "d terminal" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "new agent" })).toBeNull();
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(url()).toBe("/townhall?agent=d");
    await waitFor(() => expect(row("d")).toHaveFocus());
  });

  it("folds the form again from the header button", async () => {
    const { user } = show();
    await findRow("b");
    const toggle = screen.getByRole("button", { name: "new agent…" });
    await user.click(toggle);
    expect(screen.getByRole("form", { name: "new agent" })).toBeInTheDocument();
    await user.click(toggle);
    expect(screen.queryByRole("form", { name: "new agent" })).toBeNull();
  });
});
