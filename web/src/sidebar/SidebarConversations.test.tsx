import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as conversationsApi from "../api/conversations";
import { ApiError } from "../api/errors";
import { aConversation, aTask } from "../api/fixtures";
import { BY_REPO_KEY } from "./byRepoPref";
import { CONVERSATIONS, renderSidebar, rowButton, rowTitles, scrollArea, stubDaemon } from "./sidebarTestKit";

beforeEach(() => window.history.replaceState(null, "", "/c/login"));

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

const ready = () => screen.findByRole("button", { name: /^login redirect loop/ });

describe("conversation rows", () => {
  it("lists active rows that need you first, then newest, and ended ones under older", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    expect(rowTitles()).toEqual([
      "login redirect loop",
      "upgrade to vite 8",
      "flaky ci on main",
      "i18n keys cleanup",
      "checkout coupon rounding",
    ]);
    expect(within(scrollArea()).getByRole("heading", { name: /^active 4/ })).toBeInTheDocument();
    expect(within(scrollArea()).getByRole("heading", { name: "older 1" })).toBeInTheDocument();
  });

  it("shows one glyph, the age, a short status and the repo per row", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    const login = rowButton("login redirect loop");
    expect(login).toHaveAttribute("data-state", "waiting");
    expect(login).toHaveAttribute("aria-current", "page");
    expect(login).toHaveTextContent("login redirect loop30mfix-login needs youtownhall");
    expect(login.querySelector(".th-status")).toHaveAttribute("data-state", "waiting");
    expect(rowButton("flaky ci on main")).toHaveTextContent("e2e-smoke failedshop");
    expect(rowButton("flaky ci on main")).toHaveAttribute("data-state", "error");
    expect(rowButton("upgrade to vite 8").querySelector(".th-status .th-spin")).not.toBeNull();
    expect(rowButton("i18n keys cleanup")).toHaveTextContent("1hyour turn · 1 runningacme-access");
    const coupon = rowButton("checkout coupon rounding");
    expect(coupon).toHaveAttribute("data-state", "ended");
    expect(coupon).toHaveTextContent("2dended · 1 mergedshop");
  });

  it("opens a conversation on click", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    fireEvent.click(rowButton("flaky ci on main"));
    expect(window.location.pathname).toBe("/c/ci");
    expect(rowButton("flaky ci on main")).toHaveAttribute("aria-current", "page");
  });

  it("follows live task updates", async () => {
    stubDaemon();
    const { events } = renderSidebar();
    await ready();
    act(() => events.emit({ type: "taskUpdated", task: aTask({ name: "fix-login", conversationId: "login", repo: "townhall", state: "working" }) }));
    expect(rowButton("login redirect loop")).toHaveTextContent("your turn · 1 running");
    expect(rowTitles()[0]).toBe("upgrade to vite 8");
  });

  it("says when there are no conversations", async () => {
    stubDaemon([], []);
    renderSidebar();
    expect(await screen.findByText("no conversations yet")).toBeInTheDocument();
  });

  it("shows a failed load", async () => {
    stubDaemon().list.mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    renderSidebar();
    expect(await within(scrollArea()).findByRole("alert")).toHaveTextContent("error: cannot reach the daemon: down");
  });
});

describe("by repo", () => {
  it("groups active rows under foldable repo heads and remembers the choice", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    const toggle = screen.getByRole("button", { name: "by repo" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(localStorage.getItem(BY_REPO_KEY)).toBe("true");
    const heads = [...scrollArea().querySelectorAll(".th-group-head")].map((h) => h.textContent);
    expect(heads).toEqual(["townhall1", "shop1", "acme-access1", "no repo1"]);
    expect(rowButton("login redirect loop").querySelector(".th-convo-repo")).toHaveTextContent("");
    expect(rowButton("checkout coupon rounding")).toHaveTextContent("shop");
  });

  it("folds a group but keeps the current row", async () => {
    localStorage.setItem(BY_REPO_KEY, "true");
    stubDaemon([...CONVERSATIONS, aConversation({ id: "docs", title: "docs sweep", linkedRepo: "townhall" })]);
    renderSidebar();
    await ready();
    const head = screen.getByRole("button", { name: "townhall 2" });
    expect(head).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(head);
    expect(head).toHaveAttribute("aria-expanded", "false");
    expect(rowTitles()).toContain("login redirect loop");
    expect(rowTitles()).not.toContain("docs sweep");
    fireEvent.click(head);
    expect(rowTitles()).toContain("docs sweep");
  });
});

describe("row menu", () => {
  it("ends a conversation from a right-click", async () => {
    stubDaemon();
    const end = vi.spyOn(conversationsApi, "endConversation").mockResolvedValue(aConversation({ id: "ci", ended: true }));
    renderSidebar();
    await ready();
    fireEvent.contextMenu(rowButton("flaky ci on main"), { clientX: 40, clientY: 300 });
    const menu = screen.getByRole("menu", { name: "conversation actions" });
    expect([...menu.querySelectorAll(".th-option")].map((item) => item.textContent)).toEqual([
      "end conversation",
      "repo: autoshop",
      "repo: shopdevelop",
      "repo: townhallmain",
      "repo: acme-accessstaging",
    ]);
    fireEvent.click(within(menu).getByText("end conversation"));
    expect(end).toHaveBeenCalledWith("ci");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("links a repo from the keyboard and returns focus to the row", async () => {
    stubDaemon();
    const patch = vi
      .spyOn(conversationsApi, "patchConversation")
      .mockRejectedValue(new ApiError("http", "repo `townhall` not found", 404));
    renderSidebar();
    await ready();
    const row = rowButton("i18n keys cleanup");
    row.focus();
    fireEvent.keyDown(row, { key: "F10", shiftKey: true });
    const menu = screen.getByRole("menu");
    expect(within(menu).getByRole("menuitemradio", { name: /repo: acme-access/ })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(menu).getByText("repo: townhall"));
    expect(patch).toHaveBeenCalledWith("i18n", { linkedRepo: "townhall" });
    expect(row).toHaveFocus();
    expect(await within(scrollArea()).findByRole("alert")).toHaveTextContent("error: repo `townhall` not found");
  });

  it("closes on escape", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    fireEvent.keyDown(rowButton("login redirect loop"), { key: "ContextMenu" });
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(rowButton("login redirect loop")).toHaveFocus();
  });
});
