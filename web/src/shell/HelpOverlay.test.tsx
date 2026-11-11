import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HelpOverlay } from "./HelpOverlay";
import { KEY_MAP } from "./keyMap";

beforeEach(() => window.history.replaceState(null, "", "/townhall"));

describe("help overlay", () => {
  it("is a labelled modal dialog", () => {
    render(<HelpOverlay onClose={() => {}} />);
    const dialog = screen.getByRole("dialog", { name: "keyboard shortcuts" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
  });

  it("lists every group of the key map with its keys", () => {
    render(<HelpOverlay onClose={() => {}} />);
    const dialog = screen.getByRole("dialog");
    for (const group of KEY_MAP) {
      const section = within(dialog).getByRole("group", { name: group.title });
      for (const entry of group.entries) {
        expect(within(section).getAllByText(entry.label).length).toBeGreaterThan(0);
      }
    }
  });

  it("covers every context the key bar knows", () => {
    expect(KEY_MAP.map((g) => g.title)).toEqual([
      "anywhere",
      "home",
      "conversation",
      "session panel",
      "terminal",
      "control center",
      "repos",
      "agents",
      "plugins",
      "settings",
    ]);
  });

  it("closes on esc", () => {
    const onClose = vi.fn();
    render(<HelpOverlay onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes from the close button", () => {
    const onClose = vi.fn();
    render(<HelpOverlay onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "close" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("moves focus in and gives it back when it goes away", () => {
    render(<button type="button">opener</button>);
    const opener = screen.getByRole("button", { name: "opener" });
    opener.focus();
    const { unmount } = render(<HelpOverlay onClose={() => {}} />);
    expect(screen.getByRole("dialog")).toContainElement(document.activeElement as HTMLElement);
    unmount();
    expect(opener).toHaveFocus();
  });

  it("keeps tab inside", () => {
    render(<HelpOverlay onClose={() => {}} />);
    const close = screen.getByRole("button", { name: "close" });
    close.focus();
    fireEvent.keyDown(close, { key: "Tab" });
    expect(close).toHaveFocus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(close).toHaveFocus();
  });

  it("keeps other keys away from the screen behind it", () => {
    const behind = vi.fn();
    window.addEventListener("keydown", behind);
    render(<HelpOverlay onClose={() => {}} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "n" });
    window.removeEventListener("keydown", behind);
    expect(behind).not.toHaveBeenCalled();
    expect(window.location.pathname).toBe("/townhall");
  });
});
