import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import * as conversations from "../../api/conversations";
import { aConversation } from "../../api/fixtures";
import * as library from "../../api/library";
import type { AgentUsageSeries, UsageRange } from "../../api/types";
import { UsageGraph } from "./UsageGraph";

const HOUR = 3_600_000;

const series = (range: UsageRange, over: Partial<AgentUsageSeries> = {}): AgentUsageSeries => ({
  agent: "builder",
  range,
  bucketMs: HOUR,
  since: "2026-11-05T00:00:00.000Z",
  until: "2026-11-05T03:00:00.000Z",
  runs: 3,
  tokens: 12_345,
  buckets: [
    { start: "2026-11-05T00:00:00.000Z", runs: 1, tokens: 345 },
    { start: "2026-11-05T01:00:00.000Z", runs: 0, tokens: 0 },
    { start: "2026-11-05T02:00:00.000Z", runs: 2, tokens: 12_000 },
  ],
  byTask: [
    { key: "fix-login", runs: 1, tokens: 12_000 },
    { key: "add-docs", runs: 2, tokens: 345 },
  ],
  byRepo: [{ key: "shop", runs: 3, tokens: 12_345 }],
  byConversation: [{ key: "conv-1", runs: 3, tokens: 12_345 }],
  ...over,
});

beforeEach(() => {
  vi.spyOn(conversations, "listConversations").mockResolvedValue([]);
  vi.spyOn(library, "getAgentUsageSeries").mockImplementation((_name, range) => Promise.resolve(series(range)));
});

afterEach(() => vi.restoreAllMocks());

