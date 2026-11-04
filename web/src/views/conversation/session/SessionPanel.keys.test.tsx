import { fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { aDetail, aTask } from "../../../api/fixtures";
import type { Task } from "../../../api/types";
import { renderWithHints, shownHints } from "../../../test/renderWithHints";
import { SessionPanel } from "./index";

const tasks: readonly Task[] = [
  aTask({ name: "one", state: "needsYou" }),
  aTask({ name: "two", state: "working" }),
  aTask({ name: "three", state: "queued" }),
];

function renderPanel(openTask: string | null = null, list: readonly Task[] = tasks) {
  const onOpenTask = vi.fn();
  const view = renderWithHints(
    <>
      <input aria-label="Message" />
      <SessionPanel detail={aDetail({ tasks: list })} openTask={openTask} onOpenTask={onOpenTask} />
    </>,
  );
  return { ...view, onOpenTask };
}

const row = (name: string) => document.querySelector<HTMLButtonElement>(`button[data-task="${name}"]`)!;

describe("SessionPanel keys", () => {
  it("shows the task keys in the key bar while there are tasks", () => {
    const { unmount } = renderPanel();
    expect(shownHints()).toBe("j k ⏎");
    unmount();
    renderPanel(null, []);
    expect(shownHints()).toBe("");
  });

  it("moves through the rows with j and k and stops at the ends", async () => {
    renderPanel();
    await userEvent.keyboard("j");
    expect(row("one")).toHaveFocus();
    await userEvent.keyboard("jj");
    expect(row("three")).toHaveFocus();
    await userEvent.keyboard("j");
    expect(row("three")).toHaveFocus();
    await userEvent.keyboard("kkk");
    expect(row("one")).toHaveFocus();
  });

  it("starts from the open task", async () => {
    renderPanel("two");
    await userEvent.keyboard("k");
    expect(row("two")).toHaveFocus();
  });

  it("opens the focused task with enter", async () => {
    const { onOpenTask } = renderPanel();
    await userEvent.keyboard("jj{Enter}");
    expect(onOpenTask).toHaveBeenCalledWith("two");
  });

  it("leaves j and k to a text field", async () => {
    renderPanel();
    await userEvent.click(screen.getByRole("textbox", { name: "Message" }));
    await userEvent.keyboard("j");
    expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("j");
    expect(row("one")).not.toHaveFocus();
  });

  it("ignores j with a modifier or inside a dialog", () => {
    renderPanel();
    fireEvent.keyDown(window, { key: "j", metaKey: true });
    expect(row("one")).not.toHaveFocus();
    const dialog = document.createElement("dialog");
    const inside = document.createElement("button");
    dialog.append(inside);
    document.body.append(dialog);
    fireEvent.keyDown(inside, { key: "j" });
    expect(row("one")).not.toHaveFocus();
    dialog.remove();
  });

  it("moves with the arrow keys once a row has focus", async () => {
    renderPanel();
    row("one").focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("two")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(row("one")).toHaveFocus();
  });

  it("keeps one tab stop in the list, on the row last focused", async () => {
    renderPanel("three");
    expect(row("three")).toHaveAttribute("tabindex", "0");
    expect(row("one")).toHaveAttribute("tabindex", "-1");
    await userEvent.keyboard("kkk");
    expect(row("one")).toHaveAttribute("tabindex", "0");
    expect(row("three")).toHaveAttribute("tabindex", "-1");
  });

  it("stops listening once unmounted", async () => {
    const { unmount } = renderPanel();
    unmount();
    await userEvent.keyboard("j");
    expect(document.activeElement).toBe(document.body);
  });
});
