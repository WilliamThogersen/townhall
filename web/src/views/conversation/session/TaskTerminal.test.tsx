import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StreamState } from "../../../agents/useAgentStream";
import * as client from "../../../api/client";
import * as conversationsApi from "../../../api/conversations";
import { aDetail, anAgentInfo, aTask } from "../../../api/fixtures";
import type { AgentInfo, Task } from "../../../api/types";
import { createFakeEvents, TestDataProvider } from "../../../data/testing";
import { renderWithHints } from "../../../test/renderWithHints";
import { TaskTerminal } from "./index";

const stream = vi.hoisted(() => ({ state: { snapshot: "$ ready", status: "open", error: null } as StreamState }));

vi.mock("../../../agents/useAgentStream", () => ({ useAgentStream: () => stream.state }));

vi.mock("../../../agents/Terminal", () => ({
  Terminal: ({ snapshot }: { snapshot: string }) => <pre data-testid="terminal">{snapshot}</pre>,
}));

const login = aTask({ name: "fix-login", state: "needsYou", repo: "shop" });
const cookie = aTask({ name: "logout-cookie", state: "queued", after: ["fix-login"], repo: "shop" });

function stubDaemon(agents: readonly AgentInfo[] | Promise<readonly AgentInfo[]>, tasks: readonly Task[] = [login, cookie]) {
  vi.spyOn(client, "listAgents").mockImplementation(() => Promise.resolve(agents));
  vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(aDetail({ tasks }));
}

function renderTerminal(task: Task) {
  const onClose = vi.fn();
  const events = createFakeEvents();
  const view = renderWithHints(
    <TestDataProvider events={events}>
      <input aria-label="Message" />
      <TaskTerminal task={task} onClose={onClose} />
    </TestDataProvider>,
  );
  return { ...view, onClose, events };
}

const pane = (name: string) => screen.getByRole("article", { name: `${name} terminal` });

beforeEach(() => {
  stream.state = { snapshot: "$ ready", status: "open", error: null };
});

afterEach(() => vi.restoreAllMocks());

describe("TaskTerminal with a live agent", () => {
  beforeEach(() => stubDaemon([anAgentInfo({ name: "fix-login", repo: "shop" })]));

  it("shows the agent pane inside the panes column", async () => {
    renderTerminal(login);
    expect(await screen.findByTestId("terminal")).toHaveTextContent("$ ready");
    expect(pane("fix-login").parentElement).toHaveClass("th-panes");
    expect(pane("fix-login")).toHaveAttribute("data-active", "true");
    expect(pane("fix-login").querySelector(".th-pane-branch")).toHaveTextContent("townhall/fix-login");
  });

  it("closes from the close button", async () => {
    const { onClose } = renderTerminal(login);
    await screen.findByTestId("terminal");
    await userEvent.click(screen.getByRole("button", { name: "close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on esc and shows the hint", async () => {
    const { onClose } = renderTerminal(login);
    await screen.findByTestId("terminal");
    expect(document.querySelector("output")).toHaveTextContent("esc");
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("leaves esc to a field and to the terminal", async () => {
    const { onClose } = renderTerminal(login);
    const terminal = await screen.findByTestId("terminal");
    await userEvent.click(screen.getByRole("textbox", { name: "Input for fix-login" }));
    await userEvent.keyboard("{Escape}");
    fireEvent.keyDown(terminal, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });

  it("lets esc cancel the stop confirm without closing", async () => {
    const { onClose } = renderTerminal(login);
    await screen.findByTestId("terminal");
    await userEvent.click(screen.getByRole("button", { name: "stop" }));
    expect(screen.getByText("stop fix-login and remove its worktree?")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByText("stop fix-login and remove its worktree?")).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes once the agent is stopped", async () => {
    vi.spyOn(client, "stopAgent").mockResolvedValue("fix-login");
    const { onClose } = renderTerminal(login);
    await screen.findByTestId("terminal");
    await userEvent.click(screen.getByRole("button", { name: "stop" }));
    await userEvent.click(screen.getByRole("button", { name: "stop and remove" }));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });
});

describe("TaskTerminal without a live agent", () => {
  it("says what a queued task waits for", async () => {
    stubDaemon([]);
    renderTerminal(cookie);
    expect(await screen.findByRole("heading", { name: "queued: waits for fix-login" })).toBeInTheDocument();
    const head = pane("logout-cookie").querySelector(".th-pane-head")!;
    expect(head.querySelector(".th-status")).toHaveAttribute("data-state", "queued");
    expect(head.querySelector(".th-pane-branch")).toHaveTextContent("townhall/logout-cookie");
    expect(pane("logout-cookie")).toHaveAttribute("data-active", "true");
    expect(screen.queryByTestId("terminal")).toBeNull();
  });

  it("names the pull request of a merged task", async () => {
    const merged = aTask({ name: "session-ttl", state: "merged", pr: { number: 41, url: null, checks: "passed" } });
    stubDaemon([anAgentInfo({ name: "session-ttl", alive: false, state: { kind: "ended" } })], [merged]);
    renderTerminal(merged);
    expect(await screen.findByRole("heading", { name: "merged: #41" })).toBeInTheDocument();
  });

  it("closes from its close button and from esc", async () => {
    stubDaemon([]);
    const { onClose } = renderTerminal(cookie);
    await screen.findByRole("heading", { name: "queued: waits for fix-login" });
    await userEvent.click(screen.getByRole("button", { name: "close" }));
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("waits for the agent list before it says there is no terminal", async () => {
    let answer: (agents: readonly AgentInfo[]) => void = () => undefined;
    stubDaemon(new Promise((resolve) => (answer = resolve)));
    renderTerminal(login);
    expect(screen.queryByRole("heading")).toBeNull();
    await act(async () => answer([]));
    expect(await screen.findByRole("heading", { name: "needs you: no terminal yet" })).toBeInTheDocument();
  });

  it("swaps in the pane once the task's agent appears", async () => {
    stubDaemon([]);
    const { events } = renderTerminal(login);
    await screen.findByRole("heading", { name: "needs you: no terminal yet" });
    act(() => events.emit({ type: "agentUpdated", agent: anAgentInfo({ name: "fix-login", repo: "shop" }) }));
    expect(await screen.findByTestId("terminal")).toBeInTheDocument();
  });
});
