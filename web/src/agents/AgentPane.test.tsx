import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import type { Agent } from "../api/types";
import { AgentPane } from "./AgentPane";
import { useEffect } from "react";
import type { AttachState } from "./attachSocket";
import type { TerminalProps } from "./Terminal";

const mocks = vi.hoisted(() => ({
  state: { status: "open", error: null } as AttachState,
  terminalProps: [] as unknown[],
}));

vi.mock("./Terminal", () => ({
  Terminal: ({ name, alive, focused, onState }: TerminalProps) => {
    mocks.terminalProps.push({ name, alive, focused });
    useEffect(() => onState?.(mocks.state), [onState]);
    return <pre data-testid="terminal">{name}</pre>;
  },
}));

const agent: Agent = {
  name: "u-demo",
  kind: "shell",
  branch: "townhall/u-demo",
  worktree: "/w/u-demo",
  alive: true,
};

const props = () => ({ agent, onClose: vi.fn(), onStopped: vi.fn() });

beforeEach(() => {
  mocks.state = { status: "open", error: null };
  mocks.terminalProps.length = 0;
});

afterEach(() => vi.restoreAllMocks());

const pane = () => screen.getByRole("article", { name: "u-demo terminal" });
const head = () => pane().querySelector(".th-pane-head") as HTMLElement;
const streamStatus = () => pane().querySelector(".th-stream") as HTMLElement;

describe("AgentPane head", () => {
  it("shows status, name, kind and branch", () => {
    render(<AgentPane {...props()} />);
    expect(pane().querySelector(".th-status")).toHaveAttribute("data-state", "alive");
    expect(head().querySelector(".th-pane-title")).toHaveTextContent("u-demo");
    expect(head().querySelector(".th-pane-meta")).toHaveTextContent("shell");
    expect(head().querySelector(".th-pane-branch")).toHaveTextContent("townhall/u-demo");
    expect(head().querySelector(".th-sr-only")).toHaveTextContent("alive");
  });

  it("leaves the kind out when the daemon does not report one", () => {
    render(<AgentPane {...props()} agent={{ ...agent, kind: null }} />);
    expect(head().querySelector(".th-pane-meta")).toBeNull();
    expect(head().querySelectorAll(".th-sep")).toHaveLength(1);
  });

  it("marks a dead agent and tells the terminal not to reconnect", async () => {
    render(<AgentPane {...props()} agent={{ ...agent, alive: false }} />);
    expect(pane().querySelector(".th-status")).toHaveAttribute("data-state", "dead");
    await screen.findByTestId("terminal");
    expect(mocks.terminalProps.at(-1)).toMatchObject({ name: "u-demo", alive: false });
  });

  it("gets the amber frame when focused", () => {
    const { rerender } = render(<AgentPane {...props()} />);
    expect(pane()).toHaveAttribute("data-active", "false");
    rerender(<AgentPane {...props()} focused />);
    expect(pane()).toHaveAttribute("data-active", "true");
  });

  it("calls onClose from close", async () => {
    const p = props();
    render(<AgentPane {...p} />);
    await userEvent.click(screen.getByRole("button", { name: "close" }));
    expect(p.onClose).toHaveBeenCalledTimes(1);
  });

  it("gives the terminal the agent, with no separate input row", async () => {
    render(<AgentPane {...props()} focused />);
    expect(await screen.findByTestId("terminal")).toHaveTextContent("u-demo");
    expect(mocks.terminalProps.at(-1)).toEqual({ name: "u-demo", alive: true, focused: true });
    expect(screen.queryByPlaceholderText(/type into/)).toBeNull();
  });
});

