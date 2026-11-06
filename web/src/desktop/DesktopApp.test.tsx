import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import * as events from "../api/events";
import { aRepo, someStats } from "../api/fixtures";
import * as reposApi from "../api/repos";
import * as statsApi from "../api/stats";
import * as bridge from "./bridge";
import { DesktopApp } from "./DesktopApp";

vi.mock("./bridge", () => ({ pickFolder: vi.fn() }));

const pickFolder = vi.mocked(bridge.pickFolder);
const shop = aRepo({ name: "shop", path: "/Users/me/code/shop", baseBranch: "develop" });

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  vi.stubGlobal("__TAURI_INTERNALS__", {});
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/Users/me/.townhall", repo: "/Users/me/.townhall", repoKey: "h-1" });
  vi.spyOn(events, "connectEvents").mockReturnValue({ close: () => undefined });
  vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  pickFolder.mockReset();
});

describe("DesktopApp", () => {
  it("opens on home's first run when the daemon has no repo and adds one with the folder picker", async () => {
    vi.spyOn(reposApi, "listRepos").mockResolvedValue([]);
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(shop);
    pickFolder.mockResolvedValue(shop.path);
    render(<DesktopApp />);
    expect(await screen.findByRole("heading", { name: "add your first repository" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "townhall" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(pickFolder).toHaveBeenCalledOnce();
    expect(add).toHaveBeenCalledWith({ path: shop.path });
    expect(await screen.findByRole("textbox", { name: "Describe the work" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "add your first repository" })).not.toBeInTheDocument();
  });

  it("shows the shell and home's composer once a repo exists", async () => {
    vi.spyOn(reposApi, "listRepos").mockResolvedValue([shop]);
    render(<DesktopApp />);
    const nav = await screen.findByRole("navigation", { name: "townhall" });
    expect(within(nav).getByRole("link", { name: "settings" })).toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "Describe the work" })).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it("adds a repository from the repos page with the folder picker", async () => {
    window.history.replaceState(null, "", "/repos");
    vi.spyOn(reposApi, "listRepos").mockResolvedValue([shop]);
    const townhall = aRepo({ name: "townhall", path: "/Users/me/code/townhall" });
    vi.spyOn(reposApi, "addRepo").mockResolvedValue(townhall);
    pickFolder.mockResolvedValue(townhall.path);
    render(<DesktopApp />);
    expect(await screen.findByText("shop", { selector: ".th-repo-name b" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(await screen.findByText("townhall", { selector: ".th-repo-name b" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "folder or link" })).not.toBeInTheDocument();
  });
});
