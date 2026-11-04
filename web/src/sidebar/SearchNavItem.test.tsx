import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import * as searchApi from "../api/search";
import type { SearchHit } from "../api/search";
import { navigate } from "../router";
import { renderSidebar, rowTitles, scrollArea, shownKeyHints, stubDaemon } from "./sidebarTestKit";
import { NARROW_QUERY } from "./useSidebarSearch";

const hitFor = (conversationId: string): SearchHit => ({
  conversationId,
  title: conversationId,
  match: "message",
  entryId: 2,
  snippet: "…coupon…",
  updatedAt: "2026-11-04T10:42:05.000Z",
});

beforeEach(() => window.history.replaceState(null, "", "/c/login"));

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const ready = () => screen.findByRole("button", { name: /^login redirect loop/ });
const field = () => screen.getByRole("searchbox", { name: "search conversations" });
const searchLink = () => screen.queryByRole("link", { name: /search/ });

async function openField() {
  await ready();
  fireEvent.keyDown(document.body, { key: "/" });
  return field();
}

describe("sidebar search", () => {
  it("opens and focuses the field on / and shows its keys", async () => {
    stubDaemon();
    renderSidebar();
    const input = await openField();
    expect(input).toHaveFocus();
    expect(searchLink()).not.toBeInTheDocument();
    expect(window.location.pathname).toBe("/c/login");
    expect(shownKeyHints()).toBe("⏎ esc ⌥↑ ⌥↓");
  });

  it("leaves / alone inside a text field and while the sidebar is hidden", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    const other = document.createElement("input");
    document.body.append(other);
    fireEvent.keyDown(other, { key: "/" });
    expect(searchLink()).toBeInTheDocument();
    other.remove();
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query === NARROW_QUERY }));
    fireEvent.keyDown(document.body, { key: "/" });
    expect(searchLink()).toBeInTheDocument();
  });

  it("filters by title at once and by message text when the daemon answers", async () => {
    stubDaemon();
    const search = vi.spyOn(searchApi, "searchConversations").mockResolvedValue([hitFor("ci")]);
    renderSidebar();
    fireEvent.change(await openField(), { target: { value: "  COUPON " } });
    expect(rowTitles()).toEqual(["checkout coupon rounding"]);
    await waitFor(() => expect(rowTitles()).toEqual(["flaky ci on main", "checkout coupon rounding"]));
    expect(search).toHaveBeenCalledOnce();
    expect(search.mock.calls[0]?.[0]).toBe("COUPON");
  });

  it("says when nothing matches and warns when message search fails", async () => {
    stubDaemon();
    vi.spyOn(searchApi, "searchConversations").mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    renderSidebar();
    fireEvent.change(await openField(), { target: { value: "zebra" } });
    expect(within(scrollArea()).getByText("no conversations match zebra")).toBeInTheDocument();
    expect(await within(scrollArea()).findByText("warn: message search failed: cannot reach the daemon: down")).toBeInTheDocument();
  });

  it("opens all results on enter and clears on escape", async () => {
    stubDaemon();
    vi.spyOn(searchApi, "searchConversations").mockResolvedValue([]);
    renderSidebar();
    const input = await openField();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(window.location.pathname).toBe("/c/login");
    fireEvent.change(input, { target: { value: "coupon rounding" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(`${window.location.pathname}${window.location.search}`).toBe("/search?q=coupon%20rounding");
    expect(field()).toHaveValue("coupon rounding");
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(searchLink()).toHaveAttribute("aria-current", "page");
    expect(rowTitles()).toHaveLength(5);
  });

  it("keeps a filled field open on blur and closes an empty one", async () => {
    stubDaemon();
    vi.spyOn(searchApi, "searchConversations").mockResolvedValue([]);
    renderSidebar();
    const input = await openField();
    fireEvent.change(input, { target: { value: "ci" } });
    fireEvent.blur(input);
    expect(field()).toHaveValue("ci");
    fireEvent.change(field(), { target: { value: "" } });
    fireEvent.blur(field());
    expect(searchLink()).toBeInTheDocument();
  });

  it("opens the field from the search link", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    fireEvent.click(searchLink() as HTMLElement);
    expect(window.location.pathname).toBe("/search");
    expect(field()).toHaveFocus();
  });
});

describe("previous and next conversation", () => {
  it("moves with option and the arrows, from any focus", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    fireEvent.keyDown(document.body, { key: "ArrowDown", altKey: true });
    expect(window.location.pathname).toBe("/c/vite");
    fireEvent.keyDown(document.body, { key: "ArrowUp", altKey: true });
    fireEvent.keyDown(document.body, { key: "ArrowUp", altKey: true });
    expect(window.location.pathname).toBe("/c/login");
    fireEvent.keyDown(document.body, { key: "ArrowDown" });
    fireEvent.keyDown(document.body, { key: "ArrowDown", altKey: true, shiftKey: true });
    expect(window.location.pathname).toBe("/c/login");
  });

  it("starts at the first or last row when no conversation is open", async () => {
    stubDaemon();
    renderSidebar();
    await ready();
    act(() => navigate("/townhall"));
    fireEvent.keyDown(document.body, { key: "ArrowUp", altKey: true });
    expect(window.location.pathname).toBe("/c/coupon");
    act(() => navigate("/townhall"));
    fireEvent.keyDown(document.body, { key: "ArrowDown", altKey: true });
    expect(window.location.pathname).toBe("/c/login");
  });
});
