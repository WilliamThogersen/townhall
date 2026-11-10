import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { aMarketplace, anAvailablePlugin, aPlugin, aPreview } from "../../api/fixtures";
import * as marketsApi from "../../api/marketplaces";
import * as pluginsApi from "../../api/plugins";
import type { Marketplace, PluginList } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { PluginsPage } from "./PluginsPage";

const official = aMarketplace({
  name: "townhall-official",
  official: true,
  source: { source: "github", repo: "WilliamThogersen/townhall-plugins" },
  plugins: [{ name: "tester", description: "a tester type", version: "1.0.0", category: null }],
});
const custom = aMarketplace({
  plugins: [{ name: "lint", description: "lints the brief", version: "1.0.0", category: null }],
});

interface Daemon {
  markets: Marketplace[];
  list: PluginList;
}

function stub({ markets = [official, custom], list }: { markets?: Marketplace[]; list?: Partial<PluginList> } = {}) {
  const daemon: Daemon = { markets, list: { available: [], installed: [], ...list } };
  vi.spyOn(marketsApi, "listMarketplaces").mockImplementation(async () => daemon.markets);
  vi.spyOn(pluginsApi, "listPlugins").mockImplementation(async () => daemon.list);
  return daemon;
}

function renderPage() {
  return render(
    <TestDataProvider>
      <PluginsPage />
    </TestDataProvider>,
  );
}

const section = (name: string) => screen.findByRole("region", { name });
const article = (within_: HTMLElement, id: string) => within(within_).getByRole("article", { name: id });

afterEach(() => vi.restoreAllMocks());

