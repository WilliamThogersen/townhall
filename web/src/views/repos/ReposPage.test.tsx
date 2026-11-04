import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import { ApiError } from "../../api/errors";
import type { Agent } from "../../api/types";
import type { RepoState } from "../../desktop/useRepo";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { BROWSER_NOTE, EXTERNAL_NOTE, ReposPage } from "./ReposPage";

function HintsProbe() {
  const hints = useCurrentKeyHints();
  return <p data-testid="hints">{hints.map((hint) => `${hint.keys}=${hint.label}`).join(" | ")}</p>;
}

const mocks = vi.hoisted(() => ({
  desktop: false,
  agents: [] as readonly Agent[],
  repo: null as RepoState | null,
}));

vi.mock("../../api/base", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../api/base")>()),
  isDesktop: () => mocks.desktop,
}));

vi.mock("../../agents/AgentsProvider", () => ({
  useAgentsContext: () => ({ agents: mocks.agents, loading: false, error: null, refresh: async () => undefined }),
}));

vi.mock("../../desktop/useRepo", () => ({
  useRepo: () => mocks.repo,
}));

const info = {
  home: "/Users/me/.townhall",
  repo: "/Users/me/Developer/wexo/work/clients/shop",
  repoKey: "shop-1",
};

const agent = (name: string, alive: boolean): Agent => ({ name, kind: "shell", branch: name, worktree: `/w/${name}`, alive });

function repoState(overrides: Partial<RepoState> = {}): RepoState {
  return {
    status: { repo: info.repo, mode: "embedded" },
    busy: false,
    error: null,
    choose: vi.fn(async () => undefined),
    ...overrides,
  };
}

beforeEach(() => {
  mocks.desktop = false;
  mocks.agents = [];
  mocks.repo = repoState();
});

afterEach(() => vi.restoreAllMocks());

describe("ReposPage", () => {
  it("shows the one repo from the daemon", async () => {
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    expect(screen.getByRole("heading", { level: 1, name: "repos" })).toBeInTheDocument();
    expect(await screen.findByText("shop")).toBeInTheDocument();
    const path = screen.getByText("~/Developer/…/clients/shop");
    expect(path).toHaveAttribute("title", info.repo);
    expect(screen.getByText("no conversations")).toBeInTheDocument();
    expect(screen.getByText("no agents")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "new conversation" })).toHaveAttribute("href", "/");
  });

  it("shows only the keys that work here", async () => {
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(
      <KeyHintsProvider>
        <ReposPage />
        <HintsProbe />
      </KeyHintsProvider>,
    );
    await screen.findByText("shop");
    expect(screen.getByTestId("hints")).toHaveTextContent("n=new conversation | /=search");
  });

  it("counts the agents with their glyphs", async () => {
    mocks.agents = [agent("a", true), agent("b", true), agent("c", false)];
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    expect(await screen.findByText(/3 agents/)).toBeInTheDocument();
    expect(screen.getByText("2 running, 1 ended")).toBeInTheDocument();
  });

  it("says one agent in the singular", async () => {
    mocks.agents = [agent("a", true)];
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    expect(await screen.findByText(/1 agent$/)).toBeInTheDocument();
  });

  it("shows loading until the info arrives", async () => {
    vi.spyOn(client, "getInfo").mockReturnValue(new Promise(() => undefined));
    render(<ReposPage />);
    expect(await screen.findByRole("status")).toHaveTextContent("loading repos…");
    expect(screen.queryByText("shop")).not.toBeInTheDocument();
  });

  it("shows an error with a retry", async () => {
    const spy = vi.spyOn(client, "getInfo").mockRejectedValue(new ApiError("network", "daemon not reachable"));
    render(<ReposPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("error: repo not loaded: daemon not reachable");
    spy.mockResolvedValue(info);
    await userEvent.click(screen.getByRole("button", { name: "retry" }));
    expect(await screen.findByText("shop")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("explains in the browser how to switch instead of a button", async () => {
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    await screen.findByText("shop");
    expect(screen.getByText(BROWSER_NOTE)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /change repository/ })).not.toBeInTheDocument();
  });

  it("changes the repository in the desktop app and reloads", async () => {
    mocks.desktop = true;
    const spy = vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    await screen.findByText("shop");
    spy.mockResolvedValue({ ...info, repo: "/Users/me/Developer/townhall" });
    await userEvent.click(screen.getByRole("button", { name: "change repository…" }));
    expect(mocks.repo?.choose).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("townhall")).toBeInTheDocument();
    expect(screen.queryByText(BROWSER_NOTE)).not.toBeInTheDocument();
  });

  it("shows the desktop button as busy while choosing", async () => {
    mocks.desktop = true;
    mocks.repo = repoState({ busy: true });
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    expect(await screen.findByRole("button", { name: /choosing…/ })).toBeDisabled();
  });

  it("shows a failed choice as an error", async () => {
    mocks.desktop = true;
    mocks.repo = repoState({ error: "not a git repository" });
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("error: not a git repository");
  });

  it("explains that an outside daemon cannot be switched from the app", async () => {
    mocks.desktop = true;
    mocks.repo = repoState({ status: { repo: null, mode: "external" } });
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    render(<ReposPage />);
    await screen.findByText("shop");
    expect(screen.getByText(EXTERNAL_NOTE)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /change repository/ })).not.toBeInTheDocument();
  });
});
