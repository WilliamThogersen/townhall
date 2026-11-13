import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { aTask } from "../api/fixtures";
import type { Task } from "../api/types";
import * as data from "../data";
import { ProgressOverview } from "./ProgressOverview";

function withTasks(tasks: readonly Task[], loading = false) {
  vi.spyOn(data, "useTasks").mockReturnValue({ tasks, loading, error: null });
}

describe("ProgressOverview", () => {
  it("shows running, waiting for you and recently landed counts at a glance", () => {
    withTasks([
      aTask({ name: "a", state: "working" }),
      aTask({ name: "b", state: "inCi" }),
      aTask({ name: "c", state: "needsYou", conversationId: "c2" }),
      aTask({ name: "d", state: "merged", createdAt: new Date().toISOString() }),
    ]);
    render(<ProgressOverview />);
    const region = screen.getByRole("group", { name: "work in progress" });
    expect(region).toHaveTextContent("2 running");
    expect(region).toHaveTextContent("1 needs you");
    expect(region).toHaveTextContent("1 landed");
  });

  it("shows nothing while loading or when there is nothing to report", () => {
    withTasks([], true);
    const { container, rerender } = render(<ProgressOverview />);
    expect(container).toBeEmptyDOMElement();
    withTasks([aTask({ state: "error" })]);
    rerender(<ProgressOverview />);
    expect(container).toBeEmptyDOMElement();
  });
});
