import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, anAgentInfo, aRepo, aTask } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import * as tasksApi from "../../api/tasks";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "../../data/testing";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { ReposPage } from "./ReposPage";

const townhall = aRepo({ name: "townhall", path: "/Users/me/Developer/townhall", baseBranch: "main" });
const shop = aRepo({ name: "shop", path: "/Users/me/Developer/wexo/work/clients/shop", baseBranch: "develop" });

function HintsProbe() {
  const hints = useCurrentKeyHints();
  return <p data-testid="hints">{hints.map((hint) => `${hint.keys}=${hint.label}`).join(" | ")}</p>;
}

function renderPage(events: FakeEvents = createFakeEvents()) {
  return render(
    <TestDataProvider events={events}>
      <KeyHintsProvider>
        <ReposPage />
        <HintsProbe />
      </KeyHintsProvider>
    </TestDataProvider>,
  );
}

const rowOf = (name: string) => screen.getByText(name, { selector: ".th-repo-name b" }).closest(".th-repo") as HTMLElement;

beforeEach(() => {
  window.history.replaceState(null, "", "/repos");
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([townhall, shop]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/Users/me/.townhall", repo: "/Users/me/.townhall", repoKey: "h-1" });
  vi.spyOn(conversationsApi, "listConversations").mockResolvedValue([
    aConversation({ id: "c1", linkedRepo: "shop" }),
    aConversation({ id: "c2" }),
  ]);
  vi.spyOn(tasksApi, "listTasks").mockResolvedValue([aTask({ name: "fix-login", conversationId: "c2", repo: "shop" })]);
  vi.spyOn(client, "listAgents").mockResolvedValue([
    anAgentInfo({ name: "fix-login", repo: "shop", state: { kind: "waiting", question: "ok?" } }),
    anAgentInfo({ name: "dev-server", repo: "shop", alive: true, state: { kind: "ended" } }),
  ]);
});

afterEach(() => vi.restoreAllMocks());

describe("ReposPage", () => {
  it("lists every repo with its short path, base branch, conversations and agents", async () => {
    renderPage();
    expect(screen.getByRole("heading", { level: 1, name: "repos" })).toBeInTheDocument();
    const row = await screen.findByText("shop", { selector: ".th-repo-name b" });
    expect(screen.getByText("2", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
    const shopRow = within(row.closest(".th-repo") as HTMLElement);
    expect(shopRow.getByText("~/Developer/…/clients/shop")).toHaveAttribute("title", shop.path);
    expect(shopRow.getByText("develop")).toBeInTheDocument();
    expect(await shopRow.findByText("2 conversations")).toBeInTheDocument();
    expect(await shopRow.findByText(/2 agents/)).toHaveTextContent("2 agents: 1 waiting, 1 running");
    expect(shopRow.getByRole("link", { name: "new conversation" })).toHaveAttribute("href", "/");
    const townhallRow = within(rowOf("townhall"));
    expect(townhallRow.getByText("~/Developer/townhall")).toBeInTheDocument();
    expect(townhallRow.getByText("no conversations")).toBeInTheDocument();
    expect(townhallRow.getByText("no agents")).toBeInTheDocument();
  });

  it("says it is loading until the repos arrive", async () => {
    vi.mocked(reposApi.listRepos).mockReturnValue(new Promise(() => undefined));
    renderPage();
    expect(await screen.findByRole("status")).toHaveTextContent("loading repos…");
  });

  it("shows a failed first load as an error", async () => {
    vi.mocked(reposApi.listRepos).mockRejectedValue(new ApiError("network", "daemon not reachable"));
    renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("error: repos not loaded: daemon not reachable");
  });

  it("says when there are no repos yet", async () => {
    vi.mocked(reposApi.listRepos).mockResolvedValue([]);
    renderPage();
    expect(await screen.findByRole("heading", { name: "no repos yet" })).toBeInTheDocument();
    expect(screen.getByText("0", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
  });

  it("adds a repository from the inline form and shows it at once", async () => {
    const added = aRepo({ name: "data-hub", path: "/Users/me/Developer/data-hub", baseBranch: "trunk" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    const open = screen.getByRole("button", { name: "add repository…" });
    expect(open).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(open);
    expect(open).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("textbox", { name: "path" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "add" })).toBeDisabled();
    expect(screen.getByTestId("hints")).toHaveTextContent("⏎=add | esc=cancel");
    await userEvent.type(screen.getByRole("textbox", { name: "path" }), "~/Developer/data-hub");
    await userEvent.type(screen.getByRole("textbox", { name: "base branch" }), "trunk{Enter}");
    expect(add).toHaveBeenCalledWith({ path: "~/Developer/data-hub", baseBranch: "trunk" });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "add repository" })).not.toBeInTheDocument();
    expect(screen.getByText("3", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
  });

  it("shows the daemon's refusal in the form and keeps what you typed", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(
      new ApiError("http", "/Users/me/Developer is not the top folder of a git repo", 400),
    );
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "path" }), "~/Developer{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Developer is not the top folder of a git repo");
    expect(screen.getByRole("textbox", { name: "path" })).toHaveValue("~/Developer");
  });

  it("opens the form with a and closes it with esc", async () => {
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    expect(screen.getByTestId("hints")).toHaveTextContent("a=add repository");
    await userEvent.keyboard("a");
    const path = screen.getByRole("textbox", { name: "path" });
    expect(path).toHaveFocus();
    expect(path).toHaveValue("");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("textbox", { name: "path" })).not.toBeInTheDocument();
  });

  it("asks before removing a repo and can be cancelled", async () => {
    const remove = vi.spyOn(reposApi, "removeRepo");
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    const removeButton = screen.getByRole("button", { name: "remove shop" });
    await userEvent.click(removeButton);
    expect(screen.getByText("remove shop? the folder and its worktrees stay on disk")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "cancel" })).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByText(/the folder and its worktrees stay on disk/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "remove shop" })).toHaveFocus();
    expect(remove).not.toHaveBeenCalled();
  });

  it("removes a repo once confirmed", async () => {
    const remove = vi.spyOn(reposApi, "removeRepo").mockResolvedValue(shop);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "remove shop" }));
    await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "remove" }));
    expect(remove).toHaveBeenCalledWith("shop");
    expect(screen.queryByText("shop", { selector: ".th-repo-name b" })).not.toBeInTheDocument();
    expect(screen.getByText("1", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
  });

  it("shows the daemon's conflict as is and lets you dismiss it", async () => {
    vi.spyOn(reposApi, "removeRepo").mockRejectedValue(
      new ApiError("http", "repo `shop` has 2 running agents: stop them first", 409),
    );
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "remove shop" }));
    await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: repo `shop` has 2 running agents: stop them first");
    expect(rowOf("shop")).not.toHaveAttribute("data-confirm");
    await userEvent.click(screen.getByRole("button", { name: "dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("follows live agent updates", async () => {
    const events = createFakeEvents();
    renderPage(events);
    const townhallRow = within(await screen.findByText("townhall", { selector: ".th-repo-name b" }).then((b) => b.closest(".th-repo") as HTMLElement));
    expect(await townhallRow.findByText("no agents")).toBeInTheDocument();
    act(() => events.emit({ type: "agentUpdated", agent: anAgentInfo({ name: "docs", repo: "townhall", state: { kind: "working" } }) }));
    expect(await townhallRow.findByText(/1 agent/)).toHaveTextContent("1 agent: 1 working");
  });
});
