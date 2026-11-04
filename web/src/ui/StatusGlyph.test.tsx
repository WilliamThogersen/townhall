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

function visibleText(element: Element) {
  const copy = element.cloneNode(true) as Element;
  copy.querySelectorAll(".th-sr-only").forEach((node) => node.remove());
  return copy.textContent?.replace(/[\u2800-\u28ff]/g, "") ?? "";
}

function spokenText(container: HTMLElement) {
  return container.querySelector(".th-sr-only")?.textContent;
}

describe("GlyphCounts", () => {
  it("renders glyph and number per state, most urgent first, like the nav count", () => {
    const { container } = render(
      <GlyphCounts counts={{ dead: 2, alive: 6, waiting: 1, error: 3, working: 4 }} className="th-nav-count" />,
    );
    const group = container.firstElementChild;
    expect(group?.tagName).toBe("SPAN");
    expect(group).toHaveAttribute("class", "th-nav-count");
    expect(glyphStates(container)).toEqual(["waiting", "error", "working", "alive", "dead"]);
    expect(visibleText(group!)).toBe("1 3 4 6 2");
  });

  it("puts each glyph right before its number", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1 }} />);
    const children = Array.from(container.firstElementChild!.children);
    expect(children[0]).toHaveAttribute("data-state", "waiting");
    expect(children[1]).toHaveTextContent("1");
    expect(children[2]).toHaveAttribute("data-state", "alive");
    expect(children[3]).toHaveTextContent("6");
  });

  it("is a plain span without the glyph strip class when no class is given", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 1 }} />);
    expect(container.firstElementChild?.tagName).toBe("SPAN");
    expect(container.firstElementChild).not.toHaveAttribute("class");
    expect(container.querySelector(".th-glyphs")).toBeNull();
  });

  it("drops zero and missing states", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1, error: 0 }} />);
    expect(glyphStates(container)).toEqual(["waiting", "alive"]);
  });

  it("speaks the counts with state words for screen readers", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1, error: 1, working: 2, dead: 3 }} />);
    expect(spokenText(container)).toBe("1 waiting, 1 error, 2 working, 6 running, 3 ended");
    expect(container.querySelector(".th-sr-only")).not.toHaveAttribute("aria-hidden");
  });

  it("hides the glyphs and the bare numbers from assistive tech", () => {
    const { container } = render(<GlyphCounts counts={{ alive: 6, waiting: 1 }} />);
    const shown = Array.from(container.firstElementChild!.children).filter((el) => !el.classList.contains("th-sr-only"));
    expect(shown).toHaveLength(4);
    shown.forEach((el) => expect(el).toHaveAttribute("aria-hidden", "true"));
  });

  it("renders nothing when every count is zero or missing", () => {
    expect(render(<GlyphCounts counts={{}} />).container).toBeEmptyDOMElement();
    expect(render(<GlyphCounts counts={{ alive: 0, dead: 0 }} />).container).toBeEmptyDOMElement();
    expect(render(<GlyphCounts counts={{}} labelled />).container).toBeEmptyDOMElement();
  });

  it("ignores counts that are not positive numbers", () => {
    const { container } = render(<GlyphCounts counts={{ alive: -1, dead: Number.NaN, error: 2 }} />);
    expect(glyphStates(container)).toEqual(["error"]);
  });
});

describe("GlyphCounts labelled", () => {
  function renderInKeyBar(counts: Parameters<typeof GlyphCounts>[0]["counts"]) {
    const { container } = render(
      <span className="th-keybar-status">
        <GlyphCounts counts={counts} labelled />
      </span>,
    );
    return container.querySelector(".th-keybar-status")!;
  }

  it("renders one span per state with glyph, number and word, like the key bar", () => {
    const bar = renderInKeyBar({ alive: 6, waiting: 1, error: 1 });
    const items = Array.from(bar.children).filter((el) => !el.classList.contains("th-sr-only"));
    expect(items.map((el) => el.tagName)).toEqual(["SPAN", "SPAN", "SPAN"]);
    expect(items.map((el) => el.firstElementChild?.getAttribute("data-state"))).toEqual(["waiting", "error", "alive"]);
    expect(items.map((el) => visibleText(el))).toEqual(["1 waiting", "1 error", "6 running"]);
  });

  it("calls working working and dead ended", () => {
    const bar = renderInKeyBar({ working: 2, dead: 3 });
    const items = Array.from(bar.children).filter((el) => !el.classList.contains("th-sr-only"));
    expect(items.map((el) => visibleText(el))).toEqual(["2 working", "3 ended"]);
  });

  it("keeps the screen reader text and hides the visible copy", () => {
    const bar = renderInKeyBar({ alive: 6, waiting: 1 });
    expect(bar.querySelector(".th-sr-only")).toHaveTextContent("1 waiting, 6 running");
    Array.from(bar.children)
      .filter((el) => !el.classList.contains("th-sr-only"))
      .forEach((el) => expect(el).toHaveAttribute("aria-hidden", "true"));
  });
});
