import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import * as searchApi from "../../api/search";
import type { SearchHit } from "../../api/search";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { countLabel, SearchPage } from "./SearchPage";

const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();

const titleHit: SearchHit = {
  conversationId: "c1",
  title: "Checkout coupon rounding",
  match: "title",
  entryId: null,
  snippet: null,
  updatedAt: minutesAgo(9),
};

const messageHit: SearchHit = {
  conversationId: "c2",
  title: "flaky ci on main",
  match: "message",
  entryId: 4,
  snippet: "…the checkout test fails on main…",
  updatedAt: minutesAgo(180),
};

function open(path: string) {
  window.history.replaceState(null, "", path);
  return renderWithHints(<SearchPage />);
}

const marks = (el: HTMLElement) => [...el.querySelectorAll("mark")].map((m) => m.textContent);

beforeEach(() => window.history.replaceState(null, "", "/search"));
afterEach(() => vi.restoreAllMocks());

describe("SearchPage", () => {
  it("says how to search without a query", () => {
    const search = vi.spyOn(searchApi, "searchConversations");
    open("/search");
    expect(screen.getByRole("heading", { level: 1, name: "search" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "search conversations" })).toBeInTheDocument();
    expect(screen.getByText("press / to search titles and message text")).toBeInTheDocument();
    expect(search).not.toHaveBeenCalled();
    expect(shownHints()).toBe("n /");
  });

  it("lists the hits with the match marked, linked to their conversation", async () => {
    const search = vi.spyOn(searchApi, "searchConversations").mockResolvedValue([titleHit, messageHit]);
    open("/search?q=%20checkout%20");
    const list = await screen.findByRole("list", { name: "search results" });
    expect(search.mock.calls[0]?.[0]).toBe("checkout");
    expect(screen.getByText("checkout · 2 conversations")).toBeInTheDocument();
    const [first, second] = within(list).getAllByRole("link");
    expect(first).toHaveAttribute("href", "/c/c1");
    expect(first).toHaveTextContent("Checkout coupon rounding9mtitle");
    expect(marks(first as HTMLElement)).toEqual(["Checkout"]);
    expect(second).toHaveAttribute("href", "/c/c2");
    expect(second).toHaveTextContent("flaky ci on main3h…the checkout test fails on main…");
    expect(marks(second as HTMLElement)).toEqual(["checkout"]);
  });

  it("says when nothing matches", async () => {
    vi.spyOn(searchApi, "searchConversations").mockResolvedValue([]);
    open("/search?q=zebra");
    expect(await screen.findByRole("heading", { name: "no conversations match zebra" })).toBeInTheDocument();
    expect(screen.getByText("zebra · 0 conversations")).toBeInTheDocument();
  });

  it("shows a failed search as an error", async () => {
    vi.spyOn(searchApi, "searchConversations").mockRejectedValue(new ApiError("http", "search needs 1 to 200 characters", 400));
    open("/search?q=x");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: search needs 1 to 200 characters");
  });

  it("shows searching while the daemon thinks", async () => {
    vi.spyOn(searchApi, "searchConversations").mockReturnValue(new Promise(() => undefined));
    const { container } = open("/search?q=slow");
    await waitFor(() => expect(container.querySelector(".th-loading")).toHaveTextContent("searching…"));
    expect(screen.getByText("slow")).toBeInTheDocument();
  });

  it("counts one conversation without a plural", () => {
    expect(countLabel(1)).toBe("1 conversation");
  });
});
