import { act, render, screen, waitFor } from "@testing-library/react";
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
import { forgetHostSession } from "../plugins/hostSession";
import { ToastProvider } from "../ui/Toasts";
import { Shell } from "./Shell";

const footEntry: PluginUiEntry = {
  plugin: "panel@acme",
  slot: "sidebar.foot",
  title: "Build status",
  entry: "ui/foot.html",
  height: 40,
};

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  forgetHostSession("panel@acme");
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h/.townhall", repo: "/r/demo", repoKey: "demo-0123abcd" });
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "shop" })]);
  vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([]);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue([]);
  vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage({ session: null, weekly: null, error: "Sign in" }));
  vi.spyOn(pluginsApi, "mintHostToken").mockResolvedValue({
    plugin: "panel@acme",
    version: "1.0.0",
    token: "ef".repeat(32),
    scopes: [],
  });
});

afterEach(() => vi.restoreAllMocks());

function renderShell() {
  return render(
    <TestDataProvider>
      <AgentsProvider intervalMs={60_000}>
        <ToastProvider>
          <Shell>
            <p>screen</p>
          </Shell>
        </ToastProvider>
      </AgentsProvider>
    </TestDataProvider>,
  );
}

const sidebar = () => screen.getByRole("navigation", { name: "townhall" });
const foot = () => sidebar().querySelector(".th-sidebar-foot") as HTMLElement;

describe("Shell with a plugin sidebar.foot", () => {
  it("renders the frame in the sidebar foot above the settings button, at the manifest height", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([footEntry]);
    renderShell();
    await waitFor(() => expect(foot().querySelector("iframe")).not.toBeNull());
    const frame = foot().querySelector("iframe") as HTMLIFrameElement;
    const settings = screen.getByRole("link", { name: "settings" });
    expect(frame.compareDocumentPosition(settings) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(frame.style.height).toBe("40px");
  });

  it("answers hello with the sidebar.foot slot", async () => {
    vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([footEntry]);
    renderShell();
    await waitFor(() => expect(foot().querySelector("iframe")).not.toBeNull());
    const frame = foot().querySelector("iframe") as HTMLIFrameElement;
    const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", { data: { th: 1, type: "hello", apiVersion: 1 }, source: frame.contentWindow }),
      );
    });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0]?.[0]).toMatchObject({ type: "init", slot: "sidebar.foot", context: {} });
  });

  it("renders no frame when the plugin is disabled", async () => {
    const list = vi.spyOn(pluginsApi, "listPluginUi").mockResolvedValue([]);
    renderShell();
    await waitFor(() => expect(list).toHaveBeenCalled());
    expect(foot().querySelector("iframe")).toBeNull();
  });
});
