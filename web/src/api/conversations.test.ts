import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import {
  endConversation,
  getConversation,
  listConversations,
  patchConversation,
  sendMessage,
  startConversation,
  stopTurn,
} from "./conversations";
import { aConversation, aDetail, anAttachment, aTask, aYouEntry } from "./fixtures";

afterEach(() => vi.unstubAllGlobals());

const detail = aDetail({
  entries: [aYouEntry(1, "fix login")],
  tasks: [aTask()],
  attachments: [anAttachment()],
});

describe("conversations api", () => {
  it("lists conversations", async () => {
    const fn = mockFetch(200, [aConversation()]);
    expect(await listConversations()).toEqual([aConversation()]);
    expect(sent(fn)).toEqual({ url: "/api/conversations", method: "GET", body: undefined });
  });

  it("gets one conversation with its thread, tasks and files", async () => {
    const fn = mockFetch(200, detail);
    expect(await getConversation("a b")).toEqual(detail);
    expect(sent(fn).url).toBe("/api/conversations/a%20b");
  });

  it("starts a conversation with the first message", async () => {
    const fn = mockFetch(201, detail);
    expect(await startConversation({ text: "fix login", attachments: ["a1"] })).toEqual(detail);
    expect(sent(fn)).toEqual({
      url: "/api/conversations",
      method: "POST",
      body: { text: "fix login", attachments: ["a1"] },
    });
  });

  it("starts a conversation with files as multipart form data", async () => {
    const fn = mockFetch(201, detail);
    const shot = new File(["x"], "shot.png", { type: "image/png" });
    expect(await startConversation({ text: "match this" }, [shot])).toEqual(detail);
    const [url, init] = fn.mock.calls[0] ?? [];
    expect(url).toBe("/api/conversations");
    expect(init?.method).toBe("POST");
    expect(init?.headers).toBeUndefined();
    const form = init?.body as FormData;
    expect(form).toBeInstanceOf(FormData);
    expect(form.get("text")).toBe("match this");
    expect(form.getAll("file")).toEqual([shot]);
  });

  it("sends a message and returns your entry", async () => {
    const fn = mockFetch(201, aYouEntry(3, "and the logout"));
    expect(await sendMessage("c1", { text: "and the logout" })).toEqual(aYouEntry(3, "and the logout"));
    expect(sent(fn)).toEqual({
      url: "/api/conversations/c1/messages",
      method: "POST",
      body: { text: "and the logout" },
    });
  });

  it("patches the linked repo and title", async () => {
    const fn = mockFetch(200, aConversation({ linkedRepo: "shop" }));
    expect((await patchConversation("c1", { linkedRepo: "shop" })).linkedRepo).toBe("shop");
    expect(sent(fn)).toEqual({ url: "/api/conversations/c1", method: "PATCH", body: { linkedRepo: "shop" } });
  });

  it("stops the turn and ends the conversation without a body", async () => {
    const fn = mockFetch(200, aConversation({ ended: true }));
    expect((await stopTurn("c1")).ended).toBe(true);
    expect(await endConversation("c1")).toEqual(aConversation({ ended: true }));
    expect(sent(fn, 0)).toEqual({ url: "/api/conversations/c1/stop", method: "POST", body: undefined });
    expect(sent(fn, 1)).toEqual({ url: "/api/conversations/c1/end", method: "POST", body: undefined });
  });

  it("passes the daemon error through without a prefix", async () => {
    mockFetch(409, null, "a turn is running");
    await expect(sendMessage("c1", { text: "x" })).rejects.toMatchObject({
      kind: "http",
      status: 409,
      message: "a turn is running",
    });
  });

  it("refuses a detail with a broken entry", async () => {
    mockFetch(200, { ...detail, entries: [{ id: 1, at: "t", kind: "nope" }] });
    await expect(getConversation("c1")).rejects.toMatchObject({ kind: "protocol" });
  });
});
