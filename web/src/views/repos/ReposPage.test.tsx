import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, anAgentInfo, aRepo, aTask } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import * as tasksApi from "../../api/tasks";
import * as bridge from "../../desktop/bridge";
import { createFakeEvents, TestDataProvider, type FakeEvents } from "../../data/testing";
import { DEFAULT_SETTINGS, type ForgeAccount, type Settings } from "../../settings/settings";
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
) {
  const settings = createSettingsStore({
    load: async () => ({ ...DEFAULT_SETTINGS, forgeAccounts, ...over }),
    put: async () => DEFAULT_SETTINGS,
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

  const openAdd = async () => userEvent.click(await screen.findByRole("button", { name: "add repository…" }));
  const chooseUrl = () => userEvent.click(screen.getByRole("radio", { name: "url" }));
  const dialog = () => screen.getByRole("dialog", { name: "add repository" });

  it("adds a folder from the modal and shows it at once", async () => {
    const added = aRepo({ name: "data-hub", path: "/Users/me/Developer/data-hub", baseBranch: "trunk" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    const open = screen.getByRole("button", { name: "add repository…" });
    expect(open).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await userEvent.click(open);
    expect(open).toHaveAttribute("aria-expanded", "true");
    const modal = within(dialog());
    expect(modal.getByRole("radio", { name: "folder" })).toBeChecked();
    expect(modal.getByRole("radio", { name: "url" })).not.toBeChecked();
    expect(modal.getByRole("textbox", { name: "folder" })).toHaveFocus();
    expect(modal.queryByRole("button", { name: "browse…" })).not.toBeInTheDocument();
    expect(modal.getByRole("button", { name: "add" })).toBeDisabled();
    expect(screen.getByTestId("hints")).toHaveTextContent("⏎=add | esc=cancel");
    await userEvent.type(modal.getByRole("textbox", { name: "folder" }), "~/Developer/data-hub");
    await userEvent.type(modal.getByRole("textbox", { name: "base branch" }), "trunk{Enter}");
    expect(add).toHaveBeenCalledWith({ path: "~/Developer/data-hub", baseBranch: "trunk" });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("3", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
  });

  it("adds by git url from the url choice and sends it as the path", async () => {
    const link = "git@github.com:acme/data-hub.git";
    const added = aRepo({ name: "data-hub", path: "/Users/me/.townhall/repos/data-hub", baseBranch: "main" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await openAdd();
    await chooseUrl();
    expect(screen.queryByRole("textbox", { name: "folder" })).not.toBeInTheDocument();
    expect(screen.getByPlaceholderText(/github or gitlab/)).toBe(screen.getByRole("textbox", { name: "git url" }));
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), `${link}{Enter}`);
    expect(add).toHaveBeenCalledWith({ path: link });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
  });

  it("says what a url will clone and where, and takes an optional clone-into folder", async () => {
    const link = "https://github.com/acme/data-hub.git";
    const added = aRepo({ name: "data-hub", path: "/Users/me/Developer/data-hub", baseBranch: "main" });
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
    renderPage();
    await openAdd();
    await chooseUrl();
    const into = screen.getByRole("textbox", { name: "clone into" });
    expect(into).toHaveAttribute("placeholder", "~/.townhall/repos");
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), link);
    expect(screen.getByText("clones git@github.com:acme/data-hub.git into ~/.townhall/repos/data-hub")).toBeInTheDocument();
    await userEvent.type(into, "~/Developer");
    expect(screen.getByText("clones git@github.com:acme/data-hub.git into ~/Developer/data-hub")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: "git@github.com:acme/data-hub.git", cloneDir: "~/Developer" });
    expect(await screen.findByText("data-hub", { selector: ".th-repo-name b" })).toBeInTheDocument();
  });

  it("clones a pasted https link over ssh by default and over https when chosen", async () => {
    const link = "https://gitlab.wexo.io/konpap/vines.git";
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "vines", path: "/Users/me/.townhall/repos/vines", baseBranch: "main" }));
    renderPage();
    await openAdd();
    expect(screen.queryByRole("radio", { name: "ssh" })).not.toBeInTheDocument();
    await chooseUrl();
    expect(screen.getByRole("radio", { name: "ssh" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "https" })).not.toBeChecked();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), link);
    expect(screen.getByText("clones git@gitlab.wexo.io:konpap/vines.git into ~/.townhall/repos/vines")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "https" }));
    expect(screen.getByText(`clones ${link} into ~/.townhall/repos/vines`)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "ssh" }));
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: "git@gitlab.wexo.io:konpap/vines.git" });
  });

  it("sends the https form when https is chosen", async () => {
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "vines", path: "/Users/me/.townhall/repos/vines", baseBranch: "main" }));
    renderPage();
    await openAdd();
    await chooseUrl();
    await userEvent.click(screen.getByRole("radio", { name: "https" }));
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "git@gitlab.wexo.io:konpap/vines{Enter}");
    expect(add).toHaveBeenCalledWith({ path: "https://gitlab.wexo.io/konpap/vines.git" });
  });

  it("offers the clones folder from the settings as the default", async () => {
    renderPage(createFakeEvents(), [], { clonesDir: "~/Developer" });
    await openAdd();
    await chooseUrl();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "git@gitlab.com:group/sub/app.git");
    expect(await screen.findByText("clones git@gitlab.com:group/sub/app.git into ~/Developer/app")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "clone into" })).toHaveAttribute("placeholder", "~/Developer");
  });

  it("starts empty again when the choice changes", async () => {
    renderPage();
    await openAdd();
    await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "~/Developer/notes");
    await chooseUrl();
    expect(screen.getByRole("textbox", { name: "git url" })).toHaveValue("");
    expect(screen.getByRole("button", { name: "add" })).toBeDisabled();
  });

  it("refuses a url that names no repo before asking the daemon", async () => {
    const add = vi.spyOn(reposApi, "addRepo");
    renderPage();
    await openAdd();
    await chooseUrl();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "https://github.com/acme{Enter}");
    expect(screen.getByRole("alert")).toHaveTextContent("expected host/owner/repo");
    expect(screen.getByRole("button", { name: "add" })).toBeDisabled();
    expect(add).not.toHaveBeenCalled();
  });

  it("refuses a url in the folder choice and a plain path in the url choice", async () => {
    const add = vi.spyOn(reposApi, "addRepo");
    renderPage();
    await openAdd();
    await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "https://github.com/acme/shop{Enter}");
    expect(screen.getByRole("alert")).toHaveTextContent("choose url");
    await chooseUrl();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "~/Developer/shop{Enter}");
    expect(screen.getByRole("alert")).toHaveTextContent("expected a git url");
    expect(add).not.toHaveBeenCalled();
  });

  it("shows the daemon's reason when a clone fails and keeps the modal open", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(new ApiError("http", "/Users/me/Developer/data-hub already exists and is not a clone of x", 409));
    renderPage();
    await openAdd();
    await chooseUrl();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "https://github.com/acme/data-hub");
    await userEvent.type(screen.getByRole("textbox", { name: "clone into" }), "~/Developer");
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Developer/data-hub already exists");
    expect(dialog()).toBeInTheDocument();
  });

  describe("editing a repo", () => {
    const modal = () => screen.getByRole("dialog", { name: "edit shop" });
    const openEdit = async (name = "shop") => {
      await screen.findByText(name, { selector: ".th-repo-name b" });
      await userEvent.click(within(rowOf(name)).getByRole("button", { name: `edit ${name}` }));
    };

    it("keeps the row to the facts and two actions", async () => {
      renderPage(createFakeEvents(), [{ forge: "github", login: "me", host: "github.com", label: "me" }]);
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      const row = within(rowOf("shop"));
      expect(row.getByRole("button", { name: "edit shop" })).toBeInTheDocument();
      expect(row.getByRole("link", { name: "new conversation" })).toBeInTheDocument();
      expect(row.queryByRole("button", { name: /remove/ })).not.toBeInTheDocument();
      expect(row.queryByRole("radiogroup")).not.toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("renames it, changes its base branch and shows the saved repo at once", async () => {
      const saved = aRepo({ ...shop, name: "store", baseBranch: "main" });
      const patch = vi.spyOn(reposApi, "patchRepo").mockResolvedValue(saved);
      renderPage();
      await openEdit();
      const f = within(modal());
      expect(f.getByRole("textbox", { name: "name" })).toHaveValue("shop");
      expect(f.getByRole("textbox", { name: "name" })).toHaveFocus();
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
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("points the repo at a new folder, or moves the folder when asked", async () => {
      const patch = vi.spyOn(reposApi, "patchRepo").mockResolvedValue(aRepo({ ...shop, path: "/code/shop" }));
      renderPage();
      await openEdit();
      const f = within(modal());
      expect(f.queryByRole("checkbox")).not.toBeInTheDocument();
      await userEvent.clear(f.getByRole("textbox", { name: "folder" }));
      await userEvent.type(f.getByRole("textbox", { name: "folder" }), "/code/shop");
      await userEvent.click(f.getByRole("checkbox", { name: /move the folder/ }));
      await userEvent.click(f.getByRole("button", { name: "save" }));
      expect(patch).toHaveBeenCalledWith("shop", { path: "/code/shop", moveFolder: true });
    });

    it("keeps the modal open with the daemon's reason when it refuses", async () => {
      vi.spyOn(reposApi, "patchRepo").mockRejectedValue(new ApiError("http", "a repo named `townhall` already exists", 409));
      renderPage();
      await openEdit();
      await userEvent.clear(within(modal()).getByRole("textbox", { name: "name" }));
      await userEvent.type(within(modal()).getByRole("textbox", { name: "name" }), "townhall{Enter}");
      expect(await within(modal()).findByRole("alert")).toHaveTextContent("error: a repo named `townhall` already exists");
      expect(within(modal()).getByRole("textbox", { name: "name" })).toHaveValue("townhall");
    });

    it("does not save a name the daemon would refuse, and cancels with escape", async () => {
      const patch = vi.spyOn(reposApi, "patchRepo");
      renderPage();
      await openEdit();
      await userEvent.type(within(modal()).getByRole("textbox", { name: "name" }), " x");
      expect(within(modal()).getByRole("button", { name: "save" })).toBeDisabled();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(patch).not.toHaveBeenCalled();
      expect(screen.getByText("shop", { selector: ".th-repo-name b" })).toBeInTheDocument();
    });

    it("closes with cancel and returns focus to the edit button", async () => {
      renderPage();
      await openEdit();
      await userEvent.click(within(modal()).getByRole("button", { name: "cancel" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(within(rowOf("shop")).getByRole("button", { name: "edit shop" })).toHaveFocus();
    });

    it("does not open the add modal on top when a is pressed inside it", async () => {
      renderPage();
      await openEdit();
      within(modal()).getByRole("button", { name: "cancel" }).focus();
      await userEvent.keyboard("a");
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
      expect(modal()).toBeInTheDocument();
    });

    it("asks before removing a repo and can be cancelled", async () => {
      const remove = vi.spyOn(reposApi, "removeRepo");
      renderPage();
      await openEdit();
      await userEvent.click(within(modal()).getByRole("button", { name: "remove shop" }));
      expect(within(modal()).getByText("remove shop? the folder and its worktrees stay on disk")).toBeInTheDocument();
      const confirm = within(screen.getByRole("group", { name: "remove shop" }));
      expect(confirm.getByRole("button", { name: "cancel" })).toHaveFocus();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByText(/the folder and its worktrees stay on disk/)).not.toBeInTheDocument();
      expect(modal()).toBeInTheDocument();
      expect(within(modal()).getByRole("button", { name: "remove shop" })).toHaveFocus();
      expect(remove).not.toHaveBeenCalled();
    });

    it("removes a repo once confirmed", async () => {
      const remove = vi.spyOn(reposApi, "removeRepo").mockResolvedValue(shop);
      renderPage();
      await openEdit();
      await userEvent.click(within(modal()).getByRole("button", { name: "remove shop" }));
      await userEvent.click(within(screen.getByRole("group", { name: "remove shop" })).getByRole("button", { name: "remove" }));
      expect(remove).toHaveBeenCalledWith("shop");
      expect(screen.queryByText("shop", { selector: ".th-repo-name b" })).not.toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(screen.getByText("1", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
    });

    it("offers to delete the clone only for repos cloned into the managed folder", async () => {
      const clone = aRepo({ name: "hub", path: "/Users/me/.townhall/repos/hub", baseBranch: "main" });
      vi.mocked(reposApi.listRepos).mockResolvedValue([shop, clone]);
      const remove = vi.spyOn(reposApi, "removeRepo").mockResolvedValue(clone);
      renderPage();
      await openEdit();
      await userEvent.click(within(modal()).getByRole("button", { name: "remove shop" }));
      expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
      await userEvent.keyboard("{Escape}{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      await openEdit("hub");
      const hub = within(screen.getByRole("dialog", { name: "edit hub" }));
      await userEvent.click(hub.getByRole("button", { name: "remove hub" }));
      await userEvent.click(hub.getByRole("checkbox", { name: "also delete the clone" }));
      await userEvent.click(within(screen.getByRole("group", { name: "remove hub" })).getByRole("button", { name: "remove" }));
      expect(remove).toHaveBeenCalledWith("hub", true);
    });

    it("shows the daemon's conflict as is, closes the modal and lets you dismiss it", async () => {
      vi.spyOn(reposApi, "removeRepo").mockRejectedValue(
        new ApiError("http", "repo `shop` has 2 running agents: stop them first", 409),
      );
      renderPage();
      await openEdit();
      await userEvent.click(within(modal()).getByRole("button", { name: "remove shop" }));
      await userEvent.click(within(screen.getByRole("group", { name: "remove shop" })).getByRole("button", { name: "remove" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("error: repo `shop` has 2 running agents: stop them first");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "dismiss" }));
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  describe("searching repos", () => {
    const search = () => screen.getByRole("searchbox", { name: "search repos" });
    const names = () => screen.queryAllByText(/^(townhall|shop)$/, { selector: ".th-repo-name b" }).map((b) => b.textContent);

    it("filters live by name, case-insensitively", async () => {
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      expect(names()).toEqual(["townhall", "shop"]);
      await userEvent.type(search(), "SHO");
      expect(names()).toEqual(["shop"]);
    });

    it("filters by path too", async () => {
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.type(search(), "wexo/work");
      expect(names()).toEqual(["shop"]);
    });

    it("says when nothing matches and shows all again once cleared", async () => {
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.type(search(), "zzz");
      expect(names()).toEqual([]);
      expect(screen.getByRole("heading", { name: 'no repos match "zzz"' })).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "no repos yet" })).not.toBeInTheDocument();
      await userEvent.clear(search());
      expect(names()).toEqual(["townhall", "shop"]);
    });

    it("keeps the header count at the total and does not open the add modal while typing a", async () => {
      renderPage();
      await screen.findByText("shop", { selector: ".th-repo-name b" });
      await userEvent.type(search(), "sha");
      expect(screen.getByText("2", { selector: ".th-page-head .th-cap" })).toBeInTheDocument();
      expect(search()).toHaveValue("sha");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("is not shown before there are repos", async () => {
      vi.mocked(reposApi.listRepos).mockResolvedValue([]);
      renderPage();
      await screen.findByRole("heading", { name: "no repos yet" });
      expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    });
  });

  it("says cloning while a url is added", async () => {
    vi.spyOn(reposApi, "addRepo").mockReturnValue(new Promise(() => undefined));
    renderPage();
    await openAdd();
    await chooseUrl();
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "https://github.com/acme/data-hub{Enter}");
    expect(await screen.findByRole("status")).toHaveTextContent("cloning…");
  });

  it("says initialising while a folder is added", async () => {
    vi.spyOn(reposApi, "addRepo").mockReturnValue(new Promise(() => undefined));
    renderPage();
    await openAdd();
    await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "~/Developer/notes{Enter}");
    expect(await screen.findByRole("status")).toHaveTextContent("initialising…");
  });

  it("shows the daemon's refusal in the modal and keeps what you typed", async () => {
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(
      new ApiError("http", "/Users/me/Developer is not the top folder of a git repo", 400),
    );
    renderPage();
    await openAdd();
    await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "~/Developer{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Developer is not the top folder of a git repo");
    expect(screen.getByRole("textbox", { name: "folder" })).toHaveValue("~/Developer");
  });

  it("opens the modal with a and closes it with esc", async () => {
    renderPage();
    await screen.findByText("shop", { selector: ".th-repo-name b" });
    expect(screen.getByTestId("hints")).toHaveTextContent("a=add repository");
    await userEvent.keyboard("a");
    const path = screen.getByRole("textbox", { name: "folder" });
    expect(path).toHaveFocus();
    expect(path).toHaveValue("");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes the modal with cancel", async () => {
    renderPage();
    await openAdd();
    await userEvent.click(within(dialog()).getByRole("button", { name: "cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  describe("in the desktop app", () => {
    beforeEach(() => vi.stubGlobal("__TAURI_INTERNALS__", {}));
    afterEach(() => vi.unstubAllGlobals());

    it("opens the modal instead of the native dialog and browses only on request", async () => {
      const pick = vi.spyOn(bridge, "pickFolder").mockResolvedValue("/Users/me/code/notes");
      const added = aRepo({ name: "notes", path: "/Users/me/code/notes" });
      const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(added);
      renderPage();
      await openAdd();
      expect(pick).not.toHaveBeenCalled();
      const modal = within(dialog());
      expect(modal.getByRole("radio", { name: "folder" })).toBeChecked();
      expect(modal.getByRole("radio", { name: "url" })).toBeInTheDocument();
      await userEvent.click(modal.getByRole("button", { name: "browse…" }));
      expect(pick).toHaveBeenCalledTimes(1);
      expect(modal.getByRole("textbox", { name: "folder" })).toHaveValue("/Users/me/code/notes");
      expect(add).not.toHaveBeenCalled();
      await userEvent.click(modal.getByRole("button", { name: "add" }));
      expect(add).toHaveBeenCalledWith({ path: "/Users/me/code/notes" });
      expect(await screen.findByText("notes", { selector: ".th-repo-name b" })).toBeInTheDocument();
    });

    it("keeps the path when the native dialog is cancelled", async () => {
      vi.spyOn(bridge, "pickFolder").mockResolvedValue(null);
      renderPage();
      await openAdd();
      await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "~/code");
      await userEvent.click(screen.getByRole("button", { name: "browse…" }));
      expect(screen.getByRole("textbox", { name: "folder" })).toHaveValue("~/code");
    });

    it("offers the url choice with its clone-into field and no browse button", async () => {
      renderPage();
      await openAdd();
      await chooseUrl();
      expect(screen.queryByRole("button", { name: "browse…" })).not.toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "git url" })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "clone into" })).toBeInTheDocument();
    });
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
  const editing = async (name = "shop") => {
    await screen.findByText(name, { selector: ".th-repo-name b" });
    await userEvent.click(within(rowOf(name)).getByRole("button", { name: `edit ${name}` }));
    return within(screen.getByRole("dialog", { name: `edit ${name}` }));
  };
  const pick = (modal: ReturnType<typeof within>, name: string, forge: "GitHub" | "GitLab") =>
    within(modal.getByRole("radiogroup", { name: `${forge} account for ${name}` }));

  it("shows no account choice while no account is registered", async () => {
    renderPage();
    const modal = await editing();
    expect(modal.queryByRole("radiogroup", { name: /account for/ })).not.toBeInTheDocument();
  });

  it("offers auto and each registered account, only for the forges that have one", async () => {
    renderPage(createFakeEvents(), [work, mine]);
    const modal = await editing();
    const labels = pick(modal, "shop", "GitHub").getAllByRole("radio").map((radio) => radio.closest("label")?.textContent);
    expect(labels).toEqual(["auto", "work", "private"]);
    expect(pick(modal, "shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
    expect(modal.queryByRole("radiogroup", { name: /GitLab/ })).not.toBeInTheDocument();
  });

  it("shows the account a repo already uses and the gitlab choice when one is registered", async () => {
    vi.mocked(reposApi.listRepos).mockResolvedValue([aRepo({ ...shop, githubAccount: "private-me", gitlabAccount: "me" })]);
    renderPage(createFakeEvents(), [work, mine, glab]);
    const modal = await editing();
    expect(pick(modal, "shop", "GitHub").getByRole("radio", { name: "private" })).toBeChecked();
    expect(pick(modal, "shop", "GitLab").getByRole("radio", { name: "corp gitlab" })).toBeChecked();
  });

  it("picks an account for the repo, and auto clears the pick", async () => {
    const put = vi
      .spyOn(reposApi, "putRepoAccounts")
      .mockResolvedValueOnce(aRepo({ ...shop, githubAccount: "private-me" }))
      .mockResolvedValueOnce(aRepo({ ...shop, githubAccount: null }));
    renderPage(createFakeEvents(), [work, mine]);
    const modal = await editing();
    await userEvent.click(pick(modal, "shop", "GitHub").getByRole("radio", { name: "private" }));
    expect(put).toHaveBeenLastCalledWith("shop", { githubAccount: "private-me" });
    await screen.findByRole("radio", { name: "private", checked: true });
    await userEvent.click(pick(modal, "shop", "GitHub").getByRole("radio", { name: "auto" }));
    expect(put).toHaveBeenLastCalledWith("shop", { githubAccount: null });
    await screen.findAllByRole("radio", { name: "auto", checked: true });
    expect(pick(modal, "shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
  });

  it("shows the daemon's refusal and keeps the old pick", async () => {
    vi.spyOn(reposApi, "putRepoAccounts").mockRejectedValue(new ApiError("http", "no github account `private-me` is registered", 400));
    renderPage(createFakeEvents(), [work, mine]);
    const modal = await editing();
    await userEvent.click(pick(modal, "shop", "GitHub").getByRole("radio", { name: "private" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: no github account `private-me` is registered");
    expect(pick(modal, "shop", "GitHub").getByRole("radio", { name: "auto" })).toBeChecked();
  });
});
