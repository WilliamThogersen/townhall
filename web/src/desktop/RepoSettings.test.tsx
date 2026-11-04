import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RepoSettings, repoLabel } from "./RepoSettings";
import type { RepoState } from "./useRepo";

const state = (patch: Partial<RepoState>): RepoState => ({
  status: { repo: "/Users/me/code/demo", mode: "embedded" },
  busy: false,
  error: null,
  choose: vi.fn().mockResolvedValue(undefined),
  ...patch,
});

describe("repoLabel", () => {
  it("names the repo folder, the external daemon or no repo", () => {
    expect(repoLabel({ repo: "/Users/me/code/demo/", mode: "embedded" })).toBe("demo");
    expect(repoLabel({ repo: "/r", mode: "external" })).toBe("daemon on :7777");
    expect(repoLabel({ repo: null, mode: "stopped" })).toBe("no repository");
    expect(repoLabel(null)).toBe("no repository");
  });
});

describe("RepoSettings", () => {
  it("opens the picker from a ghost button", () => {
    const repo = state({});
    render(<RepoSettings repo={repo} />);
    const button = screen.getByRole("button", { name: "change repo…" });
    expect(button).toHaveClass("th-btn--ghost", "th-btn--sm");
    fireEvent.click(button);
    expect(repo.choose).toHaveBeenCalledOnce();
  });

  it("shows busy while the picker is open", () => {
    render(<RepoSettings repo={state({ busy: true })} />);
    expect(screen.getByRole("button", { name: /choosing…/ })).toBeDisabled();
  });

  it("cannot change the repo of an external daemon", () => {
    render(<RepoSettings repo={state({ status: { repo: "/r", mode: "external" } })} />);
    expect(screen.getByText("daemon on :7777")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "change repo…" })).toBeDisabled();
  });

  it("shows a picker error in place of the repo", () => {
    render(<RepoSettings repo={state({ error: "not a git repository: /tmp" })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("error: not a git repository: /tmp");
  });
});
