import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, aDetail, aRepo, someContributions, someStats } from "../../api/fixtures";
import * as pluginsApi from "../../api/plugins";
import * as reposApi from "../../api/repos";
import * as statsApi from "../../api/stats";
import type { Repo } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { stubObjectUrls, type FakeObjectUrls } from "../../attachments/testObjectUrls";
import { HomePage } from "./HomePage";

const HERO_PLACEHOLDER = "what should we work on? name a repo, or describe the code";

const REPOS: readonly Repo[] = [aRepo({ name: "townhall" }), aRepo({ name: "shop", baseBranch: "develop" })];
const afternoon = () => new Date(2026, 10, 4, 14, 0);

function stubInfo(userName: string | null) {
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h", repo: "/r", repoKey: "r-1", userName });
}

function renderHome(repos: readonly Repo[] | Error = REPOS) {
  const list = vi.spyOn(reposApi, "listRepos");
  if (repos instanceof Error) list.mockRejectedValue(repos);
  else list.mockResolvedValue(repos);
  return renderWithHints(
    <TestDataProvider>
      <HomePage clock={afternoon} />
    </TestDataProvider>,
  );
}

const composer = () => screen.findByRole("textbox", { name: "Describe the work" });

function startersRow(): HTMLElement {
  const row = document.querySelector<HTMLElement>(".th-home-starters");
  if (!row) throw new Error("no starters row");
  return row;
}

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  stubInfo("William Thøgersen");
  vi.spyOn(statsApi, "getStats").mockResolvedValue(someStats());
  vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions());
});

afterEach(() => vi.restoreAllMocks());

function pastedImage(name = "Screenshot 2026-11-05 at 10.06.48.png"): File {
  return new File(["x"], name, { type: "image/png" });
}

function paste(input: HTMLElement, file: File) {
  return fireEvent.paste(input, { clipboardData: { files: [file], items: [], types: ["Files"] } });
}

