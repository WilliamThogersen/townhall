import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Switch } from "./Switch";

describe("Switch", () => {
  it("renders a labelled switch button", () => {
    render(<Switch checked={false} onChange={vi.fn()} label="Work without asking" />);
    const sw = screen.getByRole("switch", { name: "Work without asking" });
    expect(sw.tagName).toBe("BUTTON");
    expect(sw).toHaveAttribute("type", "button");
    expect(sw).toHaveClass("th-switch");
  });

  it("reflects checked in aria-checked", () => {
    const { rerender } = render(<Switch checked={false} onChange={vi.fn()} label="Session panel" />);
    expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "false");
    rerender(<Switch checked onChange={vi.fn()} label="Session panel" />);
    expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "true");
  });

  it("calls onChange with the opposite value on click", async () => {
    const onChange = vi.fn();
    const { rerender } = render(<Switch checked={false} onChange={onChange} label="Session panel" />);
    await userEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenLastCalledWith(true);
    rerender(<Switch checked onChange={onChange} label="Session panel" />);
    await userEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it("toggles with space and enter", async () => {
    const onChange = vi.fn();
    render(<Switch checked={false} onChange={onChange} label="Session panel" />);
    screen.getByRole("switch").focus();
    await userEvent.keyboard(" ");
    await userEvent.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenNthCalledWith(1, true);
    expect(onChange).toHaveBeenNthCalledWith(2, true);
  });
});
