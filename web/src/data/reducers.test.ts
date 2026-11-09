import { describe, expect, it } from "vitest";
import { aConversation, aDetail, anAgentInfo, anEntry, aRepo, aTask, aYouEntry } from "../api/fixtures";
import {
  applyEvent,
  applyRepoEdited,
  applyStarted,
  reduceDetail,
  upsertAgent,
  upsertConversation,
  upsertEntry,
  upsertRepo,
  upsertTask,
  withoutRepo,
} from "./reducers";
import { INITIAL_STATE, loaded, pending } from "./state";

const at = (day: number) => `2026-11-0${day}T10:00:00.000Z`;

describe("upsertConversation", () => {
  const a = aConversation({ id: "a", updatedAt: at(1) });
  const b = aConversation({ id: "b", updatedAt: at(2) });

  it("adds a new one and keeps newest activity first", () => {
    expect(upsertConversation([b], a).map((c) => c.id)).toEqual(["b", "a"]);
    expect(upsertConversation([b, a], { ...a, updatedAt: at(3) }).map((c) => c.id)).toEqual(["a", "b"]);
  });

  it("ignores an older copy and takes one written at the same time", () => {
    const list = [b, a];
    expect(upsertConversation(list, { ...b, title: "old", updatedAt: at(1) })).toBe(list);
    expect(upsertConversation(list, { ...b, title: "same time" })[0]?.title).toBe("same time");
  });
});

describe("upsertEntry", () => {
  const one = aYouEntry(1, "one");
  const two = anEntry(2, { kind: "tasksCreated", tasks: ["a"] });

  it("appends in id order, replaces the same id and sorts a late one in", () => {
    expect(upsertEntry([one], two)).toEqual([one, two]);
    const grown = anEntry(2, { kind: "tasksCreated", tasks: ["a", "b"] });
    expect(upsertEntry([one, two], grown)).toEqual([one, grown]);
    expect(upsertEntry([two], one)).toEqual([one, two]);
  });
});

describe("list upserts", () => {
  it("matches tasks by name ignoring case and keeps their order", () => {
    const first = aTask({ name: "Fix-Login" });
    const second = aTask({ name: "add-tests" });
    const moved = aTask({ name: "fix-login", state: "working" });
    expect(upsertTask([first, second], moved)).toEqual([moved, second]);
    expect(upsertTask([first], second)).toEqual([first, second]);
  });

  it("matches agents by repo and name", () => {
    const shop = anAgentInfo({ repo: "shop" });
    const admin = anAgentInfo({ repo: "admin" });
    const waiting = anAgentInfo({ repo: "SHOP", state: { kind: "waiting", question: "go?" } });
    expect(upsertAgent([shop, admin], waiting)).toEqual([waiting, admin]);
  });

  it("adds and removes repos by name ignoring case", () => {
    const list = upsertRepo([aRepo()], aRepo({ name: "admin" }));
    expect(list.map((r) => r.name)).toEqual(["shop", "admin"]);
    expect(withoutRepo(list, "SHOP").map((r) => r.name)).toEqual(["admin"]);
  });
});

