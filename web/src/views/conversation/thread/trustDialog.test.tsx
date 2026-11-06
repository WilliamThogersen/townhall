import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { anEntry, aTask } from "../../../api/fixtures";
import type { ThreadEntry } from "../../../api/types";
import { TestDataProvider } from "../../../data/testing";
import { useCommands } from "../useConversationView";
import { Thread } from "./Thread";
import { ThreadContext, type ThreadContextValue } from "./threadContext";
import { choiceKeys, KEY_GAP_MS, TRUST_QUESTION } from "./trustDialog";

const AGENT = "fix-login";
const SEND_URL = `/api/agents/${AGENT}/send`;

interface Sent {
  readonly url: string;
  readonly body: unknown;
  readonly at: number;
}

function mockFetch() {
  const sent: Sent[] = [];
  const fn = vi.fn(async (url: string, init?: RequestInit) => {
    sent.push({ url, body: JSON.parse(String(init?.body ?? "null")), at: performance.now() });
    return new Response(JSON.stringify({ success: true, data: AGENT, error: null }), { status: 200 });
  });
  vi.stubGlobal("fetch", fn);
  return () => sent.filter((s) => s.url.endsWith("/send"));
}

function Harness({ entries }: { readonly entries: readonly ThreadEntry[] }) {
  const commands = useCommands("c1");
  const value: ThreadContextValue = {
    conversationId: "c1",
    tasks: [aTask({ name: AGENT, state: "needsYou" })],
    attachments: [],
    thinking: false,
    openTask: null,
    onOpenTask: () => undefined,
    onReply: () => undefined,
    sendMessage: commands.sendMessage,
    answerAgent: commands.answerAgent,
    answerKeys: commands.answerKeys,
    allowTool: vi.fn(async () => true),
  };
  return (
    <ThreadContext value={value}>
      <Thread entries={entries} ended={false} />
    </ThreadContext>
  );
}

function renderAsk(question: string, choices: readonly string[]) {
  const ask = anEntry(4, { kind: "needsYou", agent: AGENT, question, choices: [...choices] });
  render(
    <TestDataProvider>
      <Harness entries={[ask]} />
    </TestDataProvider>,
  );
  return screen.getByRole("group", { name: `${AGENT} needs you` });
}

afterEach(() => vi.unstubAllGlobals());

describe("claude's folder trust dialog", () => {
  const trust = ["yes, trust it", "no, exit"] as const;

  it("trusts the worktree with up, then enter, as keys", async () => {
    const sends = mockFetch();
    const group = renderAsk(TRUST_QUESTION, trust);
    await userEvent.click(within(group).getByRole("button", { name: "send yes, trust it" }));
    await waitFor(() => expect(sends()).toHaveLength(2));
    const [up, enter] = sends();
    expect(sends().map(({ url, body }) => ({ url, body }))).toEqual([
      { url: SEND_URL, body: { key: "up" } },
      { url: SEND_URL, body: { key: "enter" } },
    ]);
    expect((enter?.at ?? 0) - (up?.at ?? 0)).toBeGreaterThanOrEqual(KEY_GAP_MS - 50);
  });

  it("exits with enter alone, since the cursor starts on no", async () => {
    const sends = mockFetch();
    const group = renderAsk(TRUST_QUESTION, trust);
    await userEvent.click(within(group).getByRole("button", { name: "send no, exit" }));
    await waitFor(() => expect(sends()).toHaveLength(1));
    expect(sends()[0]).toMatchObject({ url: SEND_URL, body: { key: "enter" } });
  });

  it("still sends any other question's choice as text", async () => {
    const sends = mockFetch();
    const group = renderAsk("commit the fix? (y/n)", ["y", "n"]);
    await userEvent.click(within(group).getByRole("button", { name: "send y" }));
    await waitFor(() => expect(sends()).toHaveLength(1));
    expect(sends()[0]).toMatchObject({ url: SEND_URL, body: { text: "y" } });
  });

  it("has no free text answer, since any text would pick no, exit", () => {
    mockFetch();
    const group = renderAsk(TRUST_QUESTION, trust);
    expect(within(group).queryByRole("textbox")).toBeNull();
    expect(within(group).getByRole("button", { name: "send yes, trust it" })).toBeInTheDocument();
  });

  it("maps only the trust dialog's own choices to keys", () => {
    expect(choiceKeys(TRUST_QUESTION, "yes, trust it")).toEqual(["up", "enter"]);
    expect(choiceKeys(TRUST_QUESTION, "no, exit")).toEqual(["enter"]);
    expect(choiceKeys(TRUST_QUESTION, "maybe")).toBeNull();
    expect(choiceKeys("trust it? (y/n)", "yes, trust it")).toBeNull();
  });
});
