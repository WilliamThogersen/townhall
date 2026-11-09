import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import { ApiError } from "../api/errors";
import { aConversation, aDetail, anAgentInfo, anEntry, aRepo, aTask, aYouEntry, someStats } from "../api/fixtures";
import * as reposApi from "../api/repos";
import * as statsApi from "../api/stats";
import * as tasksApi from "../api/tasks";
import {
  useAgentInfos,
  useConversation,
  useConversationActions,
  useConversations,
  useRepos,
  useStats,
  useTasks,
} from ".";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "./testing";

afterEach(() => vi.restoreAllMocks());

const wrapper = ({ children }: { readonly children: ReactNode }) => (
  <TestDataProvider>{children}</TestDataProvider>
);

const withEvents = (events: FakeEvents) =>
  function Wrapper({ children }: { readonly children: ReactNode }) {
    return <TestDataProvider events={events}>{children}</TestDataProvider>;
  };

describe("useConversations", () => {
  it("lists conversations newest activity first", async () => {
    const older = aConversation({ id: "old", updatedAt: "2026-11-01T00:00:00.000Z" });
    const newer = aConversation({ id: "new", updatedAt: "2026-11-03T00:00:00.000Z" });
    vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([older, newer]);
    const { result } = renderHook(() => useConversations(), { wrapper });
    expect(result.current).toEqual({ conversations: [], loading: true, error: null });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.conversations.map((c) => c.id)).toEqual(["new", "old"]);
  });

  it("surfaces the error without a prefix", async () => {
    vi.spyOn(conversationsApi, "listConversations").mockRejectedValue(
      new ApiError("network", "cannot reach the daemon: down"),
    );
    const { result } = renderHook(() => useConversations(), { wrapper });
    await waitFor(() => expect(result.current.error).toBe("cannot reach the daemon: down"));
    expect(result.current.loading).toBe(false);
  });
});

describe("useConversation", () => {
  it("loads a detail on demand and reloads it", async () => {
    const get = vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(aDetail());
    const { result } = renderHook(() => useConversation("c1"), { wrapper });
    await waitFor(() => expect(result.current.detail).toEqual(aDetail()));
    act(() => result.current.reload());
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(get.mock.calls[0]?.[0]).toBe("c1");
    expect(result.current.loading).toBe(false);
  });

  it("is idle without an id", () => {
    const get = vi.spyOn(conversationsApi, "getConversation");
    const { result } = renderHook(() => useConversation(null), { wrapper });
    expect(result.current).toMatchObject({ detail: null, loading: false, error: null });
    expect(get).not.toHaveBeenCalled();
  });

  it("starts over when the id changes and keeps old data on a failed reload", async () => {
    const get = vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(aDetail());
    const { result, rerender } = renderHook(({ id }) => useConversation(id), {
      wrapper,
      initialProps: { id: "c1" },
    });
    await waitFor(() => expect(result.current.detail).not.toBeNull());
    get.mockRejectedValue(new ApiError("http", "conversation not found", 404));
    act(() => result.current.reload());
    await waitFor(() => expect(result.current.error).toBe("conversation not found"));
    expect(result.current.detail).toEqual(aDetail());
    rerender({ id: "c2" });
    expect(result.current).toMatchObject({ detail: null, loading: true, error: null });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.detail).toBeNull();
  });
});

describe("useTasks", () => {
  it("filters every task by conversation and repo and stays live", async () => {
    const mine = aTask({ name: "a", conversationId: "c1", repo: "shop" });
    const other = aTask({ name: "b", conversationId: "c2", repo: "admin" });
    const list = vi.spyOn(tasksApi, "listTasks").mockResolvedValue([mine, other]);
    const events = createFakeEvents();
    const { result } = renderHook(
      () => ({ byConversation: useTasks({ conversationId: "c1" }), byRepo: useTasks({ repo: "ADMIN" }), all: useTasks() }),
      { wrapper: withEvents(events) },
    );
    await waitFor(() => expect(result.current.all.tasks).toEqual([mine, other]));
    expect(result.current.byConversation.tasks).toEqual([mine]);
    expect(result.current.byRepo.tasks).toEqual([other]);
    const added = aTask({ name: "c", conversationId: "c1", repo: "shop" });
    act(() => events.emit({ type: "taskUpdated", task: added }));
    expect(result.current.byConversation.tasks).toEqual([mine, added]);
    expect(list).toHaveBeenCalledOnce();
    expect(list.mock.calls[0]?.[0]).toEqual({});
  });
});

