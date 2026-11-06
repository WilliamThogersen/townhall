import { describe, expect, expectTypeOf, it } from "vitest";
import type * as Ui from "../settings/settings";
import { DEFAULT_SETTINGS } from "../settings/settings";
import type {
  AgentInfo,
  AgentKey,
  Conversation,
  ConversationDetail,
  SendInput,
  ServerEvent,
  SettingKey,
  Settings,
  Task,
  ThreadEntry,
} from "./types";

const conversation = {
  id: "a1b2c3d4e5f6",
  sessionId: "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b",
  sessionStarted: false,
  title: "new conversation",
  status: "yourTurn",
  tasks: [],
  linkedRepo: null,
  ended: false,
  context: null,
  createdAt: "2026-11-04T10:42:05.000Z",
  updatedAt: "2026-11-04T10:42:05.000Z",
} as const satisfies Conversation;

const entries = [
  { id: 1, at: "2026-11-04T10:42:05.000Z", kind: "you", text: "hi", attachments: ["a1"] },
  { id: 2, at: "2026-11-04T10:42:06.000Z", kind: "townhall", text: "ok" },
  {
    id: 3,
    at: "2026-11-04T10:42:07.000Z",
    kind: "repo",
    repo: "shop",
    baseBranch: "develop",
    rule: "code",
    reason: "it has src/search/query.ts",
  },
  {
    id: 4,
    at: "2026-11-04T10:42:08.000Z",
    kind: "repoAsk",
    question: "which one?",
    candidates: ["shop", "admin"],
    offerAll: true,
  },
  { id: 5, at: "2026-11-04T10:42:09.000Z", kind: "tasksCreated", tasks: ["fix-login"] },
  {
    id: 6,
    at: "2026-11-04T10:42:10.000Z",
    kind: "needsYou",
    agent: "fix-login",
    question: "run migrations?",
    choices: ["y", "n"],
  },
  { id: 7, at: "2026-11-04T10:42:11.000Z", kind: "agent", agent: "fix-login", event: "sent", text: "y" },
  { id: 8, at: "2026-11-04T10:42:12.000Z", kind: "error", message: "claude exited with 1" },
] as const satisfies readonly ThreadEntry[];

const task = {
  name: "fix-login",
  title: "fix the login redirect",
  prompt: "the login page loops",
  repo: "shop",
  baseBranch: "develop",
  after: ["add-tests"],
  state: "needsYou",
  conversationId: "a1b2c3d4e5f6",
  attachments: [],
  createdAt: "2026-11-04T10:42:05.000Z",
  pr: { number: 41, url: null, checks: "failed" },
  agent: "builder",
  blocked: null,
  prBase: null,
} as const satisfies Task;

const agent = {
  name: "a",
  repo: "shop",
  kind: null,
  branch: "townhall/a",
  worktree: "/w/a",
  alive: true,
  state: { kind: "working" },
} as const satisfies AgentInfo;

const events = [
  { type: "conversationUpdated", conversation },
  { type: "entryAppended", conversationId: "a1b2c3d4e5f6", entry: entries[1] },
  { type: "taskUpdated", task },
  { type: "agentUpdated", agent },
  { type: "resync" },
] as const satisfies readonly ServerEvent[];

const sends = [{ text: "Down" }, { key: "down" }, { key: "enter" }] as const satisfies readonly SendInput[];

const detail = { conversation, entries, tasks: [task], attachments: [] } satisfies ConversationDetail;

describe("api contract types", () => {
  it("use the same settings names and values as the settings screen", () => {
    expectTypeOf<Settings>().toEqualTypeOf<Ui.Settings>();
    expectTypeOf<Settings["permissionMode"]>().toEqualTypeOf<Ui.PermissionMode>();
    expectTypeOf<Settings["townhallModel"]>().toEqualTypeOf<Ui.TownhallModel>();
    expectTypeOf<SettingKey>().toEqualTypeOf<keyof Ui.Settings>();
    const defaults: Settings = DEFAULT_SETTINGS;
    expect(defaults.permissionMode).toBe("acceptEdits");
    expect(defaults.townhallModel).toBe("sonnet");
  });

  it("describe the shapes the daemon sends", () => {
    expect(entries.map((e) => e.kind)).toEqual([
      "you",
      "townhall",
      "repo",
      "repoAsk",
      "tasksCreated",
      "needsYou",
      "agent",
      "error",
    ]);
    expect(events.map((e) => e.type)).toContain("resync");
    expect(detail.tasks[0]?.pr?.checks).toBe("failed");
  });

  it("send either text or one key, never both", () => {
    expectTypeOf<AgentKey>().toEqualTypeOf<"up" | "down" | "enter" | "escape" | "tab">();
    expectTypeOf<{ text: "y"; key: "up" }>().not.toExtend<SendInput>();
    expectTypeOf<{ key: "left" }>().not.toExtend<SendInput>();
    expect(sends.map((s) => ("key" in s ? s.key : s.text))).toEqual(["Down", "down", "enter"]);
  });
});
