import { render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AgentsProvider } from "../agents/AgentsProvider";
import * as client from "../api/client";
import * as conversationsApi from "../api/conversations";
import { aRepo, aUsage } from "../api/fixtures";
import * as pluginsApi from "../api/plugins";
import * as reposApi from "../api/repos";
import * as tasksApi from "../api/tasks";
import type { PluginUiEntry } from "../api/types";
import * as usageApi from "../api/usage";
import { TestDataProvider } from "../data/testing";
import { Shell } from "./Shell";

const navEntry: PluginUiEntry = {
  plugin: "panel@acme",
  slot: "sidebar.nav",
  title: "Board",
  entry: "ui/board.html",
  height: null,
};

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h/.townhall", repo: "/r/demo", repoKey: "demo-0123abcd" });
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "shop" })]);
  vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]);
  vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage({ session: null, weekly: null, error: "Sign in" }));
});

afterEach(() => vi.restoreAllMocks());

function renderShell() {
  return render(
    <TestDataProvider>
      <AgentsProvider intervalMs={60_000}>
        <Shell>
          <p>screen</p>
        </Shell>
      </AgentsProvider>
    </TestDataProvider>,
  );
}

const sidebar = () => screen.getByRole("navigation", { name: "townhall" });

describe("Shell with plugin nav items", () => {
  it("lists an enabled plugin's sidebar.nav entry in the nav", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([navEntry]);
    renderShell();
    const item = await within(sidebar()).findByRole("link", { name: /Board/ });
    expect(item).toHaveAttribute("href", "/p/panel%40acme/ui%2Fboard.html");
    expect(item.closest(".th-nav")).not.toBeNull();
  });

  it("shows no plugin item when the plugin is disabled", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([]);
    renderShell();
    await vi.waitFor(() => expect(list).toHaveBeenCalled());
    expect(within(sidebar()).queryByRole("link", { name: /Board/ })).toBeNull();
  });
});
