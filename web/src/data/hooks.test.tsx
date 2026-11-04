import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import { ApiError } from "../api/errors";
import { aConversation, aDetail, anAgentInfo, aRepo, aTask, aYouEntry, someStats } from "../api/fixtures";
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
import { TestDataProvider } from "./testing";

afterEach(() => vi.restoreAllMocks());

const wrapper = ({ children }: { readonly children: ReactNode }) => (
  <TestDataProvider>{children}</TestDataProvider>
);

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
  it("passes the filter to the daemon", async () => {
    const list = vi.spyOn(tasksApi, "listTasks").mockResolvedValue([aTask()]);
    const { result } = renderHook(() => useTasks({ conversationId: "c1" }), { wrapper });
    await waitFor(() => expect(result.current.tasks).toEqual([aTask()]));
    expect(list.mock.calls[0]?.[0]).toEqual({ conversation: "c1", repo: undefined });
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
    expect(start).toHaveBeenCalledWith({ text: "fix login" });
    expect(send).toHaveBeenCalledWith("c1", { text: "hi" });
    expect(stop).toHaveBeenCalledWith("c1");
    expect(end).toHaveBeenCalledWith("c1");
    expect(patch).toHaveBeenCalledWith("c1", { title: "login" });
  });
});