describe("UsageGraph", () => {
  it("loads the week by default and says what the range totals", async () => {
    render(<UsageGraph name="builder" />);
    expect(await screen.findByText(/3 runs · 12,345 tokens/)).toBeInTheDocument();
    expect(library.getAgentUsageSeries).toHaveBeenCalledWith("builder", "7d", expect.anything());
    expect(screen.getByRole("button", { name: "7d" })).toHaveAttribute("aria-pressed", "true");
  });

  it("reloads when another range is picked", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    await userEvent.click(screen.getByRole("button", { name: "24h" }));
    await waitFor(() => expect(library.getAgentUsageSeries).toHaveBeenCalledWith("builder", "24h", expect.anything()));
    expect(screen.getByRole("button", { name: "24h" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "7d" })).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(screen.getByRole("button", { name: "30d" }));
    await waitFor(() => expect(library.getAgentUsageSeries).toHaveBeenCalledWith("builder", "30d", expect.anything()));
  });

  it("draws one chart with a bar per bucket and switches it between tokens and runs", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    expect(screen.getByRole("button", { name: "tokens" })).toHaveAttribute("aria-pressed", "true");
    expect(within(screen.getByRole("group", { name: "tokens over time" })).getAllByRole("button")).toHaveLength(3);
    expect(screen.queryByRole("group", { name: "runs over time" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "runs" }));
    expect(screen.getByRole("button", { name: "runs" })).toHaveAttribute("aria-pressed", "true");
    expect(within(screen.getByRole("group", { name: "runs over time" })).getAllByRole("button")).toHaveLength(3);
    expect(screen.queryByRole("group", { name: "tokens over time" })).not.toBeInTheDocument();
  });

  it("labels the top of the chart with the peak and the bottom with zero", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    expect(screen.getByTestId("usage-y-max")).toHaveTextContent("12.0K");
    await userEvent.click(screen.getByRole("button", { name: "runs" }));
    expect(screen.getByTestId("usage-y-max")).toHaveTextContent("2");
  });

  it("puts time labels under the bars", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    await userEvent.click(screen.getByRole("button", { name: "24h" }));
    await waitFor(() => expect(library.getAgentUsageSeries).toHaveBeenCalledWith("builder", "24h", expect.anything()));
    const axis = screen.getByTestId("usage-x-axis");
    expect(axis).toHaveTextContent("00:00");
    expect(axis).toHaveTextContent("02:00");
  });

  it("merges the daemon's buckets into fewer, wider bars", async () => {
    const buckets = Array.from({ length: 30 }, (_, i) => ({
      start: new Date(Date.UTC(2026, 9, 11 + i)).toISOString(),
      runs: 1,
      tokens: 10,
    }));
    vi.spyOn(library, "getAgentUsageSeries").mockResolvedValue(series("30d", { bucketMs: 24 * HOUR, runs: 30, tokens: 300, buckets }));
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    const bars = within(screen.getByRole("group", { name: "tokens over time" })).getAllByRole("button");
    expect(bars).toHaveLength(15);
    expect(bars[0]).toHaveAccessibleName("2026-10-11 00:00 → 2026-10-13 00:00 UTC: 20 tokens");
  });

  it("shows the exact time and value of a bar on hover and goes back on leave", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    const bars = within(screen.getByRole("group", { name: "tokens over time" })).getAllByRole("button");
    await userEvent.hover(bars[2]!);
    expect(screen.getByText("2026-11-05 02:00–03:00 UTC · 2 runs · 12,000 tokens")).toBeInTheDocument();
    await userEvent.unhover(bars[2]!);
    expect(screen.getByText(/last 7d: 3 runs · 12,345 tokens/)).toBeInTheDocument();
  });

  it("selects a bar with focus or a click and keeps a clicked one until it is clicked again", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    await userEvent.click(screen.getByRole("button", { name: "runs" }));
    const bars = within(screen.getByRole("group", { name: "runs over time" })).getAllByRole("button");
    await userEvent.click(bars[0]!);
    await userEvent.unhover(bars[0]!);
    expect(screen.getByText("2026-11-05 00:00–01:00 UTC · 1 run · 345 tokens")).toBeInTheDocument();
    expect(bars[0]).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(bars[0]!);
    await userEvent.unhover(bars[0]!);
    act(() => bars[0]!.blur());
    expect(screen.getByText(/last 7d/)).toBeInTheDocument();
    expect(bars[0]).toHaveAttribute("aria-pressed", "false");
    act(() => bars[1]!.focus());
    expect(await screen.findByText("2026-11-05 01:00–02:00 UTC · 0 runs · 0 tokens")).toBeInTheDocument();
  });

  it("gives every bar an accessible name with its time and value", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    expect(screen.getByRole("button", { name: "2026-11-05 02:00–03:00 UTC: 12,000 tokens" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "runs" }));
    expect(screen.getByRole("button", { name: "2026-11-05 02:00–03:00 UTC: 2 runs" })).toBeInTheDocument();
  });

  it("shows a conversation by its title when it is known and by its id otherwise", async () => {
    vi.spyOn(conversations, "listConversations").mockResolvedValue([aConversation({ id: "conv-1", title: "usage meter" })]);
    vi.spyOn(library, "getAgentUsageSeries").mockResolvedValue(
      series("7d", {
        byConversation: [
          { key: "conv-1", runs: 3, tokens: 12_000 },
          { key: "conv-2", runs: 1, tokens: 345 },
        ],
      }),
    );
    render(<UsageGraph name="builder" />);
    const list = within(await screen.findByRole("list", { name: "by conversation" }));
    expect(await list.findByText("usage meter")).toBeInTheDocument();
    expect(list.getByText("conv-2")).toBeInTheDocument();
    expect(list.queryByText("conv-1")).not.toBeInTheDocument();
  });

  it("lists what used the type by task, repo and conversation", async () => {
    render(<UsageGraph name="builder" />);
    await screen.findByText(/^last \d+[hd]:/);
    const tasks = within(screen.getByRole("list", { name: "by task" }));
    expect(tasks.getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "fix-login12,000 tokens · 1 run",
      "add-docs345 tokens · 2 runs",
    ]);
    expect(within(screen.getByRole("list", { name: "by repo" })).getByText("shop")).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "by conversation" })).getByText("conv-1")).toBeInTheDocument();
  });

  it("says so when nothing ran in the range", async () => {
    vi.spyOn(library, "getAgentUsageSeries").mockResolvedValue(
      series("7d", { runs: 0, tokens: 0, buckets: [], byTask: [], byRepo: [], byConversation: [] }),
    );
    render(<UsageGraph name="builder" />);
    expect(await screen.findByText("no runs in the last 7d")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "runs over time" })).not.toBeInTheDocument();
  });

  it("shows the error when the series fails to load", async () => {
    vi.spyOn(library, "getAgentUsageSeries").mockRejectedValue(new ApiError("network", "cannot reach the daemon"));
    render(<UsageGraph name="builder" />);
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot reach the daemon");
  });
});
