import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aTask } from "./fixtures";
import { getTask, listTasks } from "./tasks";

afterEach(() => vi.unstubAllGlobals());

describe("tasks api", () => {
  it("lists every task without a filter", async () => {
    const fn = mockFetch(200, [aTask()]);
    expect(await listTasks()).toEqual([aTask()]);
    expect(sent(fn).url).toBe("/api/tasks");
  });

  it("filters by conversation, repo and state", async () => {
    const fn = mockFetch(200, []);
    await listTasks({ conversation: "c1", repo: "my shop", state: "needsYou" });
    expect(sent(fn).url).toBe("/api/tasks?conversation=c1&repo=my+shop&state=needsYou");
  });

  it("gets one task by name", async () => {
    const task = aTask({ pr: { number: 4, url: "https://x/4", checks: "passed" } });
    const fn = mockFetch(200, task);
    expect(await getTask("fix/login")).toEqual(task);
    expect(sent(fn).url).toBe("/api/tasks/fix%2Flogin");
  });

  it("refuses an unknown task state", async () => {
    mockFetch(200, [{ ...aTask(), state: "paused" }]);
    await expect(listTasks()).rejects.toMatchObject({ kind: "protocol" });
  });
});