describe("useAgentInfos", () => {
  it("lists agents with their repo and state", async () => {
    vi.spyOn(client, "listAgents").mockResolvedValue([anAgentInfo()]);
    const { result } = renderHook(() => useAgentInfos(), { wrapper });
    await waitFor(() => expect(result.current.agents).toEqual([anAgentInfo()]));
  });
});

describe("useRepos", () => {
  it("is null until loaded, then adds and removes", async () => {
    const list = vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo()]);
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "admin" }));
    const remove = vi.spyOn(reposApi, "removeRepo").mockResolvedValue(aRepo());
    const { result } = renderHook(() => useRepos(), { wrapper });
    expect(result.current.repos).toBeNull();
    await waitFor(() => expect(result.current.repos).toEqual([aRepo()]));
    list.mockResolvedValue([aRepo(), aRepo({ name: "admin" })]);
    await act(async () => expect(await result.current.add({ path: "/code/admin" })).toEqual(aRepo({ name: "admin" })));
    await waitFor(() => expect(result.current.repos).toHaveLength(2));
    list.mockResolvedValue([aRepo({ name: "admin" })]);
    await act(() => result.current.remove("shop"));
    await waitFor(() => expect(result.current.repos).toEqual([aRepo({ name: "admin" })]));
    expect(add).toHaveBeenCalledWith({ path: "/code/admin" });
    expect(remove).toHaveBeenCalledWith("shop");
  });

  it("picks a repo's accounts", async () => {
    vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo()]);
    const picked = aRepo({ gitlabAccount: "me" });
    const put = vi.spyOn(reposApi, "putRepoAccounts").mockResolvedValue(picked);
    const { result } = renderHook(() => useRepos(), { wrapper });
    await waitFor(() => expect(result.current.repos).toEqual([aRepo()]));
    await act(() => result.current.setAccounts("shop", { gitlabAccount: "me" }));
    await waitFor(() => expect(result.current.repos).toEqual([picked]));
    expect(put).toHaveBeenCalledWith("shop", { gitlabAccount: "me" });
  });
});

describe("useStats", () => {
  it("loads stats for every repo or one", async () => {
    const get = vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
    const { result, rerender } = renderHook(({ repo }) => useStats(repo), {
      wrapper,
      initialProps: { repo: undefined as string | undefined },
    });
    await waitFor(() => expect(result.current.stats).toEqual(someStats()));
    rerender({ repo: "shop" });
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(get.mock.calls[1]?.[0]).toBe("shop");
  });
});

describe("useConversationActions", () => {
  it("calls the daemon for each action", async () => {
    const start = vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(aDetail());
    const send = vi.spyOn(conversationsApi, "sendMessage").mockResolvedValue(aYouEntry(2, "hi"));
    const stop = vi.spyOn(conversationsApi, "stopTurn").mockResolvedValue(aConversation());
    const end = vi.spyOn(conversationsApi, "endConversation").mockResolvedValue(aConversation({ ended: true }));
    const patch = vi.spyOn(conversationsApi, "patchConversation").mockResolvedValue(aConversation());
    const { result } = renderHook(() => useConversationActions(), { wrapper });
    const actions = result.current;
    expect(await actions.start({ text: "fix login" })).toEqual(aDetail());
    expect(await actions.send("c1", { text: "hi" })).toEqual(aYouEntry(2, "hi"));
    await actions.stop("c1");
    expect((await actions.end("c1")).ended).toBe(true);
    await actions.patch("c1", { title: "login" });
    expect(start).toHaveBeenCalledWith({ text: "fix login" }, []);
    expect(send).toHaveBeenCalledWith("c1", { text: "hi" });
    expect(stop).toHaveBeenCalledWith("c1");
    expect(end).toHaveBeenCalledWith("c1");
    expect(patch).toHaveBeenCalledWith("c1", { title: "login" });
  });
});

