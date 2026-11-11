import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { anEntry } from "../../../api/fixtures";
import type { EntryBody, ThreadEntry } from "../../../api/types";
import { Thread } from "./Thread";
import { ThreadContext, type ThreadContextValue } from "./threadContext";

const at = new Date(2026, 10, 4, 10, 42).toISOString();
const entry = (id: number, body: EntryBody) => anEntry(id, body, at);

function renderThread(entries: readonly ThreadEntry[], over: Partial<ThreadContextValue> = {}) {
  const value: ThreadContextValue = {
    conversationId: "c1",
    tasks: [],
    attachments: [],
    thinking: false,
    openTask: null,
    onOpenTask: vi.fn(),
    onReply: vi.fn(),
    sendMessage: vi.fn(async () => true),
    answerAgent: vi.fn(async () => true),
    answerKeys: vi.fn(async () => true),
    allowTool: vi.fn(async () => true),
    ...over,
  };
  render(
    <ThreadContext value={value}>
      <Thread entries={entries} ended={false} />
    </ThreadContext>,
  );
  return value;
}

describe("Thread reply button", () => {
  it("hands the entry to onReply from a message, an event line and a question", async () => {
    const user = userEvent.setup();
    const { onReply } = renderThread([
      entry(1, { kind: "you", text: "fix it", attachments: [] }),
      entry(2, { kind: "townhall", text: "which repo?" }),
      entry(3, { kind: "agent", agent: "fix-login", event: "failed", text: "tests red" }),
      entry(4, { kind: "needsYou", agent: "fix-login", question: "commit? (y/n)", choices: ["y", "n"] }),
      entry(5, { kind: "repoAsk", question: "shop or admin?", candidates: ["shop", "admin"], offerAll: false }),
    ]);
    await user.click(screen.getByRole("button", { name: "Reply to you: fix it" }));
    expect(onReply).toHaveBeenLastCalledWith({ id: 1, author: "you", excerpt: "fix it" });
    await user.click(screen.getByRole("button", { name: "Reply to townhall: which repo?" }));
    expect(onReply).toHaveBeenLastCalledWith({ id: 2, author: "townhall", excerpt: "which repo?" });
    await user.click(screen.getByRole("button", { name: "Reply to fix-login: tests red" }));
    expect(onReply).toHaveBeenLastCalledWith({ id: 3, author: "fix-login", excerpt: "tests red" });
    await user.click(screen.getByRole("button", { name: "Reply to fix-login: commit? (y/n)" }));
    expect(onReply).toHaveBeenLastCalledWith({ id: 4, author: "fix-login", excerpt: "commit? (y/n)" });
    await user.click(screen.getByRole("button", { name: "Reply to townhall: shop or admin?" }));
    expect(onReply).toHaveBeenLastCalledWith({ id: 5, author: "townhall", excerpt: "shop or admin?" });
  });

  it("is not offered on entries there is nothing to quote from", () => {
    renderThread([
      entry(1, { kind: "repo", repo: "shop", baseBranch: "main", rule: "only", reason: "" }),
      entry(2, { kind: "tasksCreated", tasks: ["a"] }),
    ]);
    expect(screen.queryByRole("button", { name: /^Reply to/ })).not.toBeInTheDocument();
  });
});

describe("Thread quoted reply", () => {
  const scrollIntoView = vi.fn();
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    Element.prototype.scrollIntoView = scrollIntoView;
  });
  afterEach(() => {
    vi.useRealTimers();
    scrollIntoView.mockClear();
  });

  const thread = [
    entry(1, { kind: "townhall", text: "which repo should it go in?" }),
    entry(2, { kind: "you", text: "the shop one", attachments: [], replyTo: 1 }),
  ];

  it("shows the quoted snippet above a sent reply", () => {
    renderThread(thread);
    const quote = screen.getByRole("button", { name: "Go to the message you replied to" });
    expect(quote).toHaveTextContent("townhall");
    expect(quote).toHaveTextContent("which repo should it go in?");
    const message = quote.closest(".th-msg");
    expect(message).toHaveTextContent("the shop one");
  });

  it("scrolls to the original and highlights it briefly", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderThread(thread);
    await user.click(screen.getByRole("button", { name: "Go to the message you replied to" }));
    const original = document.querySelector('[data-entry-id="1"]');
    expect(original).not.toBeNull();
    expect(scrollIntoView).toHaveBeenCalledOnce();
    expect(scrollIntoView.mock.contexts[0]).toBe(original);
    expect(original).toHaveAttribute("data-flash", "true");
    act(() => void vi.advanceTimersByTime(2000));
    expect(original).not.toHaveAttribute("data-flash");
  });

  it("jumps without smooth scrolling when the user prefers reduced motion", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query === "(prefers-reduced-motion: reduce)" }));
    renderThread(thread);
    await user.click(screen.getByRole("button", { name: "Go to the message you replied to" }));
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "center", behavior: "auto" });
    vi.unstubAllGlobals();
  });

  it("says so when the original is not in the thread", () => {
    renderThread([entry(2, { kind: "you", text: "the shop one", attachments: [], replyTo: 99 })]);
    expect(screen.queryByRole("button", { name: "Go to the message you replied to" })).not.toBeInTheDocument();
    const message = screen.getByText("the shop one").closest(".th-msg") as HTMLElement;
    expect(within(message).getByText("original message not available")).toBeInTheDocument();
  });

  it("says so when the original has nothing to quote", () => {
    renderThread([
      entry(1, { kind: "repo", repo: "shop", baseBranch: "main", rule: "only", reason: "" }),
      entry(2, { kind: "you", text: "ok", attachments: [], replyTo: 1 }),
    ]);
    expect(screen.getByText("original message not available")).toBeInTheDocument();
  });

  it("shows no quote on a message that replies to nothing", () => {
    renderThread([entry(1, { kind: "you", text: "hi", attachments: [] })]);
    expect(screen.queryByText("original message not available")).not.toBeInTheDocument();
  });
});
