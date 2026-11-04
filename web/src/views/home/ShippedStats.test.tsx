import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import * as statsApi from "../../api/stats";
import type { ShippedDay, Stats } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { ShippedStats, STATS_TITLE } from "./ShippedStats";

afterEach(() => vi.restoreAllMocks());

const TODAY = new Date(2026, 10, 4, 14);

const day = (date: string, over: Partial<ShippedDay>): ShippedDay => ({
  date,
  fixes: 0,
  features: 0,
  lines: 0,
  agents: 0,
  ...over,
});

const THIS_YEAR: Stats = {
  days: [
    day("2026-11-04", { fixes: 2, features: 1, lines: 12_400, agents: 3 }),
    day("2026-06-02", { fixes: 1, agents: 1 }),
  ],
};

const TWO_YEARS: Stats = { days: [...THIS_YEAR.days, day("2025-03-12", { fixes: 4, lines: 80, agents: 2 })] };

function renderStats(stats: Stats | Error) {
  const get = vi.spyOn(statsApi, "getStats");
  if (stats instanceof Error) get.mockRejectedValue(stats);
  else get.mockResolvedValue(stats);
  return render(
    <TestDataProvider>
      <ShippedStats today={TODAY} />
    </TestDataProvider>,
  );
}

const tile = (label: string) => screen.getByRole("button", { name: new RegExp(`^${label}`) });

describe("ShippedStats", () => {
  it("renders nothing until the stats arrive", () => {
    vi.spyOn(statsApi, "getStats").mockReturnValue(new Promise(() => undefined));
    const { container } = render(
      <TestDataProvider>
        <ShippedStats today={TODAY} />
      </TestDataProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the head, one year, the tiles and the bug fixes heatmap", async () => {
    renderStats(THIS_YEAR);
    expect(await screen.findByRole("heading", { name: STATS_TITLE })).toBeInTheDocument();
    const period = screen.getByRole("radiogroup", { name: "Period" });
    expect(within(period).getAllByRole("radio").map((r) => r.parentElement?.textContent)).toEqual(["2026"]);
    expect(tile("bug fixes")).toHaveTextContent("bug fixes3+3 vs 2025");
    expect(tile("bug fixes")).toHaveAttribute("aria-pressed", "true");
    expect(tile("lines shipped")).toHaveTextContent("+12.4K");
    expect(tile("agents run")).toHaveTextContent("4");
    expect(screen.getByText("3 bug fixes shipped in the last 12 months")).toBeInTheDocument();
  });

  it("charts the number of the clicked tile", async () => {
    renderStats(THIS_YEAR);
    fireEvent.click(await screen.findByRole("button", { name: /^agents run/ }));
    expect(tile("agents run")).toHaveAttribute("aria-pressed", "true");
    expect(tile("bug fixes")).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("4 agents run in the last 12 months")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "agents by day" })).toBeInTheDocument();
  });

  it("offers the previous year when it has activity and switches to it", async () => {
    renderStats(TWO_YEARS);
    const earlier = await screen.findByRole("radio", { name: "2025" });
    expect(tile("bug fixes")).toHaveTextContent("−1 vs 2025");
    fireEvent.click(earlier);
    expect(screen.getByRole("radio", { name: "2025" })).toBeChecked();
    expect(tile("bug fixes")).toHaveTextContent("4no 2024 data");
    expect(screen.getByText("4 bug fixes shipped in 2025")).toBeInTheDocument();
  });

  it("is a quiet line when nothing has shipped", async () => {
    renderStats({ days: [day("2026-11-01", {})] });
    expect(await screen.findByText("nothing shipped yet")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: STATS_TITLE })).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });

  it("says what went wrong when the stats do not load", async () => {
    renderStats(new ApiError("network", "cannot reach the daemon"));
    expect(await screen.findByText("error: cannot reach the daemon")).toBeInTheDocument();
  });
});
