import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import type { Agent } from "../../api/types";
import { AgentRows } from "./AgentRows";
import { focusAgentRow, focusCurrentRow } from "./rowFocus";

const mk = (name: string, alive = true, kind: string | null = "shell"): Agent => ({
  name,
  kind,
  branch: `townhall/${name}`,
  worktree: `/w/${name}`,
  alive,
});

const AGENTS = [mk("a"), mk("b"), mk("c", false, null)];

function show(openName: string | null = null, onOpen = vi.fn()) {
  const listRef = createRef<HTMLUListElement>();
  render(<AgentRows agents={AGENTS} openName={openName} onOpen={onOpen} listRef={listRef} />);
  return { onOpen, listRef };
}

const row = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name} `) });

describe("AgentRows", () => {
  it("renders one trace row per agent with glyph, bold name, kind and branch", () => {
    const { listRef } = show();
    const list = screen.getByRole("list", { name: "agents" });
    expect(list).toBe(listRef.current);
    expect(list).toHaveClass("th-list", "th-control-rows");
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveClass("th-span");
    expect(items[0]).toHaveAttribute("data-depth", "1");
    const first = row("a");
    expect(first).toHaveClass("th-span-label");
    expect(first.querySelector(".th-status")).toHaveAttribute("data-state", "alive");
    expect(first.querySelector(".th-span-name")).toHaveTextContent("a");
    expect([...first.querySelectorAll(".th-span-meta")].map((m) => m.textContent)).toEqual([
      "shell",
      "townhall/a",
    ]);
    expect(first).toHaveAccessibleName("a shell townhall/a running");
  });

  it("leaves the kind out when there is none and says ended for a dead agent", () => {
    show();
    const ended = row("c");
    expect(ended.querySelectorAll(".th-span-meta")).toHaveLength(1);
    expect(ended.querySelector(".th-status")).toHaveAttribute("data-state", "dead");
    expect(ended).toHaveAccessibleName("c townhall/c ended");
  });

  it("marks only the open row as current and makes it the tab stop", () => {
    show("b");
    expect(row("b")).toHaveAttribute("aria-current", "true");
    expect(row("a")).not.toHaveAttribute("aria-current");
    expect(row("b")).toHaveAttribute("tabindex", "0");
    expect(row("a")).toHaveAttribute("tabindex", "-1");
    expect(row("c")).toHaveAttribute("tabindex", "-1");
  });

  it("makes the first row the tab stop when nothing is open", () => {
    show(null);
    expect(row("a")).toHaveAttribute("tabindex", "0");
    expect(row("b")).toHaveAttribute("tabindex", "-1");
  });

  it("opens a row on click and on enter", async () => {
    const user = userEvent.setup();
    const { onOpen } = show();
    await user.click(row("b"));
    expect(onOpen).toHaveBeenLastCalledWith("b");
    row("c").focus();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenLastCalledWith("c");
  });

  it("moves focus with the arrow keys and stops at the ends", async () => {
    const user = userEvent.setup();
    show();
    row("a").focus();
    await user.keyboard("{ArrowDown}");
    expect(row("b")).toHaveFocus();
    expect(row("b")).toHaveAttribute("tabindex", "0");
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(row("c")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(row("b")).toHaveFocus();
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(row("a")).toHaveFocus();
  });

  it("ignores other keys and arrows from outside a row", () => {
    const { listRef } = show();
    row("a").focus();
    fireEvent.keyDown(row("a"), { key: "x" });
    expect(row("a")).toHaveFocus();
    const stray = fireEvent.keyDown(listRef.current as HTMLUListElement, { key: "ArrowDown" });
    expect(stray).toBe(true);
  });
});

describe("rowFocus", () => {
  it("focuses a row by name and reports a miss", () => {
    const { listRef } = show();
    expect(focusAgentRow(listRef.current, "b")).toBe(true);
    expect(row("b")).toHaveFocus();
    expect(focusAgentRow(listRef.current, "zz")).toBe(false);
    expect(focusAgentRow(null, "b")).toBe(false);
  });

  it("focuses the tab stop row", () => {
    const { listRef } = show("b");
    expect(focusCurrentRow(listRef.current)).toBe(true);
    expect(row("b")).toHaveFocus();
    expect(focusCurrentRow(null)).toBe(false);
  });
});
