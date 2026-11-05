import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import { aUsage } from "../api/fixtures";
import type { ClaudeUsage } from "../api/types";
import * as usageApi from "../api/usage";
import { TestDataProvider } from "../data/testing";
import { UsageMeter } from "./UsageMeter";

const local = (day: number, hour: number, minute = 0) => new Date(2026, 10, day, hour, minute).getTime();
const NOW = local(5, 12, 39);

const reading = (over: Partial<ClaudeUsage> = {}): ClaudeUsage =>
  aUsage({
    session: { usedPercent: 30, resetsAt: local(5, 15, 10), ratePerHour: 1 },
    weekly: { usedPercent: 54, resetsAt: local(11, 8), ratePerHour: 1 },
    updatedAt: NOW,
    ...over,
  });

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function renderMeter(usage: ClaudeUsage | Error) {
  const get = vi.spyOn(usageApi, "getUsage");
  if (usage instanceof Error) get.mockRejectedValue(usage);
  else get.mockResolvedValue(usage);
  return render(
    <TestDataProvider>
      <UsageMeter />
    </TestDataProvider>,
  );
}

const button = () => screen.findByRole("button", { name: "Claude plan usage" });

describe("UsageMeter", () => {
  it("shows the session and the week as two thin bars", async () => {
    const { container } = renderMeter(reading());
    await button();
    expect(screen.getByText("30% 5h")).toBeInTheDocument();
    expect(screen.getByText("54% wk")).toBeInTheDocument();
    const fills = [...container.querySelectorAll<HTMLElement>(".th-usage-fill")];
    expect(fills.map((f) => f.style.getPropertyValue("--used"))).toEqual(["30%", "54%"]);
  });

  it("describes each bar in its tooltip", async () => {
    renderMeter(reading());
    await button();
    expect(screen.getByText("30% 5h").closest(".th-usage-meter")).toHaveAttribute(
      "title",
      "5h 30% used · resets 15:10 (in 2h 31m)",
    );
  });

  it("shows nothing until the usage is known, and nothing without a token", async () => {
    const { container } = renderMeter(reading({ session: null, weekly: null, error: "Sign in to Claude Code to see usage" }));
    await vi.waitFor(() => expect(usageApi.getUsage).toHaveBeenCalled());
    await Promise.resolve();
    expect(container).toBeEmptyDOMElement();
  });

  it("shows nothing when the daemon cannot be asked", async () => {
    const { container } = renderMeter(new ApiError("network", "cannot reach the daemon: down"));
    await vi.waitFor(() => expect(usageApi.getUsage).toHaveBeenCalled());
    await Promise.resolve();
    expect(container).toBeEmptyDOMElement();
  });

  it("shows one bar when only one window is known", async () => {
    renderMeter(reading({ weekly: null }));
    await button();
    expect(screen.getByText("30% 5h")).toBeInTheDocument();
    expect(screen.queryByText(/wk$/)).not.toBeInTheDocument();
  });

  it("turns amber at 60% and red at 80%", async () => {
    const { container } = renderMeter(
      reading({
        session: { usedPercent: 61, resetsAt: local(5, 15, 10), ratePerHour: null },
        weekly: { usedPercent: 80, resetsAt: local(11, 8), ratePerHour: null },
      }),
    );
    await button();
    const levels = [...container.querySelectorAll(".th-usage-meter")].map((m) => m.getAttribute("data-level"));
    expect(levels).toEqual(["warn", "hot"]);
  });

  it("shows a window whose reset has passed as empty", async () => {
    renderMeter(reading({ session: { usedPercent: 90, resetsAt: NOW - 60_000, ratePerHour: 3 } }));
    await button();
    expect(screen.getByText("0% 5h")).toBeInTheDocument();
  });

  it("dims the bars when the last refresh failed", async () => {
    const { container } = renderMeter(reading({ error: "Usage unavailable (offline?)" }));
    await button();
    expect(container.querySelector(".th-usage")).toHaveAttribute("data-stale", "true");
    expect(screen.getByText("30% 5h")).toBeInTheDocument();
  });

  it("marks a fresh reading as not stale", async () => {
    const { container } = renderMeter(reading());
    await button();
    expect(container.querySelector(".th-usage")).not.toHaveAttribute("data-stale");
  });
});

describe("UsageMeter popover", () => {
  it("opens on click with both windows and when they reset", async () => {
    renderMeter(reading());
    const trigger = await button();
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    const dialog = screen.getByRole("dialog", { name: "Claude plan usage" });
    expect(within(dialog).getByText("5 hours")).toBeInTheDocument();
    expect(within(dialog).getByText("30% used")).toBeInTheDocument();
    expect(within(dialog).getByText("resets 15:10 (in 2h 31m)")).toBeInTheDocument();
    expect(within(dialog).getByText("Week")).toBeInTheDocument();
    expect(within(dialog).getByText("54% used")).toBeInTheDocument();
    expect(within(dialog).getByText("resets Wed 08:00 (in 5d 19h)")).toBeInTheDocument();
    expect(within(dialog).getByText("Updated 12:39")).toBeInTheDocument();
  });

  it("warns on the window that will run out before it resets", async () => {
    renderMeter(
      reading({
        session: { usedPercent: 30, resetsAt: local(5, 15, 10), ratePerHour: 1 },
        weekly: { usedPercent: 54, resetsAt: local(11, 8), ratePerHour: 2 },
      }),
    );
    fireEvent.click(await button());
    const lasts = screen.getByText("lasts until reset");
    const runsOut = screen.getByText("runs out ~Fri 11:39 (in 23h 0m)");
    expect(lasts).not.toHaveAttribute("data-warn");
    expect(runsOut).toHaveAttribute("data-warn", "true");
  });

  it("says why the numbers are old", async () => {
    renderMeter(reading({ error: "Usage unavailable (offline?)", updatedAt: local(5, 11, 5) }));
    fireEvent.click(await button());
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Stale · Updated 11:05")).toBeInTheDocument();
    expect(within(dialog).getByText("Offline")).toBeInTheDocument();
  });

  it("closes on Escape and gives the focus back", async () => {
    renderMeter(reading());
    const trigger = await button();
    fireEvent.click(trigger);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("closes on Escape when the focus is outside the meter", async () => {
    renderMeter(reading());
    fireEvent.click(await button());
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    (document.activeElement as HTMLElement | null)?.blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("stops listening for Escape once closed", async () => {
    renderMeter(reading());
    const trigger = await button();
    fireEvent.click(trigger);
    fireEvent.click(trigger);
    const event = new KeyboardEvent("keydown", { key: "Escape", cancelable: true, bubbles: true });
    document.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it("closes on a click elsewhere, and on a second click on the bars", async () => {
    renderMeter(reading());
    const trigger = await button();
    fireEvent.click(trigger);
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(trigger);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole("dialog"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.mouseDown(trigger);
    fireEvent.click(trigger);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
