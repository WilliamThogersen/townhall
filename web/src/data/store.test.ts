import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import { aConversation, aDetail, anAgentInfo, aTask, aYouEntry } from "../api/fixtures";
import { AGENTS_KEY, CONVERSATIONS_KEY, detailKey, REPOS_KEY, TASKS_KEY } from "./resources";
import { deferred, flush, spyApi, startStore } from "./storeTestKit";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("data store loading", () => {
  it("loads on demand once and marks the resource pending meanwhile", async () => {
    const api = spyApi();
    api.listConversations.mockResolvedValue([aConversation()]);
    const { store } = startStore();
    expect(store.getState().conversations).toBeNull();
    store.ensure(CONVERSATIONS_KEY);
    store.ensure(CONVERSATIONS_KEY);
    expect(store.getState().conversations).toEqual({ data: null, settled: false, error: null });
    await flush();
    store.ensure(CONVERSATIONS_KEY);
    expect(store.getState().conversations).toEqual({ data: [aConversation()], settled: true, error: null });
    expect(api.listConversations).toHaveBeenCalledOnce();
  });

  it("strips an error: prefix, keeps old data on a failed refetch and retries a failed first load", async () => {
    const api = spyApi();
    api.listTasks.mockRejectedValueOnce(new Error("error: boom"));
    const { store } = startStore();
    store.ensure(TASKS_KEY);
    await flush();
    expect(store.getState().tasks).toEqual({ data: null, settled: true, error: "boom" });
    api.listTasks.mockResolvedValue([aTask()]);
    store.ensure(TASKS_KEY);
    await flush();
    expect(store.getState().tasks?.data).toEqual([aTask()]);
    api.listTasks.mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    store.refetch(TASKS_KEY);
    await flush();
    expect(store.getState().tasks).toEqual({ data: [aTask()], settled: true, error: "cannot reach the daemon: down" });
  });

  it("replays events that arrive while a load is in flight", async () => {
    const api = spyApi();
    const answer = deferred<ReturnType<typeof aDetail>>();
    api.getConversation.mockReturnValue(answer.promise);
    const { store, events } = startStore();
    store.ensure(detailKey("c1"));
    events.emit({ type: "entryAppended", conversationId: "c1", entry: aYouEntry(2, "late") });
    answer.resolve(aDetail({ entries: [aYouEntry(1, "first")] }));
    await flush();
    expect(store.getState().details.get("c1")?.data?.entries.map((e) => e.id)).toEqual([1, 2]);
  });

  it("fetches again after a load that a resync caught in flight", async () => {
    const api = spyApi();
    const first = deferred<readonly never[]>();
    api.listRepos.mockReturnValueOnce(first.promise);
    const { store, events } = startStore();
    store.ensure(REPOS_KEY);
    events.emit({ type: "resync" });
    expect(api.listRepos).toHaveBeenCalledOnce();
    first.resolve([]);
    await flush();
    expect(api.listRepos).toHaveBeenCalledTimes(2);
  });

  it("drops answers after stop and loads again after a restart", async () => {
    const api = spyApi();
    const answer = deferred<readonly never[]>();
    api.listAgents.mockReturnValueOnce(answer.promise);
    const { store, events } = startStore();
    store.ensure(AGENTS_KEY);
    store.stop();
    expect(events.isConnected()).toBe(false);
    answer.resolve([]);
    await flush();
    expect(store.getState().agents?.settled).toBe(false);
    store.start();
    store.ensure(AGENTS_KEY);
    await flush();
    expect(store.getState().agents?.settled).toBe(true);
    expect(events.isConnected()).toBe(true);
  });

  it("refuses an unknown key", () => {
    const { store } = startStore();
    expect(() => store.ensure("nope")).toThrow("unknown data key nope");
  });
});

