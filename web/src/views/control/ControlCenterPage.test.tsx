import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { aTask } from "../../api/fixtures";
import { createFakeEvents } from "../../data/testing";
import { shownHints } from "../../test/renderWithHints";
import { findItem, item, items, NOW, renderControl, stubDaemon, TASKS, url } from "./controlTestKit";

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const statValues = (container: HTMLElement) =>
  [...container.querySelectorAll(".th-trace-stat")].map((s) => [s.querySelector("span")?.textContent, s.querySelector("b")?.textContent]);

describe("ControlCenterPage timeline", () => {
  it("shows what ran today as conversations with their tasks, oldest first", async () => {
    stubDaemon();
    renderControl();
    await findItem("login redirect loop");
    expect(items()).toEqual(["c:c1", "t:fix-login", "t:auth-tests", "t:logout-cookie", "c:c2", "t:e2e-smoke"]);
    expect(screen.queryByText("old and done")).toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "control center" })).toBeInTheDocument();
    expect(screen.getByText("2 conversations · 2 repos · today")).toHaveClass("th-control-cap");
    expect(screen.getByText("● live")).toHaveAttribute("data-status", "open");
    expect(item("login redirect loop")).toHaveAttribute("aria-level", "1");
    expect(item("fix-login")).toHaveAttribute("aria-level", "2");
    expect(item("fix-login").querySelector(".th-status")).toHaveAttribute("data-state", "waiting");
    expect(within(item("fix-login")).getByText("needs you")).toHaveClass("th-tag", "th-tag--waiting");
    expect(item("logout-cookie").querySelector(".th-bar")).toHaveAttribute("data-state", "queued");
    expect(within(item("logout-cookie")).getByText("after fix-login")).toHaveClass("th-bar-label");
  });

  it("draws the axis, the now line and the loop strip with stats from the steps", async () => {
    const getSteps = stubDaemon();
    const { container } = renderControl();
    await waitFor(() => expect(item("e2e-smoke")).toHaveAccessibleName(/exited 1/));
    expect(container.querySelector(".th-span--axis")).toHaveTextContent("conversation · task");
    expect(container.querySelectorAll(".th-tick").length).toBeGreaterThan(1);
    expect(container.querySelector(".th-now")).toHaveAttribute("aria-hidden", "true");
    const loop = screen.getByLabelText("Tasks per stage");
    await waitFor(() => expect(loop).toHaveTextContent("plan 1›code 0›test 2›review 1›ship 0needs you 1"));
    expect(statValues(container)).toEqual([
      ["conversations", "2"],
      ["tasks", "4"],
      ["agents running", "2"],
      ["failing", "1"],
      ["merged today", "0"],
      ["tool calls", "4"],
    ]);
    expect(getSteps.mock.calls.map(([agent]) => agent).sort()).toEqual(["auth-tests", "e2e-smoke", "fix-login"]);
    expect(within(item("auth-tests")).getByText("test")).toHaveClass("th-span-meta");
  });

  it("unfolds a task's steps from its toggle and folds a conversation", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    const task = await findItem("auth-tests");
    expect(task).toHaveAttribute("aria-expanded", "false");
    await user.click(task.querySelector(".th-span-toggle") as HTMLElement);
    expect(url()).toBe("/townhall");
    expect(item("auth-tests")).toHaveAttribute("aria-expanded", "true");
    expect(item(/^run tests npm test -- auth/)).toHaveAttribute("aria-level", "3");
    expect(item(/^read src\/auth\/session\.ts/)).toBeInTheDocument();
    await user.click(item("login redirect loop").querySelector(".th-span-toggle") as HTMLElement);
    expect(items()).toEqual(["c:c1", "c:c2", "t:e2e-smoke"]);
  });

  it("follows live task updates", async () => {
    stubDaemon();
    const events = createFakeEvents();
    renderControl("/townhall", events);
    await findItem("fix-login");
    const merged = { ...(TASKS[0] as (typeof TASKS)[number]), state: "merged" as const };
    act(() => events.emit({ type: "taskUpdated", task: merged }));
    await waitFor(() => expect(item("fix-login").querySelector(".th-status")).toHaveAttribute("data-state", "merged"));
  });
});

