import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import { enqueue, readQueue } from "./queue";
import { readReply, writeReply } from "./replies";
import type { SubmitHandler } from "./useSubmit";

const input = () => screen.getByRole("textbox");
const list = () => screen.getByRole("list", { name: "Queued messages" });
const rows = () => within(list()).getAllByRole("listitem");

interface HarnessProps {
  readonly busy: boolean;
  readonly onSubmit: (text: string, ids: readonly string[], files: readonly File[]) => Promise<unknown>;
  readonly id?: string;
}

function Harness({ busy, onSubmit, id = "c1" }: HarnessProps) {
  return <Composer key={id} conversationId={id} busy={busy} onStop={vi.fn()} onSubmit={onSubmit} />;
}

const accepted = () => vi.fn<SubmitHandler>(async () => undefined);

function row(index: number): HTMLElement {
  const found = rows()[index];
  if (found === undefined) throw new Error(`no queued row at ${index}`);
  return found;
}

async function queueUp(user: ReturnType<typeof userEvent.setup>, ...messages: string[]) {
  for (const message of messages) await user.type(input(), `${message}{Enter}`);
}

describe("Composer queue while townhall thinks", () => {
  it("keeps the input enabled and queues on enter instead of sending", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    render(<Harness busy onSubmit={onSubmit} />);
    expect(input()).toBeEnabled();
    await user.type(input(), "  then add tests  {Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(input()).toHaveValue("");
    expect(rows()).toHaveLength(1);
    expect(row(0)).toHaveTextContent("then add tests");
    expect(readQueue("c1").map((item) => item.text)).toEqual(["then add tests"]);
  });

  it("queues several messages in order and keeps stop working", async () => {
    const user = userEvent.setup();
    const onStop = vi.fn();
    render(<Composer conversationId="c1" busy onStop={onStop} onSubmit={accepted()} />);
    await queueUp(user, "one", "two", "three");
    expect(rows().map((row) => row.textContent)).toEqual([
      expect.stringContaining("one"),
      expect.stringContaining("two"),
      expect.stringContaining("three"),
    ]);
    await user.click(screen.getByRole("button", { name: "stop" }));
    expect(onStop).toHaveBeenCalledOnce();
  });

  it("does not queue blank text or a line break from shift enter", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await user.type(input(), "   {Enter}");
    await user.type(input(), "a{Shift>}{Enter}{/Shift}b");
    expect(readQueue("c1")).toEqual([]);
    expect(input()).toHaveValue("   a\nb");
  });

  it("removes a queued message with its x", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, "one", "two");
    await user.click(within(row(0)).getByRole("button", { name: /remove/i }));
    expect(readQueue("c1").map((item) => item.text)).toEqual(["two"]);
    expect(rows()).toHaveLength(1);
  });

  it("shows no list while nothing is queued", () => {
    render(<Harness busy onSubmit={accepted()} />);
    expect(screen.queryByRole("list", { name: "Queued messages" })).not.toBeInTheDocument();
  });

  it("never posts while busy, even with messages waiting", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one", "two");
    await act(async () => undefined);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("keeps the queue per conversation across switching and remounting", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<Harness id="a" busy onSubmit={accepted()} />);
    await queueUp(user, "for a");
    rerender(<Harness id="b" busy onSubmit={accepted()} />);
    expect(screen.queryByRole("list", { name: "Queued messages" })).not.toBeInTheDocument();
    await queueUp(user, "for b");
    rerender(<Harness id="a" busy onSubmit={accepted()} />);
    expect(rows()).toHaveLength(1);
    expect(row(0)).toHaveTextContent("for a");
    rerender(<Harness id="b" busy onSubmit={accepted()} />);
    expect(row(0)).toHaveTextContent("for b");
  });

  it("shows a long message on one truncated row and keeps its full text", async () => {
    const user = userEvent.setup();
    const long = "word ".repeat(80).trim();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, long);
    expect(rows()).toHaveLength(1);
    expect(row(0).querySelector(".th-queue-text")).toHaveAttribute("title", long);
  });
});

