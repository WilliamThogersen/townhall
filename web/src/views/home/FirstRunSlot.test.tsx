import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/errors";
import { aRepo } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import { useRepos } from "../../data";
import { TestDataProvider } from "../../data/testing";
import * as bridge from "../../desktop/bridge";
import { FirstRunSlot } from "./FirstRunSlot";

vi.mock("../../desktop/bridge", () => ({ pickFolder: vi.fn() }));

const pickFolder = vi.mocked(bridge.pickFolder);

function RepoNames() {
  const { repos } = useRepos();
  return <p data-testid="repos">{repos?.map((repo) => repo.name).join(",")}</p>;
}

function renderFirstRun() {
  return render(
    <TestDataProvider>
      <FirstRunSlot />
      <RepoNames />
    </TestDataProvider>,
  );
}

beforeEach(() => {
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([]);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  pickFolder.mockReset();
});

describe("FirstRunSlot", () => {
  it("asks for the first repository", () => {
    const { container } = renderFirstRun();
    expect(screen.getByRole("heading", { name: "add your first repository" })).toBeInTheDocument();
    expect(screen.getByText("townhall works in git worktrees of your repos. it never writes into a repo itself.")).toBeInTheDocument();
    expect(container.querySelector(".th-empty .th-logo")).not.toBeNull();
  });

  it("adds a repository from the form in the browser", async () => {
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "shop" }));
    renderFirstRun();
    await waitFor(() => expect(reposApi.listRepos).toHaveBeenCalled());
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(screen.getByRole("dialog", { name: "add repository" })).toBeInTheDocument();
    expect(screen.getByRole("form", { name: "add repository" })).toHaveAttribute("data-layout", "stack");
    await userEvent.type(screen.getByRole("textbox", { name: "folder" }), "~/code/shop");
    await userEvent.type(screen.getByRole("textbox", { name: "name" }), "shop");
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: "~/code/shop", name: "shop" });
    expect(await screen.findByTestId("repos")).toHaveTextContent("shop");
  });

  it("goes back to the button when the form is cancelled", async () => {
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    await userEvent.click(screen.getByRole("button", { name: "cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "add repository…" })).toBeInTheDocument();
  });

  it("closes the modal with esc", async () => {
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(screen.getByRole("textbox", { name: "folder" })).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("offers folder or url in the desktop app and only opens the folder picker on browse", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    pickFolder.mockResolvedValue("/Users/me/Downloads");
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(
      new ApiError("http", "/Users/me/Downloads is not the top folder of a git repo", 400),
    );
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(pickFolder).not.toHaveBeenCalled();
    expect(screen.getByRole("radio", { name: "folder" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "url" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "browse…" }));
    expect(pickFolder).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Downloads is not the top folder of a git repo");
    expect(screen.getByRole("dialog", { name: "add repository" })).toBeInTheDocument();
  });

  it("clones from a url in the desktop app", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    const add = vi.spyOn(reposApi, "addRepo").mockResolvedValue(aRepo({ name: "shop" }));
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    await userEvent.click(screen.getByRole("radio", { name: "url" }));
    await userEvent.type(screen.getByRole("textbox", { name: "git url" }), "https://github.com/acme/shop");
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: "git@github.com:acme/shop.git" });
    expect(pickFolder).not.toHaveBeenCalled();
  });
});
