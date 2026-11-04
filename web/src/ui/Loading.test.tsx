import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Loading } from "./Loading";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Loading", () => {
  it("renders nothing for the first 300ms", () => {
    const { container } = render(<Loading verb="loading agents" />);
    expect(container).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(299));
    expect(container).toBeEmptyDOMElement();
  });

  it("then shows a spinner and the verb as a status", () => {
    render(<Loading verb="loading agents" />);
    act(() => vi.advanceTimersByTime(300));
    const status = screen.getByRole("status");
    expect(status).toHaveClass("th-loading");
    expect(status.querySelector(".th-spin")).toHaveAttribute("aria-hidden", "true");
    expect(status).toHaveTextContent("loading agents…");
  });

  it("waits for a custom delay", () => {
    const { container } = render(<Loading verb="starting" delayMs={1000} />);
    act(() => vi.advanceTimersByTime(999));
    expect(container).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("status")).toHaveTextContent("starting…");
  });

  it("shows at once with no delay", () => {
    render(<Loading verb="loading terminal" delayMs={0} />);
    expect(screen.getByRole("status")).toHaveTextContent("loading terminal…");
  });

  it("clears its timer when unmounted early", () => {
    const { unmount } = render(<Loading verb="loading agents" />);
    expect(vi.getTimerCount()).toBe(1);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