describe("Composer queue draining", () => {
  it("sends the queued messages one at a time once idle, each after the planner went busy and idle again", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one", "two", "three");
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit.mock.calls.map((call) => call[0])).toEqual(["one"]);
    expect(rows().map((row) => row.textContent)).toEqual([
      expect.stringContaining("two"),
      expect.stringContaining("three"),
    ]);
    await act(async () => undefined);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    rerender(<Harness busy onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit.mock.calls.map((call) => call[0])).toEqual(["one", "two"]);
    rerender(<Harness busy onSubmit={onSubmit} />);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit.mock.calls.map((call) => call[0])).toEqual(["one", "two", "three"]);
    expect(readQueue("c1")).toEqual([]);
  });

  it("waits for a send to be accepted before it sends the next", async () => {
    const user = userEvent.setup();
    let accept: () => void = () => undefined;
    const onSubmit = vi.fn<SubmitHandler>(() => new Promise<void>((done) => (accept = done)));
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one", "two");
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    rerender(<Harness busy onSubmit={onSubmit} />);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(readQueue("c1")).toHaveLength(2);
    await act(async () => accept());
    expect(readQueue("c1").map((item) => item.text)).toEqual(["two"]);
  });

  it("stops draining when a send fails, keeps the rest and shows the error", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn<SubmitHandler>().mockRejectedValue(new Error("cannot reach the daemon"));
    const { container, rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one", "two");
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    rerender(<Harness busy onSubmit={onSubmit} />);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(readQueue("c1").map((item) => item.text)).toEqual(["one", "two"]);
    expect(container.querySelector(".th-help")).toHaveTextContent("error: cannot reach the daemon");
  });

  it("carries attachments and the reply target of a queued message into its send", async () => {
    const user = userEvent.setup();
    const target = { id: 4, author: "townhall", excerpt: "which repo?" } as const;
    enqueue("c1", { text: "the api repo", attachmentIds: ["att-1"], reply: target });
    let seen: unknown = null;
    const onSubmit = vi.fn<SubmitHandler>(async () => {
      seen = readReply("c1");
    });
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(onSubmit).toHaveBeenCalledWith("the api repo", ["att-1"], []);
    expect(seen).toEqual(target);
    await user.type(input(), "x");
  });

  it("leaves a reply the user is composing alone", async () => {
    const mine = { id: 9, author: "you", excerpt: "mine" } as const;
    enqueue("c1", { text: "queued", attachmentIds: [], reply: null });
    writeReply("c1", mine);
    const onSubmit = accepted();
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    expect(readReply("c1")).toEqual(mine);
  });

  it("queues instead of jumping ahead when idle with messages still waiting", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn<SubmitHandler>().mockRejectedValueOnce(new Error("down")).mockResolvedValue(undefined);
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one");
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await act(async () => undefined);
    await user.type(input(), "two{Enter}");
    await act(async () => undefined);
    expect(onSubmit.mock.calls.map((call) => call[0])).toEqual(["one", "one"]);
    expect(readQueue("c1").map((item) => item.text)).toEqual(["two"]);
  });

  it("behaves as before when idle with an empty queue", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    render(<Harness busy={false} onSubmit={onSubmit} />);
    await user.type(input(), "hello{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("hello", [], []);
    expect(screen.queryByRole("list", { name: "Queued messages" })).not.toBeInTheDocument();
  });
});

describe("Composer queue editing", () => {
  it("pulls the last queued message back on arrow up in an empty input", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, "one", "two");
    await user.keyboard("{ArrowUp}");
    expect(input()).toHaveValue("two");
    expect(readQueue("c1").map((item) => item.text)).toEqual(["one"]);
  });

  it("re-queues the edited message on enter while busy", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one", "two");
    await user.keyboard("{ArrowUp}{Backspace}o!{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(input()).toHaveValue("");
    expect(readQueue("c1").map((item) => item.text)).toEqual(["one", "two!"]);
  });

  it("sends the edited message on enter when idle", async () => {
    const user = userEvent.setup();
    const onSubmit = accepted();
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await queueUp(user, "one");
    await user.keyboard("{ArrowUp}");
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await user.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("one", [], []);
  });

  it("puts the message back unchanged on escape", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, "one", "two");
    const before = readQueue("c1");
    await user.keyboard("{ArrowUp}");
    await user.type(input(), " changed");
    await user.keyboard("{Escape}");
    expect(input()).toHaveValue("");
    expect(readQueue("c1").map((item) => item.text)).toEqual(["one", "two"]);
    expect(readQueue("c1")[1]).toEqual(before[1]);
  });

  it("keeps the attachments and reply target of a pulled message", async () => {
    const user = userEvent.setup();
    const target = { id: 4, author: "townhall", excerpt: "which repo?" } as const;
    enqueue("c1", { text: "the api repo", attachmentIds: ["att-1"], reply: target });
    const onSubmit = accepted();
    const { rerender } = render(<Harness busy onSubmit={onSubmit} />);
    await user.click(input());
    await user.keyboard("{ArrowUp}");
    expect(readReply("c1")).toEqual(target);
    rerender(<Harness busy={false} onSubmit={onSubmit} />);
    await user.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("the api repo", ["att-1"], []);
  });

  it("keeps normal caret behaviour on arrow up in a non empty input", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, "one");
    await user.type(input(), "draft");
    await user.keyboard("{Home}{ArrowUp}");
    expect(input()).toHaveValue("draft");
    expect(readQueue("c1")).toHaveLength(1);
  });

  it("does nothing special on arrow up with an empty queue", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await user.click(input());
    await user.keyboard("{ArrowUp}");
    expect(input()).toHaveValue("");
  });

  it("ignores arrow up with a modifier", async () => {
    const user = userEvent.setup();
    render(<Harness busy onSubmit={accepted()} />);
    await queueUp(user, "one");
    await user.keyboard("{Shift>}{ArrowUp}{/Shift}");
    expect(input()).toHaveValue("");
    expect(readQueue("c1")).toHaveLength(1);
  });
});