describe("PluginsPage marketplaces", () => {
  beforeEach(() => {
    stub();
  });

  it("lists marketplaces, giving the official one a badge and no remove", async () => {
    renderPage();
    const markets = await section("marketplaces");
    const builtIn = article(markets, "townhall-official");
    expect(within(builtIn).getByText("official")).toBeInTheDocument();
    expect(within(builtIn).queryByRole("button", { name: /remove/ })).toBeNull();
    expect(within(builtIn).getByRole("button", { name: "refresh townhall-official" })).toBeInTheDocument();
    const own = article(markets, "acme-tools");
    expect(within(own).queryByText("official")).toBeNull();
    expect(within(own).getByText("https://github.com/acme/tools.git")).toBeInTheDocument();
    expect(within(own).getByText("1 plugin")).toBeInTheDocument();
    expect(within(own).getByRole("button", { name: "remove acme-tools" })).toBeInTheDocument();
  });

  it("adds a marketplace by whatever the user types and clears the field", async () => {
    const add = vi.spyOn(marketsApi, "addMarketplace").mockResolvedValue(aMarketplace({ name: "x" }));
    renderPage();
    const markets = await section("marketplaces");
    const field = within(markets).getByRole("textbox", { name: "marketplace link, owner/repo or folder" });
    await userEvent.type(field, "acme/tools");
    await userEvent.click(within(markets).getByRole("button", { name: "add marketplace" }));
    expect(add).toHaveBeenCalledWith({ source: "acme/tools" });
    expect(field).toHaveValue("");
  });

  it("does not add an empty source", async () => {
    renderPage();
    const markets = await section("marketplaces");
    expect(within(markets).getByRole("button", { name: "add marketplace" })).toBeDisabled();
  });

  it("shows the daemon's reason when an add is refused", async () => {
    vi.spyOn(marketsApi, "addMarketplace").mockRejectedValue(new ApiError("http", "could not clone", 400));
    renderPage();
    const markets = await section("marketplaces");
    await userEvent.type(within(markets).getByRole("textbox"), "nope/nope");
    await userEvent.click(within(markets).getByRole("button", { name: "add marketplace" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("could not clone");
  });

  it("refreshes and removes", async () => {
    const refresh = vi.spyOn(marketsApi, "refreshMarketplace").mockResolvedValue(custom);
    const remove = vi.spyOn(marketsApi, "removeMarketplace").mockResolvedValue(custom);
    renderPage();
    const markets = await section("marketplaces");
    await userEvent.click(within(markets).getByRole("button", { name: "refresh acme-tools" }));
    expect(refresh).toHaveBeenCalledWith("acme-tools");
    await userEvent.click(within(markets).getByRole("button", { name: "remove acme-tools" }));
    expect(remove).toHaveBeenCalledWith("acme-tools");
  });

  it("says why a marketplace cannot be removed", async () => {
    vi.spyOn(marketsApi, "removeMarketplace").mockRejectedValue(new ApiError("http", "it still has installed plugins", 409));
    renderPage();
    const markets = await section("marketplaces");
    await userEvent.click(within(markets).getByRole("button", { name: "remove acme-tools" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("it still has installed plugins");
  });

  it("shows a marketplace's own error", async () => {
    stub({ markets: [aMarketplace({ error: "could not fetch" })] });
    renderPage();
    expect(await within(await section("marketplaces")).findByText("could not fetch")).toBeInTheDocument();
  });
});

describe("PluginsPage available plugins", () => {
  const available = [
    anAvailablePlugin(),
    anAvailablePlugin({ id: "tester@townhall-official", marketplace: "townhall-official", name: "tester", description: "a tester type" }),
    anAvailablePlugin({ id: "old@acme-tools", name: "old", description: "already here", installed: true }),
  ];

  it("lists what each marketplace offers, with its state", async () => {
    stub({ list: { available, installed: [aPlugin({ id: "old@acme-tools", name: "old" })] } });
    renderPage();
    const list = await section("available plugins");
    expect(within(list).getByRole("heading", { name: "acme-tools" })).toBeInTheDocument();
    expect(within(list).getByRole("heading", { name: "townhall-official" })).toBeInTheDocument();
    const lint = article(list, "lint@acme-tools");
    expect(within(lint).getByText("lints the brief")).toBeInTheDocument();
    expect(within(lint).getByRole("button", { name: "install lint" })).toBeInTheDocument();
    const old = article(list, "old@acme-tools");
    expect(within(old).getByText("installed")).toBeInTheDocument();
    expect(within(old).queryByRole("button", { name: /install/ })).toBeNull();
  });

  it("installs through the permission prompt with the hash it showed", async () => {
    const daemon = stub({ list: { available } });
    const preview = vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(
      aPreview({ permissions: ["api:tasks:read"], permissionsHash: "sha256:shown" }),
    );
    const install = vi.spyOn(pluginsApi, "installPlugin").mockImplementation(async () => {
      daemon.list = { available, installed: [aPlugin()] };
      return aPlugin();
    });
    const patch = vi.spyOn(pluginsApi, "patchPlugin");
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    expect(preview).toHaveBeenCalledWith("lint@acme-tools");
    const dialog = await screen.findByRole("dialog", { name: "install lint" });
    expect(await within(dialog).findByText("tasks:read")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "install" }));
    expect(install).toHaveBeenCalledWith("lint@acme-tools", "sha256:shown");
    expect(patch).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("installs and enables", async () => {
    stub({ list: { available } });
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview({ permissionsHash: "sha256:shown" }));
    vi.spyOn(pluginsApi, "installPlugin").mockResolvedValue(aPlugin());
    const patch = vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin({ enabled: true }));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    const dialog = await screen.findByRole("dialog");
    await userEvent.click(await within(dialog).findByRole("button", { name: "install and enable" }));
    expect(patch).toHaveBeenCalledWith("lint@acme-tools", { enabled: true });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps the prompt open with the reason when the install is refused", async () => {
    stub({ list: { available } });
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview());
    vi.spyOn(pluginsApi, "installPlugin").mockRejectedValue(new ApiError("http", "approve is not the hash", 400));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    const dialog = await screen.findByRole("dialog");
    await userEvent.click(await within(dialog).findByRole("button", { name: "install" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("approve is not the hash");
  });

  it("shows the preview error and installs nothing when the preview fails", async () => {
    stub({ list: { available } });
    vi.spyOn(pluginsApi, "previewPlugin").mockRejectedValue(new ApiError("http", "needs plugin api 2, this townhall serves 1", 400));
    const install = vi.spyOn(pluginsApi, "installPlugin");
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("needs plugin api 2");
    expect(within(screen.getByRole("dialog")).getByRole("button", { name: "install" })).toBeDisabled();
    expect(install).not.toHaveBeenCalled();
  });

  it("closes the prompt without installing", async () => {
    stub({ list: { available } });
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview());
    const install = vi.spyOn(pluginsApi, "installPlugin");
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install lint" }));
    await userEvent.click(await screen.findByRole("button", { name: "cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(install).not.toHaveBeenCalled();
  });

  it("marks an official plugin's prompt", async () => {
    stub({ list: { available } });
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview({ plugin: "tester@townhall-official", name: "tester" }));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "install tester" }));
    const dialog = await screen.findByRole("dialog", { name: "install tester" });
    expect(within(dialog).getByText("official")).toBeInTheDocument();
  });
});

describe("PluginsPage installed plugins", () => {
  const installedBox = () => section("installed plugins");

  it("says so when nothing is installed", async () => {
    stub();
    renderPage();
    expect(await within(await installedBox()).findByText("no plugins installed")).toBeInTheDocument();
  });

  it("shows an installed, not enabled plugin and enables it", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const patch = vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin({ enabled: true }));
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    expect(within(row).getByText("installed")).toBeInTheDocument();
    expect(within(row).queryByText("enabled")).toBeNull();
    await userEvent.click(within(row).getByRole("button", { name: "enable lint" }));
    expect(patch).toHaveBeenCalledWith("lint@acme-tools", { enabled: true });
  });

  it("shows an enabled plugin and disables it", async () => {
    stub({ list: { installed: [aPlugin({ enabled: true })] } });
    const patch = vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin());
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    expect(within(row).getByText("enabled")).toBeInTheDocument();
    await userEvent.click(within(row).getByRole("button", { name: "disable lint" }));
    expect(patch).toHaveBeenCalledWith("lint@acme-tools", { enabled: false });
  });

  it("shows a pinned plugin and unpins it", async () => {
    stub({ list: { installed: [aPlugin({ pinned: true })] } });
    const patch = vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin());
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    expect(within(row).getByText("pinned")).toBeInTheDocument();
    await userEvent.click(within(row).getByRole("button", { name: "unpin lint" }));
    expect(patch).toHaveBeenCalledWith("lint@acme-tools", { pinned: false });
  });

  it("pins a plugin that is not pinned", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const patch = vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin({ pinned: true }));
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "pin lint" }));
    expect(patch).toHaveBeenCalledWith("lint@acme-tools", { pinned: true });
  });

  it("shows what the user approved for an installed plugin", async () => {
    stub({ list: { installed: [aPlugin({ permissions: ["api:tasks:read"] })] } });
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByText("permissions"));
    expect(within(row).getByText("tasks:read")).toBeInTheDocument();
  });

  it("reports an update that changed nothing the user must approve", async () => {
    stub({ list: { installed: [aPlugin()] } });
    vi.spyOn(pluginsApi, "updatePlugin").mockResolvedValue({
      outcome: "updated",
      sha: "2222222",
      added: [],
      permissionsHash: null,
      plugin: aPlugin({ sha: "2222222" }),
    });
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "update lint" }));
    expect(await within(row).findByText("updated to 2222222")).toBeInTheDocument();
  });

  it("says when there is nothing newer, and when the plugin is pinned", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const update = vi.spyOn(pluginsApi, "updatePlugin").mockResolvedValue({
      outcome: "unchanged",
      sha: null,
      added: [],
      permissionsHash: null,
      plugin: aPlugin(),
    });
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "update lint" }));
    expect(await within(row).findByText("up to date")).toBeInTheDocument();
    update.mockResolvedValue({ outcome: "pinned", sha: null, added: [], permissionsHash: null, plugin: aPlugin({ pinned: true }) });
    await userEvent.click(within(row).getByRole("button", { name: "update lint" }));
    expect(await within(row).findByText("pinned, not updated")).toBeInTheDocument();
  });

  const waiting = aPlugin({ enabled: true, error: "update to 2222222 needs approval" });

  it("shows needs approval and the added permissions, and approves with the new hash", async () => {
    stub({ list: { installed: [waiting] } });
    const update = vi.spyOn(pluginsApi, "updatePlugin").mockResolvedValueOnce({
      outcome: "needsApproval",
      sha: "2222222",
      added: ["api:tasks:create", "env:GITHUB_TOKEN"],
      permissionsHash: "sha256:new",
      plugin: waiting,
    });
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    expect(within(row).getByText("needs approval")).toBeInTheDocument();
    expect(within(row).getByText("update to 2222222 needs approval")).toBeInTheDocument();
    await userEvent.click(within(row).getByRole("button", { name: "review update lint" }));
    expect(update).toHaveBeenLastCalledWith("lint@acme-tools");
    expect(await within(row).findByText("tasks:create")).toBeInTheDocument();
    expect(within(row).getByText("GITHUB_TOKEN")).toBeInTheDocument();
    update.mockResolvedValueOnce({
      outcome: "updated",
      sha: "2222222",
      added: [],
      permissionsHash: null,
      plugin: aPlugin({ sha: "2222222" }),
    });
    await userEvent.click(within(row).getByRole("button", { name: "approve update lint" }));
    expect(update).toHaveBeenLastCalledWith("lint@acme-tools", "sha256:new");
  });

  it("does not approve until the added permissions have been shown", async () => {
    stub({ list: { installed: [waiting] } });
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    expect(within(row).queryByRole("button", { name: "approve update lint" })).toBeNull();
  });

  it("uninstalls, keeping the plugin's data unless asked to purge", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const remove = vi.spyOn(pluginsApi, "deletePlugin").mockResolvedValue(aPlugin());
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "uninstall lint" }));
    expect(remove).not.toHaveBeenCalled();
    await userEvent.click(within(row).getByRole("button", { name: "confirm uninstall" }));
    expect(remove).toHaveBeenCalledWith("lint@acme-tools", false);
  });

  it("uninstalls and purges when the box is ticked", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const remove = vi.spyOn(pluginsApi, "deletePlugin").mockResolvedValue(aPlugin());
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "uninstall lint" }));
    await userEvent.click(within(row).getByRole("checkbox", { name: "also delete its data" }));
    await userEvent.click(within(row).getByRole("button", { name: "confirm uninstall" }));
    expect(remove).toHaveBeenCalledWith("lint@acme-tools", true);
  });

  it("backs out of an uninstall", async () => {
    stub({ list: { installed: [aPlugin()] } });
    const remove = vi.spyOn(pluginsApi, "deletePlugin");
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "uninstall lint" }));
    await userEvent.click(within(row).getByRole("button", { name: "keep it" }));
    expect(within(row).getByRole("button", { name: "uninstall lint" })).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
  });

  it("shows the reason when an action is refused", async () => {
    stub({ list: { installed: [aPlugin()] } });
    vi.spyOn(pluginsApi, "patchPlugin").mockRejectedValue(new ApiError("http", "no such plugin", 404));
    renderPage();
    const row = article(await installedBox(), "lint@acme-tools");
    await userEvent.click(within(row).getByRole("button", { name: "enable lint" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no such plugin");
  });
});

describe("PluginsPage loading", () => {
  it("shows the load error when the daemon cannot be asked", async () => {
    vi.spyOn(marketsApi, "listMarketplaces").mockRejectedValue(new ApiError("network", "cannot reach the daemon"));
    vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ available: [], installed: [] });
    renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("plugins not loaded: cannot reach the daemon");
  });

  it("has one page heading", async () => {
    stub();
    renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "plugins" })).toBeInTheDocument();
  });
});
