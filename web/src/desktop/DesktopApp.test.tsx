import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import * as bridge from "./bridge";
import { DesktopApp } from "./DesktopApp";

vi.mock("./bridge", () => ({ repoStatus: vi.fn(), chooseRepo: vi.fn() }));

const repoStatus = vi.mocked(bridge.repoStatus);
const chooseRepo = vi.mocked(bridge.chooseRepo);

function deferred<T>() {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  vi.spyOn(client, "listAgents").mockResolvedValue([]);
  vi.spyOn(client, "getInfo").mockResolvedValue({ home: "/h", repo: "/r", repoKey: "r-1" });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  repoStatus.mockReset();
  chooseRepo.mockReset();
});

describe("DesktopApp", () => {
  it("says it is starting while the repo status loads", async () => {
    repoStatus.mockReturnValue(new Promise(() => {}));
    render(<DesktopApp />);
    expect(await screen.findByRole("status")).toHaveTextContent("starting…");
  });

  it("asks for a repository on first run and shows the picker as busy", async () => {
    const picked = deferred<bridge.RepoStatus>();
    repoStatus.mockResolvedValue({ repo: null, mode: "stopped" });
    chooseRepo.mockReturnValue(picked.promise);
    const { container } = render(<DesktopApp />);
    expect(await screen.findByRole("heading", { name: "choose the git repository your agents work in" })).toBeInTheDocument();
    expect(container.querySelector(".th-empty .th-logo")).not.toBeNull();
    expect(screen.getByRole("button", { name: /choosing…/ })).toBeDisabled();
    picked.resolve({ repo: "/r/demo", mode: "embedded" });
    expect(await screen.findByRole("navigation", { name: "townhall" })).toBeInTheDocument();
  });

  it("shows a picker error and lets you try again", async () => {
    repoStatus.mockResolvedValue({ repo: "/r/gone", mode: "stopped" });
    chooseRepo.mockRejectedValue(new Error("not a git repository: /tmp"));
    render(<DesktopApp />);
    fireEvent.click(await screen.findByRole("button", { name: "choose repository…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: not a git repository: /tmp");
    expect(chooseRepo).toHaveBeenCalledOnce();
  });

  it("shows the shell once the daemon runs, with no repo control in the sidebar", async () => {
    repoStatus.mockResolvedValue({ repo: "/Users/me/code/demo", mode: "embedded" });
    render(<DesktopApp />);
    const nav = await screen.findByRole("navigation", { name: "townhall" });
    expect(within(nav).queryByRole("button", { name: /change repo/ })).not.toBeInTheDocument();
    expect(within(nav).getByRole("link", { name: "settings" })).toBeInTheDocument();
    await waitFor(() => expect(client.listAgents).toHaveBeenCalled());
  });

  it("changes the repository from the repos page", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    window.history.replaceState(null, "", "/repos");
    repoStatus.mockResolvedValue({ repo: "/Users/me/code/demo", mode: "embedded" });
    chooseRepo.mockResolvedValue({ repo: "/Users/me/code/other", mode: "embedded" });
    vi.mocked(client.getInfo).mockResolvedValue({ home: "/Users/me/.townhall", repo: "/Users/me/code/demo", repoKey: "demo-1" });
    render(<DesktopApp />);
    expect(await screen.findByText("demo", { selector: ".th-repo-name b" })).toBeInTheDocument();
    vi.mocked(client.getInfo).mockResolvedValue({ home: "/Users/me/.townhall", repo: "/Users/me/code/other", repoKey: "other-1" });
    fireEvent.click(screen.getByRole("button", { name: "change repository…" }));
    expect(await screen.findByText("other", { selector: ".th-repo-name b" })).toBeInTheDocument();
    expect(chooseRepo).toHaveBeenCalledOnce();
  });
});
