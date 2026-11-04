import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, aDetail, aRepo, someStats } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import * as statsApi from "../../api/stats";
import type { Repo } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { HomePage } from "./HomePage";

const HERO_PLACEHOLDER = "what should we work on? name a repo, or describe the code";

const REPOS: readonly Repo[] = [aRepo({ name: "townhall" }), aRepo({ name: "shop", baseBranch: "develop" })];
const afternoon = () => new Date(2026, 10, 4, 14, 0);

function stubInfo(userName: string | null) {
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h", repo: "/r", repoKey: "r-1", userName });
}

function renderHome(repos: readonly Repo[] | Error = REPOS) {
  const list = vi.spyOn(reposApi, "listRepos");
  if (repos instanceof Error) list.mockRejectedValue(repos);
  else list.mockResolvedValue(repos);
  return renderWithHints(
    <TestDataProvider>
      <HomePage clock={afternoon} />
    </TestDataProvider>,
  );
}

const composer = () => screen.findByRole("textbox", { name: "Describe the work" });

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  stubInfo("William Thøgersen");
  vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
});

afterEach(() => vi.restoreAllMocks());

describe("HomePage", () => {
  it("greets by first name with the pixel logo", async () => {
    renderHome();
    const greeting = await screen.findByRole("heading", { level: 1, name: "good afternoon, William" });
    expect(greeting.querySelector(".th-logo")).not.toBeNull();
  });

  it("greets without a name when git has none", async () => {
    stubInfo(null);
    renderHome();
    expect(await screen.findByRole("heading", { level: 1, name: "good afternoon" })).toBeInTheDocument();
  });

  it("opens with the hero composer focused and the known repos linked", async () => {
    renderHome();
    const input = await composer();
    expect(input).toHaveAttribute("placeholder", HERO_PLACEHOLDER);
    await waitFor(() => expect(input).toHaveFocus());
    const known = screen.getByRole("link", { name: "2 repos known" });
    expect(known).toHaveAttribute("href", "/repos");
    expect(known).toHaveAttribute("title", "townhall, shop");
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
    expect(shownHints()).toBe("⏎ ⇧⏎ esc");
  });

  it("fills the composer from a starter without sending", async () => {
    const start = vi.spyOn(conversationsApi, "startConversation");
    renderHome();
    const input = await composer();
    input.blur();
    fireEvent.click(screen.getByRole("button", { name: "find dead code in shop" }));
    expect(input).toHaveValue("find dead code in shop");
    await waitFor(() => expect(input).toHaveFocus());
    expect(start).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "start" })).toBeEnabled();
  });

  it("starts a conversation on enter and opens it", async () => {
    const detail = aDetail({ conversation: aConversation({ id: "c42" }) });
    const start = vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(detail);
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "  fix the login loop  " } });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(start).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(window.location.pathname).toBe("/c/c42"));
    expect(start).toHaveBeenCalledWith({ text: "fix the login loop" });
  });

  it("keeps the text and shows the error when starting fails", async () => {
    vi.spyOn(conversationsApi, "startConversation").mockRejectedValue(
      new ApiError("network", "cannot reach the daemon"),
    );
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "fix the login loop" } });
    fireEvent.click(screen.getByRole("button", { name: "start" }));
    expect(await screen.findByText("error: cannot reach the daemon")).toBeInTheDocument();
    expect(input).toHaveValue("fix the login loop");
    expect(window.location.pathname).toBe("/");
  });

  it("does not start with only spaces", async () => {
    const start = vi.spyOn(conversationsApi, "startConversation");
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.submit(input.closest("form")!);
    expect(start).not.toHaveBeenCalled();
  });

  it("leaves the input on esc and comes back on n", async () => {
    renderHome();
    const input = await composer();
    await waitFor(() => expect(input).toHaveFocus());
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input).not.toHaveFocus();
    fireEvent.keyDown(document.body, { key: "n", metaKey: true });
    expect(input).not.toHaveFocus();
    fireEvent.keyDown(document.body, { key: "n" });
    expect(input).toHaveFocus();
  });

  it("shows the shipped stats", async () => {
    renderHome();
    expect(await screen.findByText("2 bug fixes shipped in the last 12 months")).toBeInTheDocument();
  });

  it("shows first run when there is no repo", async () => {
    renderHome([]);
    expect(await screen.findByRole("heading", { name: "add your first repository" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "add repository…" })).toHaveAttribute("href", "/repos");
    expect(screen.queryByRole("textbox")).toBeNull();
    await waitFor(() => expect(shownHints()).toBe("/"));
  });

  it("still offers the composer when the repos do not load", async () => {
    renderHome(new ApiError("network", "cannot reach the daemon"));
    await composer();
    expect(screen.queryByRole("link", { name: /known/ })).toBeNull();
    expect(screen.getByRole("button", { name: "find slow queries in search" })).toBeInTheDocument();
  });

  it("shows an empty home while the repos load", () => {
    vi.spyOn(reposApi, "listRepos").mockReturnValue(new Promise(() => undefined));
    const { container } = renderWithHints(
      <TestDataProvider>
        <HomePage />
      </TestDataProvider>,
    );
    act(() => undefined);
    expect(container.querySelector(".th-home")).toBeEmptyDOMElement();
  });
});
