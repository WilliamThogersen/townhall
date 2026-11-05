import { describe, expect, it } from "vitest";
import { aConversation, aTask } from "../api/fixtures";
import type { TaskState } from "../api/types";
import { summarizeRow } from "./rowSummary";

const tasks = (...states: TaskState[]) =>
  states.map((state, i) => aTask({ name: `task-${i}`, state }));

describe("summarizeRow", () => {
  it("puts a task that needs you first, by name", () => {
    const row = summarizeRow(aConversation({ status: "thinking" }), [
      aTask({ name: "e2e-smoke", state: "error" }),
      aTask({ name: "fix-login", state: "needsYou" }),
    ]);
    expect(row).toEqual({ glyph: "waiting", state: "waiting", status: "fix-login needs you", needsYou: true });
  });

  it("counts several tasks that need you", () => {
    const row = summarizeRow(aConversation(), tasks("needsYou", "needsYou", "working"));
    expect(row.status).toBe("2 tasks need you");
  });

  it("names a failed task, then counts several", () => {
    const one = summarizeRow(aConversation(), [aTask({ name: "e2e-smoke", state: "error" })]);
    expect(one).toEqual({ glyph: "error", state: "error", status: "e2e-smoke failed", needsYou: false });
    expect(summarizeRow(aConversation(), tasks("error", "error")).status).toBe("2 tasks failed");
  });

  it("shows thinking with a spinner while a turn runs", () => {
    const row = summarizeRow(aConversation({ status: "thinking" }), tasks("working"));
    expect(row).toEqual({ glyph: "working", state: null, status: "thinking", needsYou: false });
  });

  it("counts running and merged tasks on your turn", () => {
    const row = summarizeRow(aConversation(), tasks("working", "queued", "inCi", "inReview", "merged", "ended"));
    expect(row).toEqual({ glyph: "alive", state: null, status: "your turn · 4 running · 1 merged", needsYou: false });
  });

  it("is idle on your turn with nothing running", () => {
    const row = summarizeRow(aConversation(), []);
    expect(row).toEqual({ glyph: "dead", state: null, status: "your turn", needsYou: false });
  });

  it("marks an ended conversation and keeps its counts", () => {
    const row = summarizeRow(aConversation({ ended: true }), tasks("merged", "merged"));
    expect(row).toEqual({ glyph: "dead", state: "ended", status: "ended · 2 merged", needsYou: false });
    expect(summarizeRow(aConversation({ ended: true }), tasks("working")).status).toBe("ended · 1 running");
  });

  it("lets attention win over ended", () => {
    const row = summarizeRow(aConversation({ ended: true }), tasks("needsYou"));
    expect(row.state).toBe("waiting");
  });
});
