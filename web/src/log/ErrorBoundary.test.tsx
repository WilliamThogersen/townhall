import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";
import { resetReportsForTests } from "./report";

function Bomb(): never {
  throw new Error("render exploded");
}

beforeEach(() => {
  vi.useFakeTimers();
  resetReportsForTests();
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("ErrorBoundary", () => {
  it("shows the fallback and reports web.render_error", async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(null, { status: 202 }));
    vi.stubGlobal("fetch", fetchFn);
    render(
      <ErrorBoundary>
        <Bomb />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toBeInTheDocument();
    await vi.advanceTimersByTimeAsync(1500);
    const body = JSON.parse((fetchFn.mock.calls[0]?.[1] as RequestInit).body as string);
    expect(body[0]).toMatchObject({
      level: "error",
      event: "web.render_error",
      message: expect.stringContaining("render exploded"),
    });
  });

  it("renders children when nothing throws", () => {
    render(
      <ErrorBoundary>
        <p>fine</p>
      </ErrorBoundary>,
    );
    expect(screen.getByText("fine")).toBeInTheDocument();
  });
});
