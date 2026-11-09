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
import { DEFAULT_SETTINGS, type ForgeAccount, type Settings } from "../../settings/settings";
import type { PutSetting } from "../../settings/browserImport";
import { createSettingsStore } from "../../settings/settingsStore";
import { SettingsStoreContext } from "../../settings/useSettings";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { ReposPage } from "./ReposPage";

const townhall = aRepo({ name: "townhall", path: "/Users/me/Developer/townhall", baseBranch: "main" });
const shop = aRepo({ name: "shop", path: "/Users/me/Developer/wexo/work/clients/shop", baseBranch: "develop" });

function HintsProbe() {
  const hints = useCurrentKeyHints();
  return <p data-testid="hints">{hints.map((hint) => `${hint.keys}=${hint.label}`).join(" | ")}</p>;
}

function renderPage(
  events: FakeEvents = createFakeEvents(),
  forgeAccounts: readonly ForgeAccount[] = [],
  over: Partial<Settings> = {},
  put: PutSetting = async () => DEFAULT_SETTINGS,
) {
  const settings = createSettingsStore({
    load: async () => ({ ...DEFAULT_SETTINGS, forgeAccounts, ...over }),
    put,
    storage: () => null,
  });
  return render(
    <SettingsStoreContext value={settings}>
      <TestDataProvider events={events}>
        <KeyHintsProvider>
          <ReposPage />
          <HintsProbe />
        </KeyHintsProvider>
      </TestDataProvider>
    </SettingsStoreContext>,
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
    expect(screen.getByRole("textbox", { name: "folder or link" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "add" })).toBeDisabled();
    expect(screen.getByTestId("hints")).toHaveTextContent("⏎=add | esc=cancel");
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "~/Developer/data-hub");
    await userEvent.type(screen.getByRole("textbox", { name: "base branch" }), "trunk{Enter}");
    expect(add).toHaveBeenCalledWith({ path: "~/Developer/data-hub", baseBranch: "trunk" });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "add repository" })).not.toBeInTheDocument();
    expect(screen.getByText("3", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
  });

  it("takes a github or gitlab link in the path field and sends it as the path", async () => {
    const link = "git@github.com:acme/data-hub.git";
    const added = aRepo({ name: "data-hub", path: "/Users/me/.townhall/repos/data-hub", baseBranch: "main" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    expect(screen.getByPlaceholderText(/or a github\/gitlab link/)).toBe(screen.getByRole("textbox", { name: "folder or link" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), `${link}{Enter}`);
    expect(add).toHaveBeenCalledWith({ path: link });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
  });

  it("asks where to clone only once a link is typed, and says where it will land", async () => {
    const link = "https://github.com/acme/data-hub.git";
    const added = aRepo({ name: "data-hub", path: "/Users/me/Developer/data-hub", baseBranch: "main" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    const path = screen.getByRole("textbox", { name: "folder or link" });
    await userEvent.type(path, "~/Developer/data-hub");
    expect(screen.queryByRole("textbox", { name: "clone into" })).not.toBeInTheDocument();
    await userEvent.clear(path);
    await userEvent.type(path, link);
    const into = screen.getByRole("textbox", { name: "clone into" });
    expect(into).toHaveAttribute("placeholder", "~/.townhall/repos");
    expect(screen.getByText("clones into ~/.townhall/repos/data-hub")).toBeInTheDocument();
    await userEvent.type(into, "~/Developer");
    expect(screen.getByText("clones into ~/Developer/data-hub")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: link, cloneDir: "~/Developer" });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
  });

  it("offers the clones folder from the settings as the default", async () => {
    renderPage(createFakeEvents(), [], { clonesDir: "~/Developer" });
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "git@gitlab.com:group/sub/app.git");
    expect(await screen.findByText("clones into ~/Developer/app")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "clone into" })).toHaveAttribute("placeholder", "~/Developer");
  });

  it("edits the default clone folder from the repos page and saves it as the clones setting", async () => {
    const put = vi.fn<PutSetting>(async (key, value) => ({ ...DEFAULT_SETTINGS, [key]: value }));
    renderPage(createFakeEvents(), [], {}, put);
    const input = await screen.findByRole("textbox", { name: "default clone folder" });
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("placeholder", "~/.townhall/repos");
    await userEvent.type(input, "projects");
    await userEvent.tab();
    expect(screen.getByRole("alert")).toHaveTextContent("use ~, ~/folder or an absolute path");
    expect(put).not.toHaveBeenCalled();
    await userEvent.clear(input);
    await userEvent.type(input, "~/Developer{Enter}");
    expect(put).toHaveBeenLastCalledWith("clonesDir", "~/Developer");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await userEvent.clear(input);
    await userEvent.tab();
    expect(put).toHaveBeenLastCalledWith("clonesDir", "");
  });

  it("shows the saved default clone folder", async () => {
    renderPage(createFakeEvents(), [], { clonesDir: "~/Developer" });
    expect(await screen.findByRole("textbox", { name: "default clone folder" })).toHaveValue("~/Developer");
  });

  it("refuses a link that names no repo before asking the daemon", async () => {
    const add = vi.spyOn(reposApi, "addRepo");
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "https://github.com/acme{Enter}");
    expect(screen.getByRole("alert")).toHaveTextContent("expected host/owner/repo");
    expect(screen.getByRole("button", { name: "add" })).toBeDisabled();
    expect(add).not.toHaveBeenCalled();
  });

  it("shows the clone target while cloning and the daemon's reason when it fails", async () => {
    const link = "https://github.com/acme/data-hub";
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(new ApiError("http", "/Users/me/Developer/data-hub already exists and is not a clone of x", 409));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), link);
    await userEvent.type(screen.getByRole("textbox", { name: "clone into" }), "~/Developer");
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Developer/data-hub already exists");
    expect(screen.getByRole("form", { name: "add repository" })).toBeInTheDocument();
  });

  describe("editing a repo", () => {
    const form = () => screen.getByRole("form", { name: "edit shop" });

    it("renames it, changes its base branch and shows the saved repo at once", async () => {
      const saved = aRepo({ ...shop, name: "store", baseBranch: "main" });
      const patch = vi.spyOn(reposApi, "patchRepo").mockResolvedValue(saved);
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "edit shop" }));
      const f = within(form());
      expect(f.getByRole("textbox", { name: "name" })).toHaveValue("shop");
      expect(f.getByRole("textbox", { name: "base branch" })).toHaveValue("develop");
      expect(f.getByRole("textbox", { name: "folder" })).toHaveValue(shop.path);
      expect(f.getByRole("button", { name: "save" })).toBeDisabled();
      await userEvent.clear(f.getByRole("textbox", { name: "name" }));
      await userEvent.type(f.getByRole("textbox", { name: "name" }), "store");
      await userEvent.clear(f.getByRole("textbox", { name: "base branch" }));
      await userEvent.type(f.getByRole("textbox", { name: "base branch" }), "main{Enter}");
      expect(patch).toHaveBeenCalledWith("shop", { name: "store", baseBranch: "main" });
      expect(await screen.findByText("store", { selector: ".th-repo-name b" })).toBeInTheDocument();
      expect(screen.queryByText("shop", { selector: ".th-repo-name b" })).not.toBeInTheDocument();
      expect(screen.queryByRole("form", { name: "edit shop" })).not.toBeInTheDocument();
    });

    it("points the repo at a new folder, or moves the folder when asked", async () => {
      const patch = vi.spyOn(reposApi, "patchRepo").mockResolvedValue(aRepo({ ...shop, path: "/code/shop" }));
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "edit shop" }));
      const f = within(form());
      expect(f.queryByRole("checkbox")).not.toBeInTheDocument();
      await userEvent.clear(f.getByRole("textbox", { name: "folder" }));
      await userEvent.type(f.getByRole("textbox", { name: "folder" }), "/code/shop");
      await userEvent.click(f.getByRole("checkbox", { name: /move the folder/ }));
      await userEvent.click(f.getByRole("button", { name: "save" }));
      expect(patch).toHaveBeenCalledWith("shop", { path: "/code/shop", moveFolder: true });
    });

    it("keeps the form open with the daemon's reason when it refuses", async () => {
      vi.spyOn(reposApi, "patchRepo").mockRejectedValue(new ApiError("http", "a repo named `townhall` already exists", 409));
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "edit shop" }));
      await userEvent.clear(within(form()).getByRole("textbox", { name: "name" }));
      await userEvent.type(within(form()).getByRole("textbox", { name: "name" }), "townhall{Enter}");
      expect(await within(form()).findByRole("alert")).toHaveTextContent("error: a repo named `townhall` already exists");
      expect(within(form()).getByRole("textbox", { name: "name" })).toHaveValue("townhall");
    });

    it("does not save a name the daemon would refuse, and cancels with escape", async () => {
      const patch = vi.spyOn(reposApi, "patchRepo");
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "edit shop" }));
      await userEvent.type(within(form()).getByRole("textbox", { name: "name" }), " x");
      expect(within(form()).getByRole("button", { name: "save" })).toBeDisabled();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("form", { name: "edit shop" })).not.toBeInTheDocument();
      expect(patch).not.toHaveBeenCalled();
      expect(screen.getByText("shop", { selector: ".th-repo-name b" })).toBeInTheDocument();
    });
  });

  it("says cloning while a link is added and initialising while a folder is", async () => {
    vi.spyOn(reposApi, "addRepo").mockReturnValue(new Promise(() => undefined));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "https://github.com/acme/data-hub{Enter}");
    expect(await screen.findByRole("status")).toHaveTextContent("cloning…");
  });

  it("says initialising while a folder is added", async () => {
    vi.spyOn(reposApi, "addRepo").mockReturnValue(new Promise(() => undefined));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "~/Developer/notes{Enter}");
    expect(await screen.findByRole("status")).toHaveTextContent("initialising…");
  });

  it("shows the daemon's refusal in the form and keeps what you typed", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(
      new ApiError("http", "/Users/me/Developer is not the top folder of a git repo", 400),
    );
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "folder or link" }), "~/Developer{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Developer is not the top folder of a git repo");
    expect(screen.getByRole("textbox", { name: "folder or link" })).toHaveValue("~/Developer");
  });

  it("opens the form with a and closes it with esc", async () => {
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    expect(screen.getByTestId("hints")).toHaveTextContent("a=add repository");
    await userEvent.keyboard("a");
    const path = screen.getByRole("textbox", { name: "folder or link" });
    expect(path).toHaveFocus();
    expect(path).toHaveValue("");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("textbox", { name: "folder or link" })).not.toBeInTheDocument();
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

  it("offers to delete the clone only for repos cloned into the managed folder", async () => {
    const clone = aRepo({ name: "hub", path: "/Users/me/.townhall/repos/hub", baseBranch: "main" });
    vi.mocked(reposApi.listRepos).mockResolvedValue([shop, clone]);
    const remove = vi.spyOn(reposApi, "removeRepo").mockResolvedValue(clone);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "remove shop" }));
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    await userEvent.click(within(rowOf("shop")).getByRole("button", { name: "cancel" }));
    await userEvent.click(screen.getByRole("button", { name: "remove hub" }));
    await userEvent.click(within(rowOf("hub")).getByRole("checkbox", { name: "also delete the clone" }));
    await userEvent.click(within(rowOf("hub")).getByRole("button", { name: "remove" }));
    expect(remove).toHaveBeenCalledWith("hub", true);
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

