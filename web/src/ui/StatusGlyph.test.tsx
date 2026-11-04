import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AGENT_STATE_ORDER, GlyphCounts, StatusGlyph, type AgentState } from "./StatusGlyph";

const STATES: readonly AgentState[] = ["alive", "working", "waiting", "error", "dead"];

function glyphStates(container: HTMLElement) {
  return Array.from(container.querySelectorAll(".th-status"), (el) => el.getAttribute("data-state"));
}

describe("StatusGlyph", () => {
  it.each(STATES)("renders %s as an aria-hidden th-status", (state) => {
    const { container } = render(<StatusGlyph state={state} />);
    const glyph = container.firstElementChild;
    expect(glyph).toHaveClass("th-status");
    expect(glyph).toHaveAttribute("data-state", state);
    expect(glyph).toHaveAttribute("aria-hidden", "true");
  });

  it("puts a spinner inside only for working", () => {
    for (const state of STATES) {
      const { container, unmount } = render(<StatusGlyph state={state} />);
      expect(container.querySelector(".th-status .th-spin") !== null).toBe(state === "working");
      unmount();
    }
  });

  it("leaves the glyph itself to the css for every other state", () => {
    const { container } = render(<StatusGlyph state="alive" />);
    expect(container.querySelector(".th-status")).toBeEmptyDOMElement();
  });
});

describe("AGENT_STATE_ORDER", () => {
  it("lists the most urgent state first", () => {
    expect(AGENT_STATE_ORDER).toEqual(["waiting", "error", "working", "alive", "dead"]);
  });
});

describe("GlyphCounts", () => {
  it("renders one glyph and number per state, most urgent first", () => {
    const { container } = render(<GlyphCounts counts={{ dead: 2, alive: 6, waiting: 1, error: 3, working: 4 }} />);
    expect(glyphStates(container)).toEqual(["waiting", "error", "working", "alive", "dead"]);
    const visible = container.querySelector(".th-glyphs > [aria-hidden='true']");
    expect(visible?.textContent?.replace(/[⠀-⣿]/g, "")).toBe("1 3 4 6 2");
  });

  it("drops zero and missing states", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1, error: 0 }} />);
    expect(glyphStates(container)).toEqual(["waiting", "alive"]);
  });

  it("speaks the counts as text for screen readers", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1, dead: 0 }} />);
    const spoken = container.querySelector(".th-glyphs > .th-sr-only");
    expect(spoken).toHaveTextContent("1 waiting, 6 alive");
    expect(spoken).not.toHaveAttribute("aria-hidden");
  });

  it("hides the visible glyphs and numbers from assistive tech", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 1 }} />);
    expect(container.querySelector(".th-glyphs > [aria-hidden='true']")).toHaveTextContent("1");
  });

  it("adds the caller's class to the group", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 1 }} className="th-nav-count" />);
    expect(container.firstElementChild).toHaveClass("th-glyphs", "th-nav-count");
  });

  it("renders nothing when every count is zero or missing", () => {
    expect(render(<GlyphCounts counts={{}} />).container).toBeEmptyDOMElement();
    expect(render(<GlyphCounts counts={{ alive: 0, dead: 0 }} />).container).toBeEmptyDOMElement();
  });

  it("ignores counts that are not positive numbers", () => {
    const { container } = render(<GlyphCounts counts={{ alive: -1, dead: Number.NaN, error: 2 }} />);
    expect(glyphStates(container)).toEqual(["error"]);
  });
});