describe("ControlCenterPage keys", () => {
  it("moves with up and down, folds with left and right and opens with enter", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    await findItem("login redirect loop");
    expect(shownHints()).toBe("↑↓ ←→ ⏎ f");
    await user.keyboard("{ArrowDown}");
    expect(item("login redirect loop")).toHaveFocus();
    await user.keyboard("{ArrowDown}{ArrowRight}");
    expect(item("fix-login")).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{ArrowRight}");
    expect(item(/^edit src\/auth\/redirect\.ts/)).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(item("fix-login")).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(item("fix-login")).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{Enter}");
    expect(url()).toBe("/c/c1?t=fix-login");
  });

  it("only folds a task row on click without navigating", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    await user.click(await findItem("auth-tests"));
    expect(item("auth-tests")).toHaveAttribute("aria-expanded", "true");
    expect(url()).toBe("/townhall");
    await user.click(item("auth-tests"));
    expect(item("auth-tests")).toHaveAttribute("aria-expanded", "false");
    expect(url()).toBe("/townhall");
  });

  it("opens a conversation row on click", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    await user.click(await findItem("flaky ci on main"));
    expect(url()).toBe("/c/c2");
  });
});

describe("ControlCenterPage filters", () => {
  it("shows one conversation from ?c= and links back to all", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl("/townhall?c=c2&repo=shop");
    await findItem("flaky ci on main");
    expect(items()).toEqual(["c:c2", "t:e2e-smoke"]);
    expect(screen.getByText("flaky ci on main · today")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "all conversations" }));
    expect(url()).toBe("/townhall?repo=shop");
  });

  it("shows an older conversation from ?c= since its day", async () => {
    stubDaemon();
    renderControl("/townhall?c=c3");
    await findItem("old and done");
    expect(screen.getByText("old and done · since 2 nov")).toBeInTheDocument();
  });

  it("opens the repo menu with f and keeps the choice in the url", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    await findItem("fix-login");
    await user.keyboard("f");
    const menu = await screen.findByRole("menu", { name: "filter by repo" });
    expect(within(menu).getByRole("menuitemradio", { name: /all repos/ })).toHaveAttribute("aria-checked", "true");
    await user.click(within(menu).getByRole("menuitemradio", { name: /shop/ }));
    expect(url()).toBe("/townhall?repo=shop");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(items()).toEqual(["c:c2", "t:e2e-smoke"]);
    const button = screen.getByRole("button", { name: "shop ▾" });
    expect(button).toHaveFocus();
    await user.click(button);
    await user.click(screen.getByRole("menuitemradio", { name: /all repos/ }));
    expect(url()).toBe("/townhall");
  });

  it("closes the repo menu when its button is pressed again", async () => {
    stubDaemon();
    const user = userEvent.setup();
    renderControl();
    await findItem("fix-login");
    const button = screen.getByRole("button", { name: "all repos ▾" });
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    await user.click(button);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveAttribute("aria-expanded", "false");
  });
});

describe("ControlCenterPage empty, loading and errors", () => {
  it("says nothing ran today with a link home and only the filter key", async () => {
    stubDaemon({ conversations: [], tasks: [] });
    renderControl();
    expect(await screen.findByRole("heading", { name: "nothing ran today" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "new conversation" })).toHaveAttribute("href", "/");
    expect(screen.queryByLabelText("Tasks per stage")).toBeNull();
    expect(document.querySelector(".th-control-cap")).toHaveTextContent(/^today$/);
    expect(shownHints()).toBe("f");
  });

  it("names the repo filter when it leaves nothing", async () => {
    stubDaemon();
    renderControl("/townhall?repo=data-hub");
    expect(await screen.findByText("in data-hub")).toBeInTheDocument();
  });

  it("offers all conversations when ?c= finds nothing", async () => {
    stubDaemon();
    renderControl("/townhall?c=nope");
    expect(await screen.findByRole("heading", { name: "nothing to show for this conversation" })).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "all conversations" })).toHaveLength(2);
  });

  it("shows loading timeline… until the first answer", async () => {
    stubDaemon({ conversations: "pending" });
    renderControl();
    expect(await screen.findByText("loading timeline…", {}, { timeout: 1000 })).toBeInTheDocument();
    expect(screen.queryByText(/today/)).toBeNull();
  });

  it("shows the load error", async () => {
    stubDaemon({ conversations: new ApiError("network", "cannot reach the daemon") });
    renderControl();
    expect(await screen.findByRole("heading", { name: "error: cannot reach the daemon" })).toBeInTheDocument();
  });

  it("says steps could not load under an unfolded task", async () => {
    const getSteps = stubDaemon({ tasks: [aTask({ name: "x", conversationId: "c1", state: "ended" })] });
    getSteps.mockRejectedValue(new ApiError("http", "agent not found", 404));
    const user = userEvent.setup();
    renderControl();
    await user.click((await findItem("x")).querySelector(".th-span-toggle") as HTMLElement);
    expect(await screen.findByText("error: agent not found")).toBeInTheDocument();
  });
});
