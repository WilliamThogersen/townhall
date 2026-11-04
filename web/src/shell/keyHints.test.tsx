import { render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { KeyHintsProvider, useCurrentKeyHints, useKeyHints, type KeyHint } from "./keyHints";

function Current() {
  const hints = useCurrentKeyHints();
  return <p>{hints.map((h) => `${h.keys}=${h.label}`).join(" ") || "none"}</p>;
}

function Hints({ hints }: { hints: readonly KeyHint[] }) {
  useKeyHints(hints);
  return null;
}

const reply: KeyHint = { keys: "r", label: "reply" };
const search: KeyHint = { keys: "/", label: "search" };

describe("key hints", () => {
  it("shows nothing until a screen registers hints", () => {
    render(<KeyHintsProvider><Current /></KeyHintsProvider>);
    expect(screen.getByText("none")).toBeInTheDocument();
  });

  it("shows the registered hints and clears them on unmount", () => {
    const { rerender } = render(
      <KeyHintsProvider><Current /><Hints hints={[reply, search]} /></KeyHintsProvider>,
    );
    expect(screen.getByText("r=reply /=search")).toBeInTheDocument();
    rerender(<KeyHintsProvider><Current /></KeyHintsProvider>);
    expect(screen.getByText("none")).toBeInTheDocument();
  });

  it("follows new hints from the same screen", () => {
    const { rerender } = render(
      <KeyHintsProvider><Current /><Hints hints={[reply]} /></KeyHintsProvider>,
    );
    rerender(<KeyHintsProvider><Current /><Hints hints={[search]} /></KeyHintsProvider>);
    expect(screen.getByText("/=search")).toBeInTheDocument();
  });

  it("keeps the newest screen on top and falls back when it leaves", () => {
    const { rerender } = render(
      <KeyHintsProvider>
        <Current />
        <Hints hints={[reply]} />
        <Hints hints={[search]} />
      </KeyHintsProvider>,
    );
    expect(screen.getByText("/=search")).toBeInTheDocument();
    rerender(
      <KeyHintsProvider>
        <Current />
        <Hints hints={[reply]} />
      </KeyHintsProvider>,
    );
    expect(screen.getByText("r=reply")).toBeInTheDocument();
  });

  it("keeps the order when an older screen changes its hints", () => {
    const quit: KeyHint = { keys: "q", label: "quit" };
    const { rerender } = render(
      <KeyHintsProvider>
        <Current />
        <Hints hints={[reply]} />
        <Hints hints={[search]} />
      </KeyHintsProvider>,
    );
    rerender(
      <KeyHintsProvider>
        <Current />
        <Hints hints={[quit]} />
        <Hints hints={[search]} />
      </KeyHintsProvider>,
    );
    expect(screen.getByText("/=search")).toBeInTheDocument();
  });

  it("is a no-op outside the provider", () => {
    expect(() => renderHook(() => useKeyHints([reply]))).not.toThrow();
    expect(renderHook(() => useCurrentKeyHints()).result.current).toEqual([]);
  });
});
