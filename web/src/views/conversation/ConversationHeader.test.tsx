import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { aConversation, aDetail, aTask } from "../../api/fixtures";
import type { ConversationDetail } from "../../api/types";
import { ConversationHeader, type ConversationHeaderProps } from "./ConversationHeader";

function renderHeader(detail: ConversationDetail, over: Partial<ConversationHeaderProps> = {}) {
  const props: ConversationHeaderProps = {
    detail,
    repo: "~ townhall · main",
    railOpen: true,
    onToggleRail: vi.fn(),
    onStop: vi.fn(async () => undefined),
    onEnd: vi.fn(async () => undefined),
    ...over,
  };
  render(<ConversationHeader {...props} />);
  return props;
}

const titled = (over = {}) => aConversation({ title: "login redirect loop", ...over });

describe("ConversationHeader", () => {
  it("shows the title, repo and status", () => {
    renderHeader(aDetail({ conversation: titled() }));
    expect(screen.getByRole("heading", { level: 2, name: "login redirect loop" })).toBeInTheDocument();
    expect(screen.getByText("~ townhall · main")).toHaveClass("th-convo-head-repo");
    expect(screen.getByText("your turn")).toHaveAttribute("data-status", "yourTurn");
    expect(screen.queryByRole("button", { name: "stop" })).toBeNull();
  });

  it("offers stop with a spinner while thinking", async () => {
    const props = renderHeader(aDetail({ conversation: titled({ status: "thinking" }) }));
    const status = screen.getByText("thinking");
    expect(status.querySelector(".th-spin")).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "stop" }));
    expect(props.onStop).toHaveBeenCalled();
  });

  it("says ended and hides end once ended", () => {
    renderHeader(aDetail({ conversation: titled({ ended: true }) }));
    expect(screen.getByText("ended")).toHaveAttribute("data-status", "ended");
    expect(screen.queryByRole("button", { name: "end" })).toBeNull();
  });

  it("toggles the session panel and shows who needs you on it", async () => {
    const tasks = [aTask({ name: "a", state: "needsYou" }), aTask({ name: "b", state: "needsYou" })];
    const props = renderHeader(aDetail({ conversation: titled(), tasks }), { railOpen: false });
    const toggle = screen.getByRole("button", { name: "2 · session" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveClass("th-rail-toggle");
    expect(toggle).toHaveAttribute("aria-controls", "th-session-panel");
    expect(toggle.querySelector(".th-status[data-state='waiting']")).not.toBeNull();
    await userEvent.click(toggle);
    expect(props.onToggleRail).toHaveBeenCalled();
  });

  it("ends at once without unfinished tasks", async () => {
    const props = renderHeader(aDetail({ conversation: titled(), tasks: [aTask({ state: "merged" })] }));
    await userEvent.click(screen.getByRole("button", { name: "end" }));
    expect(props.onEnd).toHaveBeenCalled();
  });

  it("asks first when tasks keep running, and esc cancels", async () => {
    const tasks = [aTask({ name: "a", state: "working" }), aTask({ name: "b", state: "queued" })];
    const props = renderHeader(aDetail({ conversation: titled(), tasks }));
    const end = screen.getByRole("button", { name: "end" });
    await userEvent.click(end);
    expect(screen.getByText("end login redirect loop? 2 tasks keep running")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "cancel" })).toHaveFocus();
    expect(props.onEnd).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByText(/keep running/)).toBeNull();
    expect(screen.getByRole("button", { name: "end" })).toHaveFocus();
    await userEvent.click(screen.getByRole("button", { name: "end" }));
    await userEvent.click(screen.getByRole("button", { name: "end" }));
    expect(props.onEnd).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/keep running/)).toBeNull();
  });
});