describe("AgentPane stream status", () => {
  it.each([
    [{ status: "open", error: null }, "open", "● live"],
    [{ status: "connecting", error: null }, "connecting", "connecting"],
    [{ status: "reconnecting", error: null }, "connecting", "reconnecting"],
    [{ status: "closed", error: "agent `u-demo` not found" }, "ended", "○ ended: session closed"],
    [{ status: "closed", error: "capture failed" }, "error", "× error: capture failed"],
  ] as const)("renders %o as %s", async (state, status, text) => {
    mocks.state = state;
    render(<AgentPane {...props()} />);
    await screen.findByTestId("terminal");
    expect(streamStatus()).toHaveAttribute("data-status", status);
    expect(streamStatus()).toHaveTextContent(text);
  });

  it("spins while connecting", () => {
    mocks.state = { status: "connecting", error: null };
    render(<AgentPane {...props()} />);
    expect(streamStatus().querySelector(".th-spin")).not.toBeNull();
  });

  it("hides the glyph from screen readers", () => {
    render(<AgentPane {...props()} />);
    expect(streamStatus().querySelector('[aria-hidden="true"]')).toHaveTextContent("●");
  });
});

describe("AgentPane stop", () => {
  const openConfirm = async () => {
    await userEvent.click(screen.getByRole("button", { name: "stop" }));
  };

  it("asks in place with cancel focused", async () => {
    render(<AgentPane {...props()} />);
    await openConfirm();
    expect(head()).toHaveAttribute("data-confirm", "true");
    expect(head().querySelector(".th-confirm")).toHaveTextContent("stop u-demo and remove its worktree?");
    expect(screen.getByRole("button", { name: "cancel" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "stop and remove" })).toHaveClass("th-btn--danger-solid");
    expect(head().querySelector(".th-stream")).toBeNull();
    expect(screen.queryByRole("button", { name: "close" })).toBeNull();
  });

  it("cancel closes the question and returns focus to stop", async () => {
    render(<AgentPane {...props()} />);
    await openConfirm();
    await userEvent.click(screen.getByRole("button", { name: "cancel" }));
    expect(head()).not.toHaveAttribute("data-confirm");
    expect(screen.getByRole("button", { name: "stop" })).toHaveFocus();
  });

  it("esc cancels and does not reach outer handlers", async () => {
    const outer = vi.fn();
    document.addEventListener("keydown", outer);
    render(<AgentPane {...props()} />);
    await openConfirm();
    await userEvent.keyboard("{Escape}");
    document.removeEventListener("keydown", outer);
    expect(head()).not.toHaveAttribute("data-confirm");
    expect(outer).not.toHaveBeenCalled();
  });

  it("stops the agent then calls onStopped", async () => {
    let release: (value: string) => void = () => {};
    const stop = vi
      .spyOn(client, "stopAgent")
      .mockImplementation(() => new Promise((resolve) => (release = resolve)));
    const p = props();
    render(<AgentPane {...p} />);
    await openConfirm();
    await userEvent.click(screen.getByRole("button", { name: "stop and remove" }));
    expect(stop).toHaveBeenCalledWith("u-demo");
    expect(screen.getByRole("button", { name: /stopping…/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: "cancel" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    expect(head()).toHaveAttribute("data-confirm", "true");
    expect(p.onStopped).not.toHaveBeenCalled();
    release("stopped");
    await waitFor(() => expect(p.onStopped).toHaveBeenCalledTimes(1));
  });

  it("shows the api error and lets you retry", async () => {
    const stop = vi.spyOn(client, "stopAgent").mockRejectedValueOnce(new Error("worktree is dirty"));
    const p = props();
    render(<AgentPane {...p} />);
    await openConfirm();
    await userEvent.click(screen.getByRole("button", { name: "stop and remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: worktree is dirty");
    expect(screen.getByRole("button", { name: "cancel" })).toHaveFocus();
    expect(p.onStopped).not.toHaveBeenCalled();
    stop.mockResolvedValue("stopped");
    await userEvent.click(screen.getByRole("button", { name: "stop and remove" }));
    await waitFor(() => expect(p.onStopped).toHaveBeenCalledTimes(1));
  });
});
