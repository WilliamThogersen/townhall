import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { anEntry, aTask, aYouEntry } from "../../../api/fixtures";
import type { EntryBody, ThreadEntry } from "../../../api/types";
import { Thread } from "./Thread";
import { ThreadContext, type ThreadContextValue } from "./threadContext";

function context(over: Partial<ThreadContextValue> = {}): ThreadContextValue {
  return {
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
}

function renderThread(entries: readonly ThreadEntry[], over: Partial<ThreadContextValue> = {}, ended = false) {
  const value = context(over);
  const view = render(
    <ThreadContext value={value}>
      <Thread entries={entries} ended={ended} />
    </ThreadContext>,
  );
  const rerender = (next: readonly ThreadEntry[]) =>
    view.rerender(
      <ThreadContext value={value}>
        <Thread entries={next} ended={ended} />
      </ThreadContext>,
    );
  return { value, rerender };
}

const at = new Date(2026, 10, 4, 10, 42).toISOString();
const entry = (id: number, body: EntryBody) => anEntry(id, body, at);

describe("Thread messages", () => {
  it("replaces a pasted image marker with the thumbnail", () => {
    const shot = { id: "i1", conversationId: "c1", name: "shot.png", mime: "image/png", size: 2048, kind: "image", createdAt: at } as const;
    const { container } = render(
      <ThreadContext value={context({ attachments: [shot] })}>
        <Thread entries={[entry(1, { kind: "you", text: "why red?\n[Image #1]", attachments: ["i1"] })]} ended={false} />
      </ThreadContext>,
    );
    expect(container.querySelector(".th-msg-body")).not.toHaveTextContent("[Image #1]");
    expect(container.querySelector(".th-msg-body")).toHaveTextContent("why red?");
    expect(container.querySelector(".th-msg-images .th-thumb")).not.toBeNull();
  });

  it("shows your message with its time and townhall's reply as markdown", () => {
    renderThread([entry(1, { kind: "you", text: "fix it\nplease", attachments: [] }), entry(2, { kind: "townhall", text: "on it: `a.ts`" })]);
    const log = screen.getByRole("log", { name: "Thread" });
    const [you, reply] = log.querySelectorAll(".th-msg");
    expect(you).toHaveClass("th-msg--you");
    expect(you?.querySelector(".th-msg-body")?.innerHTML).toBe("<p>fix it<br>please</p>");
    expect(you?.querySelector("time")).toHaveTextContent("10:42");
    expect(you).not.toHaveClass("th-msg--townhall");
    expect(reply).not.toHaveClass("th-msg--you");
    expect(reply).toHaveClass("th-msg--townhall");
    expect(reply?.querySelector("code")).toHaveTextContent("a.ts");
  });

  it("writes repo events with the reason", () => {
    renderThread([
      entry(1, { kind: "repo", repo: "shop", baseBranch: "develop", rule: "code", reason: "it has src/search/query.ts" }),
      entry(2, { kind: "repo", repo: "townhall", baseBranch: "main", rule: "named", reason: "" }),
    ]);
    const events = document.querySelectorAll(".th-event[data-kind='repo']");
    expect(events[0]).toHaveTextContent("~repo shop · develop, it has src/search/query.ts10:42");
    expect(events[1]).toHaveTextContent("repo townhall · main, you named it");
  });

  it("writes agent lines and errors with their glyphs", () => {
    renderThread([
      entry(1, { kind: "agent", agent: "fix-login", event: "sent", text: "y" }),
      entry(2, { kind: "agent", agent: "fix-login", event: "passed", text: "merged into main" }),
      entry(3, { kind: "agent", agent: "auth", event: "failed", text: "exited with code 1" }),
      entry(4, { kind: "agent", agent: "auth", event: "ended", text: "" }),
      entry(5, { kind: "error", message: "claude exited" }),
    ]);
    const lines = [...document.querySelectorAll(".th-event")].map((line) => line.textContent);
    expect(lines).toEqual([
      "→sent to fix-login: y10:42",
      "✓fix-login merged into main10:42",
      "×auth exited with code 110:42",
      "○auth ended10:42",
      "×error: claude exited10:42",
    ]);
    expect(document.querySelector(".th-event[data-event='failed']")).not.toBeNull();
  });

  it("writes a task event as a system line, not a message from you", () => {
    renderThread([entry(1, { kind: "event", text: "task fix ended (ended)" })]);
    const line = document.querySelector(".th-event[data-kind='event']");
    expect(line).toHaveTextContent("◇task fix ended (ended)10:42");
    expect(document.querySelector(".th-msg-you, [data-from='you']")).toBeNull();
  });

  it("shows a plugin's entry with the plugin as its author, never as you", () => {
    renderThread([
      entry(1, { kind: "plugin", plugin: "lint@acme", text: "created by plugin lint@acme: task `fix`" }),
    ]);
    const line = document.querySelector(".th-event[data-kind='plugin']");
    expect(line).toHaveTextContent("plugin lint@acme: created by plugin lint@acme: task `fix`10:42");
    expect(document.querySelector(".th-msg--you")).toBeNull();
  });

  it("writes a handoff with who handed to whom and why", () => {
    renderThread([
      entry(1, { kind: "handoff", task: "fix-login", from: "shipper", to: "coder", reason: "ci fails on lint" }),
    ]);
    const line = document.querySelector(".th-event[data-kind='handoff']");
    expect(line).toHaveTextContent("↩fix-login: shipper handed back to coder, ci fails on lint10:42");
  });

  it("shows a thinking row while townhall thinks, else the ended note", () => {
    renderThread([aYouEntry(1, "hi")], { thinking: true }, true);
    expect(screen.getByRole("status")).toHaveTextContent("thinking");
    expect(screen.queryByText("info: ended. a message continues it")).toBeNull();
  });

  it("notes an ended conversation", () => {
    renderThread([aYouEntry(1, "hi")], {}, true);
    expect(screen.getByText("info: ended. a message continues it")).toBeInTheDocument();
  });
});

describe("Thread task block", () => {
  const tasks = [
    aTask({ name: "fix-login", title: "clear the session", repo: "townhall", baseBranch: "main", state: "needsYou" }),
    aTask({ name: "auth-tests", title: "cover expiry", repo: "townhall", state: "working" }),
    aTask({ name: "logout", title: "clear the cookie", repo: "townhall", state: "queued", after: ["fix-login"] }),
  ];
  const block = entry(3, { kind: "tasksCreated", tasks: ["fix-login", "auth-tests", "logout", "unknown"] });

  it("lists each task with its glyph, words and title", () => {
    renderThread([block], { tasks });
    const group = screen.getByRole("group", { name: "4 tasks for townhall · main" });
    const lines = group.querySelectorAll(".th-agent-line");
    expect([...lines].map((line) => line.querySelector(".th-status")?.getAttribute("data-state"))).toEqual([
      "waiting",
      "working",
      "queued",
      "queued",
    ]);
    expect(lines[0]).toHaveTextContent("fix-loginneeds youclear the sessionopen");
    expect(lines[2]).toHaveTextContent("logoutafter fix-loginclear the cookieopen");
    expect(lines[1]?.querySelector(".th-spin")).not.toBeNull();
  });

  it("lines up with the replies and keeps its time in its head", () => {
    renderThread([block], { tasks });
    const group = screen.getByRole("group", { name: "4 tasks for townhall · main" });
    const message = group.closest(".th-msg");
    expect(message).toHaveClass("th-msg--block");
    expect(group.querySelector(".th-block-head time")).not.toBeNull();
    expect(message?.querySelectorAll("time")).toHaveLength(1);
  });

  it("opens a task's terminal and marks the open one", async () => {
    const { value } = renderThread([block], { tasks, openTask: "auth-tests" });
    expect(screen.getByRole("button", { name: "open auth-tests" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(screen.getByRole("button", { name: "open fix-login" }));
    expect(value.onOpenTask).toHaveBeenCalledWith("fix-login");
  });
});

describe("Thread needs-you block", () => {
  const ask = entry(4, { kind: "needsYou", agent: "fix-login", question: "commit the fix? (y/n)", choices: ["y", "n"] });
  const tasks = [aTask({ name: "fix-login", state: "needsYou" })];

  it("quotes the question and sends a choice to the agent", async () => {
    const { value } = renderThread([ask], { tasks });
    const group = screen.getByRole("group", { name: "fix-login needs you" });
    expect(within(group).getByText("commit the fix? (y/n)")).toHaveClass("th-ask-quote");
    await userEvent.click(within(group).getByRole("button", { name: "send y" }));
    expect(value.answerAgent).toHaveBeenCalledWith("fix-login", "y");
    await userEvent.click(within(group).getByRole("button", { name: "open terminal" }));
    expect(value.onOpenTask).toHaveBeenCalledWith("fix-login");
  });

  it("shows what the adviser said when it handed the prompt over", () => {
    const advised = entry(5, {
      kind: "needsYou",
      agent: "fix-login",
      question: "Do you want to proceed?\n1. Yes\n2. No",
      choices: ["1", "2"],
      advice: "it publishes to a registry",
    });
    renderThread([advised], { tasks });
    const group = screen.getByRole("group", { name: "fix-login needs you" });
    expect(within(group).getByText("it publishes to a registry")).toHaveClass("th-ask-advice");
    expect(within(group).getByText("adviser")).toBeVisible();
  });

  it("shows no adviser line without advice", () => {
    renderThread([ask], { tasks });
    expect(screen.queryByText("adviser")).toBeNull();
  });

  it("sends a free text answer and clears it", async () => {
    const { value } = renderThread([ask], { tasks });
    const input = screen.getByRole("textbox", { name: "Answer for fix-login" });
    const send = screen.getByRole("button", { name: "send" });
    expect(send).toBeDisabled();
    await userEvent.type(input, "commit as wip{Enter}");
    expect(value.answerAgent).toHaveBeenCalledWith("fix-login", "commit as wip");
    expect(input).toHaveValue("");
  });

  it("keeps the text when the answer fails", async () => {
    renderThread([ask], { tasks, answerAgent: vi.fn(async () => false) });
    const input = screen.getByRole("textbox", { name: "Answer for fix-login" });
    await userEvent.type(input, "later{Enter}");
    expect(input).toHaveValue("later");
  });

  it("drops the answers once the agent got one and shows that answer instead of a sent line", () => {
    const sent = entry(5, { kind: "agent", agent: "fix-login", event: "sent", text: "y" });
    renderThread([ask, sent], { tasks });
    const group = screen.getByRole("group", { name: "fix-login asked" });
    expect(group).toHaveAttribute("data-answered", "true");
    expect(within(group).getByText("fix-login asked")).toBeInTheDocument();
    expect(within(group).getByText("y")).toHaveClass("th-ask-picked");
    expect(within(group).queryByRole("button", { name: "send y" })).toBeNull();
    expect(within(group).queryByRole("textbox", { name: "Answer for fix-login" })).toBeNull();
    expect(within(group).getByRole("button", { name: "open terminal" })).toBeInTheDocument();
    expect(screen.queryByText(/sent to/)).toBeNull();
  });

  it("still shows a sent line that answered no question", () => {
    const later = entry(6, { kind: "agent", agent: "fix-login", event: "sent", text: "also the logout" });
    const sent = entry(5, { kind: "agent", agent: "fix-login", event: "sent", text: "y" });
    renderThread([ask, sent, later], { tasks });
    expect(screen.getByText(/sent to/)).toHaveTextContent("sent to fix-login: also the logout");
  });
});

describe("Thread permission dialog", () => {
  const question = [
    "Bash: cd /w/web/\u2026",
    "Do you want to proceed?",
    "\u276f 1. Yes",
    "2. Yes, and don\u2019t ask again for: cd *",
    "3. Yes, and switch to auto mode \u00b7 auto mode handles these prompts for you",
    "4. No",
  ].join("\n");
  const dialog = entry(4, { kind: "needsYou", agent: "fix-login", question, choices: ["1", "2", "3", "4"] });
  const tasks = [aTask({ name: "fix-login", state: "needsYou" })];

  it("shows the command, the question and one button per option", async () => {
    const { value } = renderThread([dialog], { tasks });
    const group = screen.getByRole("group", { name: "fix-login needs you" });
    expect(within(group).getByText("Bash: cd /w/web/\u2026")).toHaveClass("th-ask-context");
    expect(within(group).getByText("Do you want to proceed?")).toHaveClass("th-ask-quote");
    const options = within(group).getAllByRole("button", { name: /^\d / });
    expect(options.map((b) => b.textContent)).toEqual([
      "1Yes",
      "2Yes, and don\u2019t ask again for: cd *",
      "3Yes, and switch to auto modeauto mode handles these prompts for you",
      "4No",
    ]);
    expect(within(group).queryByRole("textbox", { name: "Answer for fix-login" })).toBeNull();
    await userEvent.click(within(group).getByRole("button", { name: /^3 Yes, and switch to auto mode/ }));
    expect(value.answerAgent).toHaveBeenCalledWith("fix-login", "3");
  });

  it("offers to always allow the tool for the agent type, with the pattern to edit", async () => {
    const { value } = renderThread([dialog], { tasks });
    const group = screen.getByRole("group", { name: "fix-login needs you" });
    const pattern = within(group).getByRole("textbox", { name: "Pattern to always allow for builder" });
    expect(pattern).toHaveValue("Bash(cd *)");
    expect(within(group).getByText(/applies from the next launch of builder/)).toBeInTheDocument();
    await userEvent.clear(pattern);
    await userEvent.type(pattern, "Bash(cd /w/*)");
    await userEvent.click(within(group).getByRole("button", { name: "always allow for builder" }));
    expect(value.allowTool).toHaveBeenCalledWith("fix-login", "Bash(cd /w/*)");
    expect(value.answerAgent).toHaveBeenCalledWith("fix-login", "1");
  });

  it("answers allow once without saving anything", async () => {
    const { value } = renderThread([dialog], { tasks });
    await userEvent.click(screen.getByRole("button", { name: /^1 Yes$/ }));
    expect(value.answerAgent).toHaveBeenCalledWith("fix-login", "1");
    expect(value.allowTool).not.toHaveBeenCalled();
  });

  it("does not answer when saving the tool failed", async () => {
    const { value } = renderThread([dialog], { tasks, allowTool: vi.fn(async () => false) });
    await userEvent.click(screen.getByRole("button", { name: "always allow for builder" }));
    expect(value.answerAgent).not.toHaveBeenCalled();
  });

  it("offers nothing to save for a question that is not a tool call", () => {
    const plain = entry(7, { kind: "needsYou", agent: "fix-login", question: "Pick one?\n1. a\n2. b", choices: ["1", "2"] });
    renderThread([plain], { tasks });
    expect(screen.queryByRole("button", { name: /always allow/ })).toBeNull();
  });

  it("says in the thread that a tool was added to the type", () => {
    const added = entry(8, { kind: "agent", agent: "fix-login", event: "allowed", text: "Bash(gh pr *) to builder" });
    renderThread([added], { tasks });
    expect(screen.getByText(/allowed/)).toHaveTextContent("fix-login allowed Bash(gh pr *) for agent type builder");
  });

  it("names the picked option once answered", () => {
    const sent = entry(5, { kind: "agent", agent: "fix-login", event: "sent", text: "3" });
    renderThread([dialog, sent], { tasks: [aTask({ name: "fix-login", state: "working" })] });
    const group = screen.getByRole("group", { name: "fix-login asked" });
    expect(within(group).getByText("3 \u00b7 Yes, and switch to auto mode")).toHaveClass("th-ask-picked");
    expect(within(group).queryByRole("button", { name: /^\d / })).toBeNull();
  });
});

describe("Thread repo question", () => {
  const ask = entry(2, { kind: "repoAsk", question: "two repos have a checkout flow. which one?", candidates: ["shop", "shop-b2b"], offerAll: true });

  it("offers each candidate and both, sending the text as a message", async () => {
    const { value } = renderThread([aYouEntry(1, "fix checkout"), ask]);
    const group = screen.getByRole("group", { name: "townhall needs you" });
    expect(within(group).getAllByRole("button").map((b) => b.textContent)).toEqual(["shop", "shop-b2b", "both"]);
    await userEvent.click(within(group).getByRole("button", { name: "both" }));
    expect(value.sendMessage).toHaveBeenCalledWith("both");
  });

  it("disables the answers while townhall thinks and drops them once you wrote", () => {
    const { rerender } = renderThread([aYouEntry(1, "fix checkout"), ask], { thinking: true });
    expect(screen.getByRole("button", { name: "shop" })).toBeDisabled();
    rerender([aYouEntry(1, "fix checkout"), ask, aYouEntry(3, "shop")]);
    expect(screen.queryByRole("button", { name: "shop" })).toBeNull();
  });
});

describe("Thread scrolling", () => {
  afterEach(() => vi.unstubAllGlobals());

  function sizeBox(box: HTMLElement, sizes: { scrollHeight: number; clientHeight: number }) {
    Object.defineProperty(box, "scrollHeight", { configurable: true, get: () => sizes.scrollHeight });
    Object.defineProperty(box, "clientHeight", { configurable: true, get: () => sizes.clientHeight });
  }

  it("follows new entries while at the bottom, and stays put once you scrolled up", () => {
    const first = [aYouEntry(1, "one")];
    const { rerender } = renderThread(first);
    const box = screen.getByRole("log");
    const sizes = { scrollHeight: 1000, clientHeight: 400 };
    sizeBox(box, sizes);
    rerender([...first, aYouEntry(2, "two")]);
    expect(box.scrollTop).toBe(1000);

    box.scrollTop = 100;
    act(() => {
      box.dispatchEvent(new Event("scroll"));
    });
    sizes.scrollHeight = 1400;
    rerender([...first, aYouEntry(2, "two"), aYouEntry(3, "three")]);
    expect(box.scrollTop).toBe(100);

    box.scrollTop = 1000;
    act(() => {
      box.dispatchEvent(new Event("scroll"));
    });
    sizes.scrollHeight = 1600;
    rerender([...first, aYouEntry(2, "two"), aYouEntry(3, "three"), aYouEntry(4, "four")]);
    expect(box.scrollTop).toBe(1600);
  });

  it("stays at the bottom when the thread box itself gets shorter", () => {
    const resize = { observed: [] as Element[], callback: () => undefined as void };
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: () => void) {
          resize.callback = callback;
        }
        observe(target: Element) {
          resize.observed.push(target);
        }
        disconnect() {}
      },
    );
    renderThread([aYouEntry(1, "one")]);
    const box = screen.getByRole("log");
    const sizes = { scrollHeight: 1000, clientHeight: 400 };
    sizeBox(box, sizes);
    box.scrollTop = 600;
    act(() => {
      box.dispatchEvent(new Event("scroll"));
    });
    expect(resize.observed).toContain(box);
    sizes.clientHeight = 320;
    act(() => resize.callback());
    expect(box.scrollTop).toBe(1000);
  });
});