describe("live hooks", () => {
  it("useConversation follows entries, its conversation and your sent messages", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(aDetail({ entries: [aYouEntry(1, "hi")] }));
    vi.spyOn(conversationsApi, "sendMessage").mockResolvedValue(aYouEntry(3, "more"));
    const events = createFakeEvents();
    const { result } = renderHook(() => ({ one: useConversation("c1"), act: useConversationActions() }), {
      wrapper: withEvents(events),
    });
    await waitFor(() => expect(result.current.one.detail).not.toBeNull());
    const reply = anEntry(2, { kind: "townhall", text: "on it" });
    act(() => events.emit({ type: "entryAppended", conversationId: "c1", entry: reply }));
    act(() => events.emit({ type: "conversationUpdated", conversation: aConversation({ status: "thinking" }) }));
    await act(() => result.current.act.send("c1", { text: "more" }));
    expect(result.current.one.detail?.entries.map((e) => e.id)).toEqual([1, 2, 3]);
    expect(result.current.one.detail?.conversation.status).toBe("thinking");
  });

  it("useConversations and useAgentInfos follow their events", async () => {
    vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([aConversation()]);
    vi.spyOn(client, "listAgents").mockResolvedValue([anAgentInfo()]);
    const events = createFakeEvents();
    const { result } = renderHook(() => ({ list: useConversations(), agents: useAgentInfos() }), {
      wrapper: withEvents(events),
    });
    await waitFor(() => expect(result.current.agents.loading).toBe(false));
    await waitFor(() => expect(result.current.list.loading).toBe(false));
    const fresh = aConversation({ id: "c2", updatedAt: "2026-11-05T00:00:00.000Z" });
    const waiting = anAgentInfo({ state: { kind: "waiting", question: "go?" } });
    act(() => {
      events.emit({ type: "conversationUpdated", conversation: fresh });
      events.emit({ type: "agentUpdated", agent: waiting });
    });
    expect(result.current.list.conversations.map((c) => c.id)).toEqual(["c2", "c1"]);
    expect(result.current.agents.agents).toEqual([waiting]);
  });

  it("useStats refetches on resync and keeps the last stats when that fails", async () => {
    const get = vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
    const events = createFakeEvents();
    const { result } = renderHook(() => useStats(), { wrapper: withEvents(events) });
    await waitFor(() => expect(result.current.stats).toEqual(someStats()));
    get.mockRejectedValue(new ApiError("http", "error: repo not found", 404));
    act(() => events.emit({ type: "resync" }));
    await waitFor(() => expect(result.current.error).toBe("repo not found"));
    expect(result.current).toMatchObject({ stats: someStats(), loading: false });
    get.mockResolvedValue(someStats());
    act(() => events.emit({ type: "resync" }));
    await waitFor(() => expect(result.current.error).toBeNull());
  });

  it("useStats starts over for another repo and reports a first failure", async () => {
    const get = vi.spyOn(statsApi, "getStats").mockResolvedValueOnce(someStats());
    const { result, rerender } = renderHook(({ repo }) => useStats(repo), {
      wrapper,
      initialProps: { repo: "shop" },
    });
    await waitFor(() => expect(result.current.stats).not.toBeNull());
    get.mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    rerender({ repo: "admin" });
    expect(result.current).toEqual({ stats: null, loading: true, error: null });
    await waitFor(() => expect(result.current.error).toBe("cannot reach the daemon: down"));
    expect(result.current.stats).toBeNull();
  });
});
