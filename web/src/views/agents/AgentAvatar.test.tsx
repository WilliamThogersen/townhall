import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { composeAvatar, defaultAvatar, GRID, type Avatar } from "./avatar";
import { AgentAvatar, AvatarFace } from "./AgentAvatar";

const avatar: Avatar = { head: "square", eyes: "dots", mouth: "flat", accessory: "antenna", color: "ok" };

const ROWS = [
  "#.......#",
  ".#.....#.",
  "..#...#..",
  "...#.#...",
  "....#....",
  "...#.#...",
  "..#...#..",
  ".#.....#.",
  "#.......#",
];

const pathOf = (container: HTMLElement, tone: "body" | "face") =>
  container.querySelector(`path[data-tone="${tone}"]`)?.getAttribute("d") ?? "";
const squares = (d: string) => (d.match(/M/g) ?? []).length;

describe("AvatarFace", () => {
  it("draws one sharp svg square per body and face cell", () => {
    const { container } = render(<AvatarFace avatar={avatar} />);
    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("viewBox", `0 0 ${GRID.width} ${GRID.height}`);
    expect(svg).toHaveAttribute("shape-rendering", "crispEdges");
    const cells = composeAvatar(avatar);
    expect(squares(pathOf(container, "body"))).toBe(cells.filter((c) => c.tone === "body").length);
    expect(squares(pathOf(container, "face"))).toBe(cells.filter((c) => c.tone === "face").length);
  });

  it("draws a literal grid instead of the parts", () => {
    const { container } = render(<AvatarFace avatar={avatar} grid={ROWS} />);
    expect(squares(pathOf(container, "body"))).toBe(17);
    expect(squares(pathOf(container, "face"))).toBe(0);
  });

  it("is decorative unless it has a label", () => {
    const { container, rerender } = render(<AvatarFace avatar={avatar} />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    rerender(<AvatarFace avatar={avatar} label="planner avatar" />);
    expect(container.querySelector("svg")).toHaveAttribute("role", "img");
    expect(container.querySelector("svg")).toHaveAttribute("aria-label", "planner avatar");
    expect(container.querySelector("svg")).not.toHaveAttribute("aria-hidden");
  });

  it("takes its color from the palette through a class and scales by whole cells", () => {
    const { container } = render(<AvatarFace avatar={{ ...avatar, color: "fg-muted" }} scale={3} />);
    const svg = container.querySelector("svg");
    expect(svg).toHaveClass("th-avatar", "th-avatar--fg-muted");
    expect(svg).toHaveAttribute("width", String(GRID.width * 3));
    expect(svg).toHaveAttribute("height", String(GRID.height * 3));
  });
});

describe("AgentAvatar", () => {
  it("shows the name hash default without a spec", () => {
    const { container } = render(<AgentAvatar name="planner" spec={null} />);
    const expected = render(<AvatarFace avatar={defaultAvatar("planner")} />).container;
    expect(pathOf(container, "body")).toBe(pathOf(expected, "body"));
    expect(container.querySelector("svg")).toHaveClass(`th-avatar--${defaultAvatar("planner").color}`);
  });

  it("follows the spec it is given and fills the keys it leaves out from the default", () => {
    const { container } = render(<AgentAvatar name="planner" spec={{ color: "info", eyes: "cyclops" }} />);
    expect(container.querySelector("svg")).toHaveClass("th-avatar--info");
    const expected = render(<AvatarFace avatar={{ ...defaultAvatar("planner"), color: "info", eyes: "cyclops" }} />).container;
    expect(pathOf(container, "body")).toBe(pathOf(expected, "body"));
    expect(pathOf(container, "face")).toBe(pathOf(expected, "face"));
  });

  it("draws the spec's grid", () => {
    const { container } = render(<AgentAvatar name="planner" spec={{ grid: ROWS }} />);
    expect(squares(pathOf(container, "body"))).toBe(17);
  });
});
