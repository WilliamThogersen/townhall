import { describe, expect, it } from "vitest";
import { aConversation, aStep, aTask } from "../../api/fixtures";
import type { TimeAxis } from "./timeAxis";
import { headCaption, traceStats } from "./traceStats";
import type { StepsBook } from "./traceRows";
import type { ScopedConversation } from "./traceScope";

const iso = (h: number, day = 4) => new Date(2026, 10, day, h).toISOString();
const NOW = new Date(2026, 10, 4, 21).getTime();

const failedPr = { number: 41, url: null, checks: "failed" as const };
const ENTRIES: ScopedConversation[] = [
  {
    conversation: aConversation({ id: "c1", title: "login redirect loop", linkedRepo: "Shop" }),
    tasks: [
      aTask({ name: "a", state: "working", repo: "townhall" }),
      aTask({ name: "b", state: "needsYou", repo: "townhall" }),
      aTask({ name: "c", state: "error", repo: "shop" }),
      aTask({ name: "d", state: "inReview", pr: failedPr }),
      aTask({ name: "e", state: "merged", createdAt: iso(20) }),
      aTask({ name: "f", state: "merged", createdAt: iso(9, 3) }),
    ],
    repo: "Shop",
    live: true,
  },
  { conversation: aConversation({ id: "c2" }), tasks: [], repo: "no repo", live: false },
];

const BOOK: StepsBook = new Map([
  ["a", { steps: [aStep({ id: "1" }), aStep({ id: "2" })], loading: false, error: null }],
  ["f", { steps: [aStep({ id: "3", startedAt: iso(9, 3), endedAt: iso(10, 3) })], loading: false, error: null }],
]);

describe("traceStats", () => {
  it("counts conversations, tasks, running agents, failing, merged today and tool calls", () => {
    expect(traceStats(ENTRIES, BOOK, NOW)).toEqual([
      { label: "conversations", value: 2 },
      { label: "tasks", value: 6 },
      { label: "agents running", value: 2 },
      { label: "failing", value: 2 },
      { label: "merged today", value: 1 },
      { label: "tool calls", value: 3 },
    ]);
  });
});

describe("headCaption", () => {
  const axis = (today: boolean): TimeAxis => ({ start: new Date(2026, 10, 2).getTime(), end: NOW, step: 1, ticks: 1, now: NOW, today });

  it("counts conversations and repos and names the window", () => {
    expect(headCaption(ENTRIES, null, axis(true))).toBe("2 conversations · 2 repos · today");
    expect(headCaption(ENTRIES.slice(1), null, axis(false))).toBe("1 conversation · 0 repos · since 2 nov");
    expect(headCaption([{ ...ENTRIES[1]!, tasks: [aTask()] }], null, axis(true))).toBe("1 conversation · 1 repo · today");
  });

  it("names the one conversation it is filtered to", () => {
    expect(headCaption(ENTRIES.slice(0, 1), "c1", axis(true))).toBe("login redirect loop · today");
    expect(headCaption([], "c9", axis(true))).toBe("today");
  });
});