describe("applyRepoEdited", () => {
  const edited = aRepo({ name: "store", baseBranch: "develop" });
  const state = {
    ...INITIAL_STATE,
    repos: loaded([aRepo({ name: "Shop" }), aRepo({ name: "admin" })]),
    tasks: loaded([aTask({ name: "t1", repo: "shop" }), aTask({ name: "t2", repo: "admin" })]),
    conversations: loaded([aConversation({ id: "c1", linkedRepo: "SHOP" }), aConversation({ id: "c2", linkedRepo: null })]),
    agents: loaded([anAgentInfo({ repo: "shop" }), anAgentInfo({ repo: "admin" })]),
    details: new Map([["c1", loaded(aDetail({ conversation: aConversation({ id: "c1", linkedRepo: "shop" }), tasks: [aTask({ repo: "shop" })] }))]]),
  };

  it("replaces the repo in place and renames it in everything that mentions it", () => {
    const next = applyRepoEdited(state, "shop", edited);
    expect(next.repos?.data?.map((r) => r.name)).toEqual(["store", "admin"]);
    expect(next.tasks?.data?.map((t) => t.repo)).toEqual(["store", "admin"]);
    expect(next.conversations?.data?.map((c) => c.linkedRepo)).toEqual(["store", null]);
    expect(next.agents?.data?.map((a) => a.repo)).toEqual(["store", "admin"]);
    const detail = next.details.get("c1")?.data;
    expect([detail?.conversation.linkedRepo, detail?.tasks[0]?.repo]).toEqual(["store", "store"]);
  });

  it("only swaps the repo when its name stays", () => {
    const moved = aRepo({ name: "Shop", path: "/elsewhere" });
    const next = applyRepoEdited(state, "shop", moved);
    expect(next.repos?.data?.[0]).toBe(moved);
    expect(next.tasks).toBe(state.tasks);
    expect(next.agents).toBe(state.agents);
  });
});

describe("reduceDetail", () => {
  const detail = aDetail({ entries: [aYouEntry(1, "hi")] });

  it("leaves other conversations alone", () => {
    const other = aConversation({ id: "c2" });
    expect(reduceDetail(detail, { type: "conversationUpdated", conversation: other })).toBe(detail);
    expect(reduceDetail(detail, { type: "entryAppended", conversationId: "c2", entry: aYouEntry(2, "x") })).toBe(
      detail,
    );
    expect(reduceDetail(detail, { type: "taskUpdated", task: aTask({ conversationId: "c2" }) })).toBe(detail);
    expect(reduceDetail(detail, { type: "agentUpdated", agent: anAgentInfo() })).toBe(detail);
  });

  it("takes its own conversation, entries and tasks", () => {
    const thinking = aConversation({ status: "thinking" });
    expect(reduceDetail(detail, { type: "conversationUpdated", conversation: thinking }).conversation).toBe(thinking);
    const entry = aYouEntry(2, "more");
    expect(reduceDetail(detail, { type: "entryAppended", conversationId: "c1", entry }).entries).toHaveLength(2);
    expect(reduceDetail(detail, { type: "taskUpdated", task: aTask() }).tasks).toEqual([aTask()]);
  });
});

describe("applyEvent", () => {
  it("returns the same state when nothing loaded is touched", () => {
    const event = { type: "conversationUpdated", conversation: aConversation() } as const;
    expect(applyEvent(INITIAL_STATE, event)).toBe(INITIAL_STATE);
    const waiting = { ...INITIAL_STATE, conversations: pending<never>() };
    expect(applyEvent(waiting, event)).toBe(waiting);
  });

  it("updates every loaded resource the event touches", () => {
    const state = {
      ...INITIAL_STATE,
      tasks: loaded([aTask()]),
      details: new Map([["c1", loaded(aDetail())]]),
    };
    const working = aTask({ state: "working" });
    const next = applyEvent(state, { type: "taskUpdated", task: working });
    expect(next.tasks?.data).toEqual([working]);
    expect(next.details.get("c1")?.data?.tasks).toEqual([working]);
  });
});

describe("applyStarted", () => {
  it("loads the detail and adds the conversation and its tasks to loaded lists", () => {
    const detail = aDetail({ conversation: aConversation({ id: "new" }), tasks: [aTask()] });
    const state = { ...INITIAL_STATE, conversations: loaded([aConversation()]), tasks: loaded([]) };
    const next = applyStarted(state, detail);
    expect(next.details.get("new")).toEqual(loaded(detail));
    expect(next.conversations?.data?.map((c) => c.id)).toEqual(["new", "c1"]);
    expect(next.tasks?.data).toEqual([aTask()]);
    expect(applyStarted(INITIAL_STATE, detail).conversations).toBeNull();
  });
});