describe("data store events", () => {
  async function loadedStore() {
    const api = spyApi();
    api.listConversations.mockResolvedValue([aConversation()]);
    api.getConversation.mockResolvedValue(aDetail());
    api.listTasks.mockResolvedValue([aTask()]);
    api.listAgents.mockResolvedValue([anAgentInfo()]);
    const kit = startStore();
    [CONVERSATIONS_KEY, detailKey("c1"), TASKS_KEY, AGENTS_KEY, REPOS_KEY].forEach(kit.store.ensure);
    await flush();
    return { api, ...kit };
  }

  it("conversationUpdated updates the list and the loaded detail", async () => {
    const { store, events } = await loadedStore();
    const thinking = aConversation({ status: "thinking", updatedAt: "2026-11-05T00:00:00.000Z" });
    events.emit({ type: "conversationUpdated", conversation: thinking });
    expect(store.getState().conversations?.data).toEqual([thinking]);
    expect(store.getState().details.get("c1")?.data?.conversation).toEqual(thinking);
  });

  it("entryAppended adds to the loaded detail in id order", async () => {
    const { store, events } = await loadedStore();
    events.emit({ type: "entryAppended", conversationId: "c1", entry: aYouEntry(2, "two") });
    events.emit({ type: "entryAppended", conversationId: "c1", entry: aYouEntry(1, "one") });
    events.emit({ type: "entryAppended", conversationId: "c1", entry: aYouEntry(2, "two again") });
    const texts = store.getState().details.get("c1")?.data?.entries.map((e) => (e.kind === "you" ? e.text : ""));
    expect(texts).toEqual(["one", "two again"]);
  });

  it("taskUpdated updates tasks and its detail and refetches agents", async () => {
    const { store, events, api } = await loadedStore();
    const working = aTask({ state: "working" });
    events.emit({ type: "taskUpdated", task: working });
    expect(store.getState().tasks?.data).toEqual([working]);
    expect(store.getState().details.get("c1")?.data?.tasks).toEqual([working]);
    expect(api.listAgents).toHaveBeenCalledTimes(2);
  });

  it("agentUpdated updates the agent list", async () => {
    const { store, events } = await loadedStore();
    const waiting = anAgentInfo({ state: { kind: "waiting", question: "go?" } });
    events.emit({ type: "agentUpdated", agent: waiting });
    expect(store.getState().agents?.data).toEqual([waiting]);
  });

  it("resync refetches everything loaded and bumps the generation", async () => {
    const { store, events, api } = await loadedStore();
    events.emit({ type: "resync" });
    await flush();
    expect(store.getState().generation).toBe(1);
    for (const spy of Object.values(api)) expect(spy).toHaveBeenCalledTimes(2);
  });

  it("a reconnect refetches everything loaded", async () => {
    const { store, events, api } = await loadedStore();
    events.setStatus("closed");
    expect(store.getState().connection).toBe("closed");
    events.setStatus("connecting");
    events.setStatus("open");
    await flush();
    expect(store.getState().connection).toBe("open");
    expect(api.getConversation).toHaveBeenCalledTimes(2);
    expect(store.getState().generation).toBe(1);
  });
});

describe("data store connection", () => {
  it("on the first open refetches only what had already settled", async () => {
    const api = spyApi();
    const tasks = deferred<readonly never[]>();
    api.listTasks.mockReturnValueOnce(tasks.promise);
    const { store, events } = startStore("connecting");
    store.ensure(CONVERSATIONS_KEY);
    await flush();
    store.ensure(TASKS_KEY);
    events.setStatus("open");
    tasks.resolve([]);
    await flush();
    expect(api.listConversations).toHaveBeenCalledTimes(2);
    expect(api.listTasks).toHaveBeenCalledOnce();
    expect(store.getState().generation).toBe(0);
  });

  it("polls agents only once they are loaded", async () => {
    vi.useFakeTimers();
    const api = spyApi();
    const { store } = startStore("open", 1000);
    await vi.advanceTimersByTimeAsync(3000);
    expect(api.listAgents).not.toHaveBeenCalled();
    store.ensure(AGENTS_KEY);
    await vi.advanceTimersByTimeAsync(2000);
    expect(api.listAgents).toHaveBeenCalledTimes(3);
    store.stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(api.listAgents).toHaveBeenCalledTimes(3);
  });

  it("starts once and notifies subscribers until they leave", () => {
    const { store, events } = startStore("connecting");
    const listener = vi.fn();
    const leave = store.subscribe(listener);
    store.start();
    events.setStatus("closed");
    events.setStatus("closed");
    expect(listener).toHaveBeenCalledOnce();
    leave();
    events.setStatus("connecting");
    expect(listener).toHaveBeenCalledOnce();
    expect(store.getState().connection).toBe("connecting");
  });
});
