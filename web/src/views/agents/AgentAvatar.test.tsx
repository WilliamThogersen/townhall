import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { composeAvatar, defaultAvatar, GRID, type Avatar } from "./avatar";
import { AgentAvatar, AvatarFace } from "./AgentAvatar";
import { saveAvatar } from "./avatarPrefs";

vi.mock("../../log/report", () => ({ logWarn: vi.fn() }));

const avatar: Avatar = { head: "square", eyes: "dots", mouth: "flat", accessory: "antenna", color: "ok" };

afterEach(() => localStorage.clear());

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
  it("shows the hash default and follows a saved choice", () => {
    const first = defaultAvatar("planner");
    const { container } = render(<AgentAvatar name="planner" />);
    expect(pathOf(container, "body")).toBe(render(<AvatarFace avatar={first} />).container.querySelector('path[data-tone="body"]')?.getAttribute("d"));
    const before = pathOf(container, "body");
    act(() => {
      saveAvatar("planner", { head: "tapered", eyes: "cyclops", mouth: "open", accessory: "crest", color: "info" });
    });
    expect(pathOf(container, "body")).not.toBe(before);
    expect(container.querySelector("svg")).toHaveClass("th-avatar--info");
  });
});
