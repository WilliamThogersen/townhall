import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Logo } from "./Logo";

function renderLogo(props: Parameters<typeof Logo>[0] = {}) {
  const { container } = render(<Logo {...props} />);
  const svg = container.querySelector("svg");
  if (!svg) throw new Error("logo did not render an svg");
  return svg;
}

describe("Logo", () => {
  it("renders a decorative pixel svg in the current color", () => {
    const svg = renderLogo();
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("viewBox", "0 0 18 14");
    expect(svg).toHaveAttribute("fill", "currentColor");
    expect(svg).toHaveAttribute("shape-rendering", "crispEdges");
    expect(svg).toHaveAttribute("class", "th-logo");
    expect(svg.querySelector("path")?.getAttribute("d")).toMatch(/^M8 0h2v1h-2z/);
  });

  it("is 18 by 14 by default", () => {
    const svg = renderLogo();
    expect(svg).toHaveAttribute("width", "18");
    expect(svg).toHaveAttribute("height", "14");
  });

  it("keeps the 18 by 14 ratio at a custom width", () => {
    const svg = renderLogo({ width: 108 });
    expect(svg).toHaveAttribute("width", "108");
    expect(svg).toHaveAttribute("height", "84");
  });

  it("adds an extra class after th-logo", () => {
    const svg = renderLogo({ className: "th-greeting-logo" });
    expect(svg).toHaveAttribute("class", "th-logo th-greeting-logo");
  });
});
