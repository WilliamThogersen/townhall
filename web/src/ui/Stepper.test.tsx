import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Stepper } from "./Stepper";

function renderStepper(value: number, onChange = vi.fn()) {
  const view = render(<Stepper value={value} min={1} max={50} onChange={onChange} label="Agents at once" />);
  return { onChange, ...view };
}

describe("Stepper", () => {
  it("renders a labelled group with the value in an output", () => {
    renderStepper(6);
    const group = screen.getByRole("group", { name: "Agents at once" });
    expect(group).toHaveClass("th-stepper");
    const output = group.querySelector("output");
    expect(output).toHaveTextContent("6");
    expect(output).toHaveAttribute("aria-live", "polite");
  });

  it("orders fewer, value, more with minus and plus glyphs", () => {
    renderStepper(6);
    const group = screen.getByRole("group");
    const children = Array.from(group.children);
    expect(children.map((child) => child.tagName)).toEqual(["BUTTON", "OUTPUT", "BUTTON"]);
    expect(screen.getByRole("button", { name: "Fewer" })).toHaveTextContent("−");
    expect(screen.getByRole("button", { name: "More" })).toHaveTextContent("+");
    screen.getAllByRole("button").forEach((button) => expect(button).toHaveAttribute("type", "button"));
  });

  it("calls onChange with one less and one more", async () => {
    const { onChange } = renderStepper(6);
    await userEvent.click(screen.getByRole("button", { name: "Fewer" }));
    expect(onChange).toHaveBeenLastCalledWith(5);
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    expect(onChange).toHaveBeenLastCalledWith(7);
  });

  it("disables fewer at min", () => {
    renderStepper(1);
    expect(screen.getByRole("button", { name: "Fewer" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "More" })).toBeEnabled();
  });

  it("disables more at max", () => {
    renderStepper(50);
    expect(screen.getByRole("button", { name: "More" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Fewer" })).toBeEnabled();
  });

  it("enables both in between", () => {
    renderStepper(25);
    screen.getAllByRole("button").forEach((button) => expect(button).toBeEnabled());
  });

  it("moves focus to the other button when a step reaches a limit", async () => {
    const { onChange, rerender } = renderStepper(49);
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    expect(onChange).toHaveBeenLastCalledWith(50);
    expect(screen.getByRole("button", { name: "Fewer" })).toHaveFocus();
    rerender(<Stepper value={2} min={1} max={50} onChange={onChange} label="Agents at once" />);
    await userEvent.click(screen.getByRole("button", { name: "Fewer" }));
    expect(onChange).toHaveBeenLastCalledWith(1);
    expect(screen.getByRole("button", { name: "More" })).toHaveFocus();
  });

  it("keeps focus on the button while a step stays inside the limits", async () => {
    renderStepper(6);
    const more = screen.getByRole("button", { name: "More" });
    await userEvent.click(more);
    expect(more).toHaveFocus();
  });

  it("clamps a value outside the limits", async () => {
    const { onChange } = renderStepper(80);
    await userEvent.click(screen.getByRole("button", { name: "Fewer" }));
    expect(onChange).toHaveBeenLastCalledWith(50);
  });
});
