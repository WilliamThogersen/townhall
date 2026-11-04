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
    const form = screen.getByRole("form", { name: "add repository" });
    expect(form).toHaveAttribute("data-layout", "stack");
    await userEvent.type(screen.getByRole("textbox", { name: "path" }), "~/code/shop");
    await userEvent.type(screen.getByRole("textbox", { name: "name" }), "shop");
    await userEvent.click(screen.getByRole("button", { name: "add" }));
    expect(add).toHaveBeenCalledWith({ path: "~/code/shop", name: "shop" });
    expect(await screen.findByTestId("repos")).toHaveTextContent("shop");
  });

  it("goes back to the button when the form is cancelled", async () => {
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    await userEvent.click(screen.getByRole("button", { name: "cancel" }));
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "add repository…" })).toBeInTheDocument();
  });

  it("opens the folder picker in the desktop app and shows a refusal under the button", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    pickFolder.mockResolvedValue("/Users/me/Downloads");
    vi.spyOn(reposApi, "addRepo").mockRejectedValue(
      new ApiError("http", "/Users/me/Downloads is not the top folder of a git repo", 400),
    );
    renderFirstRun();
    await userEvent.click(screen.getByRole("button", { name: "add repository…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: /Users/me/Downloads is not the top folder of a git repo");
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
    expect(pickFolder).toHaveBeenCalledOnce();
  });
});
