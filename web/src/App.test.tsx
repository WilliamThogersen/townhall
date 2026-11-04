import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "./api/client";
import { App } from "./App";
import { navigate } from "./router";
import { DEFAULT_SETTINGS } from "./settings/settings";
import { createSettingsStore } from "./settings/settingsStore";
import { SettingsStoreContext } from "./settings/useSettings";

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h", repo: "/r", repoKey: "r-1" });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

const heading = (name: string) => screen.getByRole("heading", { level: 1, name });

describe("App routes", () => {
  it.each([
    ["/", "conversations come next"],
    ["/c/fix-login", "conversations come next"],
    ["/search?q=x", "search comes next"],
    ["/nope", "not found"],
  ])("%s shows %s", async (path, title) => {
    window.history.replaceState(null, "", path);
    render(<App />);
    expect(screen.getByText(title)).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it.each([
    ["/repos", "repos"],
    ["/settings", "settings"],
  ])("%s shows the %s page", async (path, title) => {
    window.history.replaceState(null, "", path);
    render(<App />);
    expect(heading(title)).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it("/townhall shows the control center", async () => {
    window.history.replaceState(null, "", "/townhall");
    render(<App />);
    expect(heading("control center")).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "no agents yet" })).toBeInTheDocument();
  });

  it("switches screens on navigate and keeps one shell", async () => {
    const { container } = render(<App sidebarFoot={<span>foot</span>} />);
    act(() => navigate("/c/abc"));
    expect(screen.getByText("conversation abc will open here")).toBeInTheDocument();
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
    expect(screen.getByText("conversations come next")).toBeInTheDocument();
    await waitFor(() => expect(localStorage.getItem("th-settings")).toBeNull());
    expect(fetch.mock.calls.map(([url, init]) => `${init?.method} ${url}`)).toEqual([
      "GET /api/settings",
      "PUT /api/settings/agentsAtOnce",
    ]);
    expect(JSON.parse(String(fetch.mock.calls[1]?.[1]?.body))).toEqual({ value: 9 });
  });
});
