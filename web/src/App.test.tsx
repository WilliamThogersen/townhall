import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "./api/client";
import * as conversationsApi from "./api/conversations";
import * as events from "./api/events";
import { aConversation, aDetail, aRepo, aUsage, someStats } from "./api/fixtures";
import * as marketsApi from "./api/marketplaces";
import * as pluginsApi from "./api/plugins";
import * as reposApi from "./api/repos";
import * as searchApi from "./api/search";
import * as statsApi from "./api/stats";
import * as tasksApi from "./api/tasks";
import * as usageApi from "./api/usage";
import { App } from "./App";
import { navigate } from "./router";
import { DEFAULT_SETTINGS } from "./settings/settings";
import { createSettingsStore } from "./settings/settingsStore";
import { SettingsStoreContext } from "./settings/useSettings";

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h", repo: "/r", repoKey: "r-1" });
  vi.spyOn(events, "connectEvents").mockReturnValue({ close: () => undefined });
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo()]);
  vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
  vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]);
  vi.spyOn(searchApi, "searchConversations").mockResolvedValue([]);
  vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage());
  vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([]);
  vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ available: [], installed: [] });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

const heading = (name: string) => screen.getByRole("heading", { level: 1, name });

describe("App routes", () => {
  it("/ shows home", async () => {
    render(<App />);
    expect(await screen.findByRole("textbox", { name: "Describe the work" })).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it.each([
    ["/search?q=x", "no conversations match x"],
    ["/nope", "not found"],
  ])("%s shows %s", async (path, title) => {
    window.history.replaceState(null, "", path);
    render(<App />);
    expect(await screen.findByText(title)).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it.each([
    ["/repos", "repos"],
    ["/agents", "agents"],
    ["/plugins", "plugins"],
    ["/settings", "settings"],
  ])("%s shows the %s page", async (path, title) => {
    window.history.replaceState(null, "", path);
    render(<App />);
    expect(heading(title)).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it("/townhall shows the control center", async () => {
    vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]);
    vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]);
    window.history.replaceState(null, "", "/townhall");
    render(<App />);
    expect(heading("control center")).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "nothing ran today" })).toBeInTheDocument();
  });

  it("switches screens on navigate and keeps one shell", async () => {
    const chat = aDetail({ conversation: aConversation({ id: "abc", title: "abc chat" }) });
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(chat);
    const { container } = render(<App sidebarFoot={<span>foot</span>} />);
    act(() => navigate("/c/abc"));
    expect(await screen.findByRole("heading", { level: 2, name: "abc chat" })).toBeInTheDocument();
    expect(container.querySelectorAll(".th-shell")).toHaveLength(1);
    expect(screen.getByText("foot")).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalledOnce());
  });

  it("moves browser settings to the daemon at start, without opening /settings", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ ...DEFAULT_SETTINGS, agentsAtOnce: 9 }));
    const fetch = vi.fn(async (_url: string, init?: RequestInit) => {
      const agentsAtOnce = init?.method === "PUT" ? 9 : DEFAULT_SETTINGS.agentsAtOnce;
      return Response.json({ success: true, data: { ...DEFAULT_SETTINGS, agentsAtOnce }, error: null });
    });
    vi.stubGlobal("fetch", fetch);
    render(
      <SettingsStoreContext value={createSettingsStore()}>
        <App />
      </SettingsStoreContext>,
    );
    expect(await screen.findByRole("textbox", { name: "Describe the work" })).toBeInTheDocument();
    await waitFor(() => expect(localStorage.getItem("th-settings")).toBeNull());
    const calls = fetch.mock.calls.filter(([url]) => !String(url).endsWith("/logs/client"));
    expect(calls.map(([url, init]) => `${init?.method} ${url}`)).toEqual([
      "GET /api/settings",
      "PUT /api/settings/agentsAtOnce",
    ]);
    expect(JSON.parse(String(calls[1]?.[1]?.body))).toEqual({ value: 9 });
  });
});
