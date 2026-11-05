import { describe, expect, it } from "vitest";
import { aConversation, anAgentInfo, aStep, aTask } from "../../api/fixtures";
import type { TimeAxis } from "./timeAxis";
import {
  buildRows,
  conversationKey,
  isExpanded,
  latestStep,
  rowLabel,
  taskHref,
  taskKey,
  toggleKey,
  type RowsInput,
  type StepsBook,
} from "./traceRows";

const iso = (h: number, m = 0) => new Date(2026, 10, 4, h, m).toISOString();
const NOW = new Date(2026, 10, 4, 11).getTime();
const AXIS: TimeAxis = { start: new Date(2026, 10, 4, 10).getTime(), end: NOW + 20 * 60_000, step: 600_000, ticks: 8, now: NOW, today: true };

const login = aConversation({ id: "c1", title: "login redirect loop", createdAt: iso(10), updatedAt: iso(10, 50) });
const vite = aConversation({ id: "c2", title: "upgrade to vite 8", status: "thinking", createdAt: iso(10, 55) });
const fix = aTask({ name: "Fix-Login", conversationId: "c1", state: "needsYou", repo: "townhall", createdAt: iso(10, 5) });
const tests = aTask({ name: "auth-tests", conversationId: "c1", state: "working", repo: "townhall", createdAt: iso(10, 6) });

const ENTRIES = [
  { conversation: login, tasks: [fix, tests], repo: "townhall", live: true },
  { conversation: vite, tasks: [], repo: "no repo", live: true },
];

const testStep = aStep({ id: "s9", tool: "Bash", summary: "npm test", startedAt: iso(10, 50), endedAt: null, result: "running" });
const BOOK: StepsBook = new Map([
  ["auth-tests", { steps: [aStep({ id: "s1", startedAt: iso(10, 7), endedAt: iso(10, 9), result: "ok" }), testStep], loading: false, error: null }],
]);

const input = (over: Partial<RowsInput> = {}): RowsInput => ({
  entries: ENTRIES,
  book: BOOK,
  agents: [anAgentInfo({ name: "fix-login", repo: "townhall", state: { kind: "waiting", question: "y?" } })],
  toggled: new Set(),
  axis: AXIS,
  ...over,
});

describe("folding", () => {
  it("opens conversations and folds tasks by default, and flips with toggle", () => {
    const toggled = toggleKey(new Set(), "t:a");
    expect(isExpanded(conversationKey("c1"), new Set())).toBe(true);
    expect(isExpanded(taskKey("a"), new Set())).toBe(false);
    expect(isExpanded(taskKey("A"), toggled)).toBe(true);
    expect(toggleKey(toggled, "t:a").has("t:a")).toBe(false);
  });
});

describe("buildRows", () => {
  it("lists conversations with their tasks, steps folded", () => {
    const rows = buildRows(input());
    expect(rows.map((r) => [r.key, r.depth, r.name, r.meta])).toEqual([
      ["c:c1", 0, "login redirect loop", "townhall"],
      ["t:fix-login", 1, "Fix-Login", "review"],
      ["t:auth-tests", 1, "auth-tests", "test"],
      ["c:c2", 0, "upgrade to vite 8", "no repo"],
    ]);
    const [convo, task] = rows;
    expect(convo).toMatchObject({ kind: "conversation", glyph: null, expanded: true, parent: null, href: "/c/c1" });
    expect(task).toMatchObject({ kind: "task", glyph: "waiting", expanded: false, parent: "c:c1", href: "/c/c1?t=Fix-Login" });
    expect(task?.tag).toEqual({ text: "needs you", tone: "waiting" });
    expect(rows[3]?.bar?.state).toBe("running");
  });

  it("hides the tasks of a folded conversation", () => {
    const rows = buildRows(input({ toggled: new Set(["c:c1"]) }));
    expect(rows.map((r) => r.key)).toEqual(["c:c1", "c:c2"]);
    expect(rows[0]?.expanded).toBe(false);
  });

  it("shows an unfolded task's steps with the slowest and failed tags", () => {
    const rows = buildRows(input({ toggled: new Set(["t:auth-tests"]) }));
    const steps = rows.filter((r) => r.kind === "step");
    expect(steps.map((r) => [r.key, r.depth, r.name, r.meta, r.parent])).toEqual([
      ["s:auth-tests:s1", 2, "read", "src/login.ts", "t:auth-tests"],
      ["s:auth-tests:s9", 2, "run tests", "npm test", "t:auth-tests"],
    ]);
    expect(steps[1]?.bar).toMatchObject({ state: "running", label: "10m · running" });
    expect(steps[0]?.href).toBe("/c/c1?t=auth-tests");
  });

  it("puts a note under an unfolded task while its steps load, fail or are empty", () => {
    const loading: StepsBook = new Map([["fix-login", { steps: [], loading: true, error: null }]]);
    const failed: StepsBook = new Map([["fix-login", { steps: [], loading: false, error: "agent not found" }]]);
    const empty: StepsBook = new Map([["fix-login", { steps: [], loading: false, error: null }]]);
    const note = (book: StepsBook) => buildRows(input({ book, toggled: new Set(["t:fix-login"]) })).find((r) => r.kind === "note");
    expect(note(loading)).toMatchObject({ name: "loading steps…", depth: 2, bar: null, key: "n:fix-login" });
    expect(note(failed)?.name).toBe("error: agent not found");
    expect(note(empty)?.name).toBe("no steps yet");
    expect(note(new Map())?.name).toBe("loading steps…");
  });

  it("says no steps yet for a queued task without asking the daemon", () => {
    const queued = aTask({ name: "logout", conversationId: "c1", state: "queued" });
    const entries = [{ ...ENTRIES[0]!, tasks: [queued] }];
    const rows = buildRows(input({ entries, toggled: new Set(["t:logout"]) }));
    expect(rows.at(-1)?.name).toBe("no steps yet");
  });
});

describe("helpers", () => {
  it("finds the latest step ignoring case and labels rows for screen readers", () => {
    expect(latestStep(BOOK, "Auth-Tests")?.id).toBe("s9");
    expect(latestStep(BOOK, "nope")).toBeNull();
    const [, task] = buildRows(input());
    expect(rowLabel(task!)).toBe("Fix-Login review needs you 55m");
    expect(taskHref(aTask({ name: "a b", conversationId: "c 1" }))).toBe("/c/c%201?t=a%20b");
  });
});
