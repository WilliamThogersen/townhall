import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
import { AgentsProvider } from "../agents/AgentsProvider";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import { ApiError } from "../api/errors";
import { aRepo, aUsage } from "../api/fixtures";
import * as reposApi from "../api/repos";
import * as tasksApi from "../api/tasks";
import type { AgentInfo } from "../api/types";
import * as usageApi from "../api/usage";
import { TestDataProvider } from "../data/testing";
import { navigate } from "../router";
import { useKeyHints } from "./keyHints";
import { daemonErrorText } from "./DaemonBanner";
import { Shell } from "./Shell";

const mk = (name: string, alive: boolean): AgentInfo => ({
  name, kind: "shell", branch: `townhall/${name}`, worktree: `/w/${name}`, alive,
  repo: "demo", state: { kind: alive ? "working" : "ended" },
});
const info = { home: "/h/.townhall", repo: "/r/demo", repoKey: "demo-0123abcd" };

let listAgents: MockInstance<typeof client.listAgents>;
let listRepos: MockInstance<typeof reposApi.listRepos>;

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  listAgents = vi.spyOn(client, "listAgents").mockResolvedValue([mk("a", true), mk("b", false)]);
  vi.spyOn(client, "getInfo").mockResolvedValue(info);
  listRepos = vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "shop" }), aRepo({ name: "site" })]);
  vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]);
  vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage({ session: null, weekly: null, error: "Sign in to Claude Code to see usage" }));
});

afterEach(() => vi.restoreAllMocks());

function Screen() {
  useKeyHints([{ keys: "r", label: "reply" }]);
  return <p>screen</p>;
}

function renderShell(foot?: ReactNode) {
  return render(
    <TestDataProvider>
      <AgentsProvider intervalMs={60_000}>
        <Shell sidebarFoot={foot}>
          <Screen />
        </Shell>
      </AgentsProvider>
    </TestDataProvider>,
  );
}

const sidebar = () => screen.getByRole("navigation", { name: "townhall" });
const link = (name: string | RegExp) => within(sidebar()).getByRole("link", { name });

