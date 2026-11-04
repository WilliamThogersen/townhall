import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Link } from "./Link";

let seen: { prevented: boolean } | null = null;

function recordAndStopJsdomNavigation(e: Event) {
  seen = { prevented: e.defaultPrevented };
  e.preventDefault();
}

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  seen = null;
  window.addEventListener("click", recordAndStopJsdomNavigation);
});

afterEach(() => window.removeEventListener("click", recordAndStopJsdomNavigation));

describe("Link", () => {
  it("renders a real link", () => {
    render(<Link to="/repos" className="x">repos</Link>);
    const link = screen.getByRole("link", { name: "repos" });
    expect(link).toHaveAttribute("href", "/repos");
    expect(link).toHaveClass("x");
  });

  it("navigates in place on a plain left click", () => {
    render(<Link to="/repos">repos</Link>);
    fireEvent.click(screen.getByRole("link"));
    expect(seen?.prevented).toBe(true);
    expect(window.location.pathname).toBe("/repos");
  });

  it("replaces the history entry when asked", () => {
    const before = window.history.length;
    render(<Link to="/search" replace>search</Link>);
    fireEvent.click(screen.getByRole("link"));
    expect(window.location.pathname).toBe("/search");
    expect(window.history.length).toBe(before);
  });

  it.each([
    ["meta", { metaKey: true }],
    ["ctrl", { ctrlKey: true }],
    ["shift", { shiftKey: true }],
    ["alt", { altKey: true }],
    ["middle", { button: 1 }],
  ])("leaves a %s click to the browser", (_, init) => {
    render(<Link to="/repos">repos</Link>);
    fireEvent.click(screen.getByRole("link"), init);
    expect(seen?.prevented).toBe(false);
    expect(window.location.pathname).toBe("/");
  });

  it("leaves a link with another target to the browser", () => {
    render(<Link to="/repos" target="_blank">repos</Link>);
    fireEvent.click(screen.getByRole("link"));
    expect(seen?.prevented).toBe(false);
    expect(window.location.pathname).toBe("/");
  });

  it("calls onClick first and respects its preventDefault", () => {
    const onClick = vi.fn((e: { preventDefault: () => void }) => e.preventDefault());
    render(<Link to="/repos" onClick={onClick}>repos</Link>);
    fireEvent.click(screen.getByRole("link"));
    expect(onClick).toHaveBeenCalledOnce();
    expect(window.location.pathname).toBe("/");
  });
});