describe("HomePage attachments", () => {
  let urls: FakeObjectUrls;
  beforeEach(() => {
    urls = stubObjectUrls();
  });
  afterEach(() => urls.restore());

  it("shows a pasted image as a thumbnail and leaves the input empty", async () => {
    renderHome();
    const input = await composer();
    const file = pastedImage();
    expect(paste(input, file)).toBe(false);
    expect(input).toHaveValue("");
    const group = await screen.findByRole("group", { name: "Attachments" });
    expect(group.querySelector("img")).toHaveAttribute("src", `blob:${file.name}`);
    expect(screen.getByRole("button", { name: "start" })).toBeEnabled();
  });

  it("removes a thumbnail with its remove button", async () => {
    renderHome();
    const input = await composer();
    const file = pastedImage();
    paste(input, file);
    fireEvent.click(await screen.findByRole("button", { name: `Remove ${file.name}` }));
    expect(screen.queryByRole("group", { name: "Attachments" })).toBeNull();
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
  });

  it("opens the lightbox from a thumbnail", async () => {
    renderHome();
    const input = await composer();
    const file = pastedImage();
    paste(input, file);
    fireEvent.click(await screen.findByRole("button", { name: `View ${file.name}` }));
    expect(await screen.findByRole("button", { name: "close esc" })).toBeInTheDocument();
  });

  it("shows a pasted file as a card", async () => {
    renderHome();
    const input = await composer();
    paste(input, new File(["notes"], "notes.txt", { type: "text/plain" }));
    const group = await screen.findByRole("group", { name: "Attachments" });
    expect(group.querySelector(".th-file")).toHaveTextContent("notes.txt");
  });

  it("starts the conversation with the text and the files, then opens it", async () => {
    const detail = aDetail({ conversation: aConversation({ id: "c43" }) });
    const start = vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(detail);
    renderHome();
    const input = await composer();
    const file = pastedImage();
    paste(input, file);
    fireEvent.change(input, { target: { value: "match this" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(window.location.pathname).toBe("/c/c43"));
    expect(start).toHaveBeenCalledWith({ text: "match this" }, [file]);
  });

  it("starts with only a file", async () => {
    const detail = aDetail({ conversation: aConversation({ id: "c44" }) });
    const start = vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(detail);
    renderHome();
    const input = await composer();
    const file = pastedImage();
    paste(input, file);
    fireEvent.click(await screen.findByRole("button", { name: "start" }));
    await waitFor(() => expect(start).toHaveBeenCalledWith({ text: "" }, [file]));
  });

  it("keeps the files when starting fails", async () => {
    vi.spyOn(conversationsApi, "startConversation").mockRejectedValue(new ApiError("network", "cannot reach the daemon"));
    renderHome();
    const input = await composer();
    paste(input, pastedImage());
    fireEvent.click(await screen.findByRole("button", { name: "start" }));
    expect(await screen.findByText("error: cannot reach the daemon")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Attachments" })).toBeInTheDocument();
  });

  it("takes files dropped on the page", async () => {
    const { container } = renderHome();
    await composer();
    const file = pastedImage("dropped.png");
    fireEvent.drop(container.querySelector(".th-home")!, { dataTransfer: { files: [file], types: ["Files"] } });
    const group = await screen.findByRole("group", { name: "Attachments" });
    expect(group.querySelector("img")).toHaveAttribute("src", "blob:dropped.png");
  });
});

describe("HomePage", () => {
  it("greets by first name with the pixel logo", async () => {
    renderHome();
    const greeting = await screen.findByRole("heading", { level: 1, name: "good afternoon, William" });
    expect(greeting.querySelector(".th-logo")).not.toBeNull();
  });

  it("greets without a name when git has none", async () => {
    stubInfo(null);
    renderHome();
    expect(await screen.findByRole("heading", { level: 1, name: "good afternoon" })).toBeInTheDocument();
  });

  it("opens with the hero composer focused and the known repos linked", async () => {
    renderHome();
    const input = await composer();
    expect(input).toHaveAttribute("placeholder", HERO_PLACEHOLDER);
    await waitFor(() => expect(input).toHaveFocus());
    const known = screen.getByRole("link", { name: "2 repos known" });
    expect(known).toHaveAttribute("href", "/repos");
    expect(known).toHaveAttribute("title", "townhall, shop");
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
    expect(shownHints()).toBe("⏎ ⇧⏎ esc");
  });

  it("fills the composer from a starter without sending", async () => {
    const start = vi.spyOn(conversationsApi, "startConversation");
    renderHome();
    const input = await composer();
    input.blur();
    fireEvent.click(screen.getByRole("button", { name: "find dead code in shop" }));
    expect(input).toHaveValue("find dead code in shop");
    await waitFor(() => expect(input).toHaveFocus());
    expect(start).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "start" })).toBeEnabled();
  });

  it("shows only the built in starters when no plugin adds any", async () => {
    renderHome();
    await composer();
    const labels = within(startersRow()).getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual(["fix the failing tests on townhall main", "find dead code in shop", "tidy the readme in townhall"]);
  });

  it("adds the starters of enabled plugins after the built in ones and fills the composer with their text", async () => {
    vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(
      someContributions({
        starters: [
          { plugin: "starter-prompts@official", title: "tidy up", text: "tidy the readme and fix what is stale" },
          { plugin: "other@acme", title: "tidy up", text: "a second plugin with the same title" },
        ],
      }),
    );
    const start = vi.spyOn(conversationsApi, "startConversation");
    renderHome();
    const input = await composer();
    await screen.findAllByRole("button", { name: "tidy up" });
    const labels = within(startersRow()).getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual([
      "fix the failing tests on townhall main",
      "find dead code in shop",
      "tidy the readme in townhall",
      "tidy up",
      "tidy up",
    ]);
    input.blur();
    fireEvent.click(screen.getAllByRole("button", { name: "tidy up" })[0]!);
    expect(input).toHaveValue("tidy the readme and fix what is stale");
    expect(start).not.toHaveBeenCalled();
    fireEvent.click(screen.getAllByRole("button", { name: "tidy up" })[1]!);
    expect(input).toHaveValue("a second plugin with the same title");
  });

  it("keeps the built in starters when the plugin contributions cannot be loaded", async () => {
    vi.spyOn(pluginsApi, "getContributions").mockRejectedValue(new Error("down"));
    renderHome();
    await composer();
    await waitFor(() => expect(pluginsApi.getContributions).toHaveBeenCalled());
    expect(within(startersRow()).getAllByRole("button")).toHaveLength(3);
  });

  it("starts a conversation on enter and opens it", async () => {
    const detail = aDetail({ conversation: aConversation({ id: "c42" }) });
    const start = vi.spyOn(conversationsApi, "startConversation").mockResolvedValue(detail);
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "  fix the login loop  " } });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(start).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(window.location.pathname).toBe("/c/c42"));
    expect(start).toHaveBeenCalledWith({ text: "fix the login loop" }, []);
  });

  it("keeps the text and shows the error when starting fails", async () => {
    vi.spyOn(conversationsApi, "startConversation").mockRejectedValue(
      new ApiError("network", "cannot reach the daemon"),
    );
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "fix the login loop" } });
    fireEvent.click(screen.getByRole("button", { name: "start" }));
    expect(await screen.findByText("error: cannot reach the daemon")).toBeInTheDocument();
    expect(input).toHaveValue("fix the login loop");
    expect(window.location.pathname).toBe("/");
  });

  it("does not start with only spaces", async () => {
    const start = vi.spyOn(conversationsApi, "startConversation");
    renderHome();
    const input = await composer();
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.submit(input.closest("form")!);
    expect(start).not.toHaveBeenCalled();
  });

  it("leaves the input on esc and comes back on n", async () => {
    renderHome();
    const input = await composer();
    await waitFor(() => expect(input).toHaveFocus());
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input).not.toHaveFocus();
    fireEvent.keyDown(document.body, { key: "n", metaKey: true });
    expect(input).not.toHaveFocus();
    fireEvent.keyDown(document.body, { key: "n" });
    expect(input).toHaveFocus();
  });

  it("shows the shipped stats", async () => {
    renderHome();
    expect(await screen.findByText("2 bug fixes shipped in the last 12 months")).toBeInTheDocument();
  });

  it("shows first run when there is no repo", async () => {
    renderHome([]);
    expect(await screen.findByRole("heading", { name: "add your first repository" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "add repository…" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).toBeNull();
    await waitFor(() => expect(shownHints()).toBe("/"));
  });

  it("shows the add form's keys on first run while the form is open", async () => {
    renderHome([]);
    fireEvent.click(await screen.findByRole("button", { name: "add repository…" }));
    expect(screen.getByRole("form", { name: "add repository" })).toBeInTheDocument();
    await waitFor(() => expect(shownHints()).toBe("⏎ esc"));
    fireEvent.click(screen.getByRole("button", { name: "cancel" }));
    await waitFor(() => expect(shownHints()).toBe("/"));
  });

  it("still offers the composer when the repos do not load", async () => {
    renderHome(new ApiError("network", "cannot reach the daemon"));
    await composer();
    expect(screen.queryByRole("link", { name: /known/ })).toBeNull();
    expect(screen.getByRole("button", { name: "find slow queries in search" })).toBeInTheDocument();
  });

  it("shows an empty home while the repos load", () => {
    vi.spyOn(reposApi, "listRepos").mockReturnValue(new Promise(() => undefined));
    const { container } = renderWithHints(
      <TestDataProvider>
        <HomePage />
      </TestDataProvider>,
    );
    act(() => undefined);
    expect(container.querySelector(".th-home")).toBeEmptyDOMElement();
  });
});