describe("Shell", () => {
  it("lays out sidebar, main and key bar", async () => {
    const { container } = renderShell();
    const shell = container.querySelector(".th-shell");
    expect(shell?.children[0]).toHaveClass("th-sidebar");
    expect(shell?.children[1]).toHaveClass("th-main");
    expect(shell?.children[2]).toHaveClass("th-keybar");
    expect(within(screen.getByRole("main")).getByText("screen")).toBeInTheDocument();
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("makes the sidebar header a window drag region that keeps the logo link", async () => {
    const { container } = renderShell();
    const top = container.querySelector(".th-sidebar-top");
    expect(top).toHaveAttribute("data-tauri-drag-region", "deep");
    expect(top).toContainElement(link(/townhall/));
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("links every nav item to its route", async () => {
    renderShell();
    expect(link(/townhall/)).toHaveAttribute("href", "/townhall");
    expect(link(/new conversation/)).toHaveAttribute("href", "/");
    expect(link(/search/)).toHaveAttribute("href", "/search");
    expect(link(/repos/)).toHaveAttribute("href", "/repos");
    expect(link(/agents/)).toHaveAttribute("href", "/agents");
    expect(link(/plugins/)).toHaveAttribute("href", "/plugins");
    expect(link(/control center/)).toHaveAttribute("href", "/townhall");
    expect(link("settings")).toHaveAttribute("href", "/settings");
    expect(link(/new conversation/)).toHaveAttribute("aria-keyshortcuts", "n");
    expect(link(/search/)).toHaveAttribute("aria-keyshortcuts", "/");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("marks the current route", async () => {
    renderShell();
    expect(link(/new conversation/)).toHaveAttribute("aria-current", "page");
    expect(link(/control center/)).not.toHaveAttribute("aria-current");
    act(() => navigate("/townhall"));
    expect(link(/control center/)).toHaveAttribute("aria-current", "page");
    expect(link(/townhall/)).toHaveAttribute("aria-current", "page");
    expect(link(/new conversation/)).not.toHaveAttribute("aria-current");
    act(() => navigate("/settings"));
    expect(link("settings")).toHaveAttribute("aria-current", "page");
    fireEvent.click(link(/repos/));
    expect(window.location.pathname).toBe("/repos");
    expect(link(/repos/)).toHaveAttribute("aria-current", "page");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("shows the repo count and agents by state", async () => {
    renderShell();
    await waitFor(() => expect(link(/repos/)).toHaveTextContent("repos2"));
    expect(listRepos).toHaveBeenCalledOnce();
    expect(link(/control center/)).toHaveTextContent("1 running, 1 ended");
    const status = screen.getByRole("contentinfo").querySelector(".th-keybar-status");
    expect(status).toHaveTextContent("1 running");
    expect(status).toHaveTextContent("1 ended");
  });

  it("shows no repo count when the daemon cannot say", async () => {
    listRepos.mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    renderShell();
    await waitFor(() => expect(listRepos).toHaveBeenCalled());
    expect(link(/repos/)).toHaveTextContent(/^~repos$/);
  });

  it("shows the empty conversation sections", async () => {
    renderShell();
    expect(screen.getByRole("heading", { name: /^active 0/ })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "older 0" })).toBeInTheDocument();
    expect(await screen.findByText("no conversations yet")).toBeInTheDocument();
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("puts the extra foot control before settings", async () => {
    renderShell(<span>repo control</span>);
    const foot = screen.getByText("repo control").parentElement;
    expect(foot).toHaveClass("th-sidebar-foot");
    expect(foot?.lastElementChild).toHaveTextContent("settings");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("puts the usage meter at the start of the foot, next to settings", async () => {
    vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage());
    renderShell(<span>repo control</span>);
    const meter = await screen.findByRole("button", { name: "Claude plan usage" });
    const foot = meter.closest(".th-sidebar-foot");
    expect(foot?.firstElementChild).toContainElement(meter);
    expect(foot?.lastElementChild).toHaveTextContent("settings");
    expect(foot).toHaveTextContent("repo control");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("leaves the foot as it was when there is no usage to show", async () => {
    renderShell();
    await waitFor(() => expect(usageApi.getUsage).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Claude plan usage" })).not.toBeInTheDocument();
    expect(sidebar().querySelector(".th-sidebar-foot")?.children).toHaveLength(1);
  });

  it("shows the screen's key hints", async () => {
    renderShell();
    const keybar = screen.getByRole("contentinfo");
    expect(within(keybar).getByText("r")).toHaveClass("th-kbd");
    expect(keybar).toHaveTextContent("rreply");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("always offers the help key in the key bar", async () => {
    renderShell();
    expect(within(screen.getByRole("contentinfo")).getByText("?")).toHaveClass("th-kbd");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("opens the help overlay on ? and gives focus back on esc", async () => {
    renderShell();
    const reposLink = link(/repos/);
    reposLink.focus();
    fireEvent.keyDown(reposLink, { key: "?", shiftKey: true });
    const dialog = screen.getByRole("dialog", { name: "keyboard shortcuts" });
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(reposLink).toHaveFocus();
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("does not open the overlay from the search field", async () => {
    renderShell();
    fireEvent.keyDown(document.body, { key: "/" });
    const search = screen.getByRole("searchbox", { name: "search conversations" });
    fireEvent.keyDown(search, { key: "?", shiftKey: true });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });

  it("answers the global keys", async () => {
    window.history.replaceState(null, "", "/townhall");
    renderShell();
    fireEvent.keyDown(document.body, { key: "/" });
    expect(screen.getByRole("searchbox", { name: "search conversations" })).toHaveFocus();
    expect(window.location.pathname).toBe("/townhall");
    fireEvent.keyDown(document.body, { key: "n" });
    expect(link(/new conversation/)).toHaveAttribute("aria-current", "page");
    await waitFor(() => expect(listAgents).toHaveBeenCalled());
  });
});

describe("daemon banner", () => {
  it("shows the poll error and clears it on retry", async () => {
    listAgents.mockRejectedValue(new ApiError("network", "cannot reach the daemon: Failed to fetch"));
    renderShell();
    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("error: cannot reach the daemon: Failed to fetch");
    listAgents.mockResolvedValue([]);
    fireEvent.click(within(banner).getByRole("button", { name: "retry" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(screen.getByText("screen")).toBeInTheDocument();
  });

  it("names the daemon for every kind of failure", () => {
    expect(daemonErrorText("HTTP 502: response is not JSON")).toBe(
      "cannot reach the daemon: HTTP 502: response is not JSON",
    );
    expect(daemonErrorText("cannot reach the daemon: x")).toBe("cannot reach the daemon: x");
  });
});
