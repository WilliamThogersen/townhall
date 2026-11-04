import { afterEach, describe, expect, it, vi } from "vitest";
import * as conversationsApi from "../api/conversations";
import { ApiError } from "../api/errors";
import { aConversation, aDetail, aRepo, aTask, aYouEntry } from "../api/fixtures";
import * as reposApi from "../api/repos";
import { CONVERSATIONS_KEY, detailKey, REPOS_KEY, TASKS_KEY } from "./resources";
import { flush, spyApi, startStore } from "./storeTestKit";

afterEach(() => vi.restoreAllMocks());

async function loaded() {
  const api = spyApi();
  api.listConversations.mockResolvedValue([aConversation()]);
  api.getConversation.mockResolvedValue(aDetail());
  api.listRepos.mockResolvedValue([aRepo()]);
  const kit = startStore();
  [CONVERSATIONS_KEY, detailKey("c1"), TASKS_KEY, REPOS_KEY].forEach(kit.store.ensure);
  await flush();
  return kit;
}

describe("conversation actions", () => {
  it("start puts the new conversation, its detail and tasks in the store", async () => {
    const { store } = await loaded();
    const detail = aDetail({ conversation: aConversation({ id: "c2" }), tasks: [aTask({ conversationId: "c2" })] });
    vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(detail);
    expect(await store.actions.start({ text: "fix login" })).toEqual(detail);
    expect(store.getState().details.get("c2")?.data).toEqual(detail);
    expect(store.getState().conversations?.data?.map((c) => c.id)).toEqual(["c2", "c1"]);
    expect(store.getState().tasks?.data).toEqual(detail.tasks);
  });

  it("send adds your entry to the detail", async () => {
    const { store } = await loaded();
    vi.spyOn(conversationsApi, "sendMessage").mockResolvedValue(aYouEntry(1, "hi"));
    await store.actions.send("c1", { text: "hi" });
    expect(store.getState().details.get("c1")?.data?.entries).toEqual([aYouEntry(1, "hi")]);
  });

  it("stop, end and patch put the answered conversation in the store", async () => {
    const { store } = await loaded();
    const later = (over: Parameters<typeof aConversation>[0]) =>
      aConversation({ updatedAt: "2026-11-05T00:00:00.000Z", ...over });
    vi.spyOn(conversationsApi, "stopTurn").mockResolvedValue(later({ status: "yourTurn" }));
    vi.spyOn(conversationsApi, "endConversation").mockResolvedValue(later({ ended: true }));
    vi.spyOn(conversationsApi, "patchConversation").mockResolvedValue(later({ ended: true, title: "login" }));
    await store.actions.stop("c1");
    expect((await store.actions.end("c1")).ended).toBe(true);
    expect(store.getState().details.get("c1")?.data?.conversation.ended).toBe(true);
    await store.actions.patch("c1", { title: "login" });
    expect(store.getState().conversations?.data?.[0]?.title).toBe("login");
  });

  it("rejects with the daemon error and changes nothing", async () => {
    const { store } = await loaded();
    const before = store.getState();
    vi.spyOn(conversationsApi, "sendMessage").mockRejectedValue(new ApiError("http", "a turn is running", 409));
    await expect(store.actions.send("c1", { text: "x" })).rejects.toThrow("a turn is running");
    expect(store.getState()).toBe(before);
  });
});

describe("repo actions", () => {
  it("adds and removes repos in the loaded list", async () => {
    const { store } = await loaded();
    vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "admin" }));
    vi.spyOn(reposApi, "removeRepo").mockResolvedValue(aRepo());
    expect(await store.addRepo({ path: "/code/admin" })).toEqual(aRepo({ name: "admin" }));
    expect(store.getState().repos?.data?.map((r) => r.name)).toEqual(["shop", "admin"]);
    await store.removeRepo("shop");
    expect(store.getState().repos?.data?.map((r) => r.name)).toEqual(["admin"]);
  });
});