describe("repo accounts", () => {
  const work: ForgeAccount = { forge: "github", login: "work-me", host: "github.com", label: "work" };
  const mine: ForgeAccount = { forge: "github", login: "private-me", host: "github.com", label: "private" };
  const glab: ForgeAccount = { forge: "gitlab", login: "me", host: "gitlab.corp.io", label: "corp gitlab" };
  const pick = (name: string, forge: "GitHub" | "GitLab") =>
    within(within(rowOf(name)).getByRole("radiogroup", { name: `${forge} account for ${name}` }));

  it("shows no account choice while no account is registered", async () => {
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    expect(screen.queryByRole("radiogroup", { name: /account for/ })).not.toBeInTheDocument();
  });

  it("offers auto and each registered account, only for the forges that have one", async () => {
    renderPage(createFakeEvents(), [work, mine]);
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    await within(rowOf("shop")).findByRole("radiogroup", { name: "GitHub account for shop" });
    const labels = pick("shop", "GitHub").getAllByRole("radio").map((radio) => radio.closest("label")?.textContent);
    expect(labels).toEqual(["auto", "work", "private"]);
    expect(pick("shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
    expect(within(rowOf("shop")).queryByRole("radiogroup", { name: /GitLab/ })).not.toBeInTheDocument();
    expect(within(rowOf("townhall")).getByRole("radiogroup", { name: "GitHub account for townhall" })).toBeInTheDocument();
  });

  it("shows the account a repo already uses and the gitlab choice when one is registered", async () => {
    vi.mocked(reposApi.listRepos).mockResolvedValue([aRepo({ ...shop, githubAccount: "private-me", gitlabAccount: "me" })]);
    renderPage(createFakeEvents(), [work, mine, glab]);
    await within(await screen.findByText("shop", { selector: ".th-repo-name b" }).then(() => rowOf("shop"))).findByRole("radiogroup", {
      name: "GitHub account for shop",
    });
    expect(pick("shop", "GitHub").getByRole("radio", { name: "private" })).toBeChecked();
    expect(pick("shop", "GitLab").getByRole("radio", { name: "corp gitlab" })).toBeChecked();
  });

  it("picks an account for the repo, and auto clears the pick", async () => {
    const put = vi
      .spyOn(reposApi, "putRepoAccounts")
      .mockResolvedValueOnce(aRepo({ ...shop, githubAccount: "private-me" }))
      .mockResolvedValueOnce(aRepo({ ...shop, githubAccount: null }));
    renderPage(createFakeEvents(), [work, mine]);
    await within(await screen.findByText("shop", { selector: ".th-repo-name b" }).then(() => rowOf("shop"))).findByRole("radiogroup", {
      name: "GitHub account for shop",
    });
    await userEvent.click(pick("shop", "GitHub").getByRole("radio", { name: "private" }));
    expect(put).toHaveBeenLastCalledWith("shop", { githubAccount: "private-me" });
    await screen.findByRole("radio", { name: "private", checked: true });
    await userEvent.click(pick("shop", "GitHub").getByRole("radio", { name: "auto" }));
    expect(put).toHaveBeenLastCalledWith("shop", { githubAccount: null });
    await screen.findAllByRole("radio", { name: "auto", checked: true });
    expect(pick("shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
  });

  it("shows the daemon's refusal and keeps the old pick", async () => {
    vi.spyOn(reposApi, "putRepoAccounts").mockRejectedValue(new ApiError("http", "no github account `private-me` is registered", 400));
    renderPage(createFakeEvents(), [work, mine]);
    await within(await screen.findByText("shop", { selector: ".th-repo-name b" }).then(() => rowOf("shop"))).findByRole("radiogroup", {
      name: "GitHub account for shop",
    });
    await userEvent.click(pick("shop", "GitHub").getByRole("radio", { name: "private" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: no github account `private-me` is registered");
    expect(pick("shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
  });
});
