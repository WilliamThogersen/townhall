import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { heatModel } from "./heat";
import { Heatmap } from "./Heatmap";
import { METRICS } from "./metrics";
import { periodDates } from "./shipped";
import { nextIndex } from "./useHeatTip";

const TODAY = new Date(2026, 10, 4);
const dates = periodDates(TODAY, 0);
const values = dates.map((_, i) => (i === dates.length - 1 ? 3 : 0));
const model = heatModel({ dates, values, metric: METRICS[0]!, when: "in the last 12 months" });

function renderMap() {
  const view = render(<Heatmap model={model} label="bug fixes by day" />);
  const map = screen.getByRole("group", { name: "bug fixes by day" });
  const cell = (index: number) => view.container.querySelector(`.th-heat-grid [data-index="${index}"]`)!;
  return { map, cell };
}

const tip = () => screen.queryByRole("status");

describe("nextIndex", () => {
  it("moves a week left and right and a day up and down", () => {
    expect(nextIndex("ArrowLeft", 20, 367)).toBe(13);
    expect(nextIndex("ArrowRight", 20, 367)).toBe(27);
    expect(nextIndex("ArrowUp", 20, 367)).toBe(19);
    expect(nextIndex("ArrowDown", 20, 367)).toBe(21);
  });

  it("stays inside the grid and jumps to the ends", () => {
    expect(nextIndex("ArrowLeft", 3, 367)).toBe(0);
    expect(nextIndex("ArrowRight", 364, 367)).toBe(366);
    expect(nextIndex("Home", 200, 367)).toBe(0);
    expect(nextIndex("End", 2, 367)).toBe(366);
  });

  it("ignores other keys", () => {
    expect(nextIndex("a", 3, 367)).toBeNull();
  });
});

describe("Heatmap", () => {
  it("draws one cell per day with its level, the month and day labels, and the legend", () => {
    const { cell } = renderMap();
    expect(cell(366)).toHaveAttribute("data-level", "4");
    expect(cell(0)).toHaveAttribute("data-level", "0");
    const grid = document.querySelector(".th-heat-grid")!;
    expect(grid).toHaveAttribute("aria-hidden", "true");
    expect(within(grid as HTMLElement).getByText("nov", { selector: ".th-heat-month" })).toBeInTheDocument();
    expect([...grid.querySelectorAll(".th-heat-day")].map((d) => d.textContent)).toEqual(["mon", "wed", "fri"]);
    expect(screen.getByText("3 bug fixes shipped in the last 12 months")).toBeInTheDocument();
    expect(document.querySelectorAll(".th-heat-legend .th-heat-cell")).toHaveLength(5);
  });

  it("shows the day on hover and hides it when the mouse leaves", () => {
    const { map, cell } = renderMap();
    expect(tip()).toBeNull();
    fireEvent.mouseOver(cell(366));
    expect(tip()).toHaveTextContent("3 bug fixes · wed 4 nov");
    expect(cell(366)).toHaveAttribute("data-active", "true");
    fireEvent.mouseOver(cell(0));
    expect(tip()).toHaveTextContent("0 bug fixes · mon 3 nov");
    fireEvent.mouseLeave(map);
    expect(tip()).toBeNull();
  });

  it("ignores hovering the labels", () => {
    const { map } = renderMap();
    fireEvent.mouseOver(within(map).getByText("mon"));
    expect(tip()).toBeNull();
  });

  it("shows today on focus and walks the days with the arrow keys", () => {
    const { map } = renderMap();
    fireEvent.focus(map);
    expect(tip()).toHaveTextContent("3 bug fixes · wed 4 nov");
    fireEvent.keyDown(map, { key: "ArrowUp" });
    expect(tip()).toHaveTextContent("tue 3 nov");
    fireEvent.keyDown(map, { key: "ArrowLeft" });
    expect(tip()).toHaveTextContent("tue 27 oct");
    fireEvent.keyDown(map, { key: "Home" });
    expect(tip()).toHaveTextContent("mon 3 nov");
    fireEvent.keyDown(map, { key: "x" });
    expect(tip()).toHaveTextContent("mon 3 nov");
    fireEvent.blur(map);
    expect(tip()).toBeNull();
  });

  it("keeps the hovered day when a click focuses the map", () => {
    const { map, cell } = renderMap();
    fireEvent.mouseOver(cell(7));
    fireEvent.focus(map);
    expect(tip()).toHaveTextContent("mon 10 nov");
  });

  it("lists the monthly totals in a table for screen readers", () => {
    renderMap();
    const table = screen.getByRole("table", { name: "bug fixes per month" });
    expect(within(table).getByRole("rowheader", { name: "nov 2026" })).toBeInTheDocument();
    expect(within(table).getAllByRole("row")).toHaveLength(13);
  });
});
