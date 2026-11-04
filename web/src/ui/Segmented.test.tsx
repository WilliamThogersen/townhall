import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Segmented } from "./Segmented";

type Mode = "ask" | "edits" | "skip";

const OPTIONS = [
  { value: "ask", label: "ask" },
  { value: "edits", label: "accept edits" },
  { value: "skip", label: "skip all", tone: "danger" },
] as const satisfies readonly { value: Mode; label: string; tone?: "danger" }[];

function renderModes(value: Mode = "edits", onChange = vi.fn()) {
  render(<Segmented<Mode> name="perm" label="Permissions" value={value} options={OPTIONS} onChange={onChange} />);
  return onChange;
}

describe("Segmented", () => {
  it("renders a labelled radio group with one radio per option", () => {
    renderModes();
    const group = screen.getByRole("radiogroup", { name: "Permissions" });
    expect(group).toHaveClass("th-segmented");
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(3);
    radios.forEach((radio) => expect(radio).toHaveAttribute("name", "perm"));
    expect(radios.map((radio) => (radio as HTMLInputElement).value)).toEqual(["ask", "edits", "skip"]);
  });

  it("checks the option that matches value", () => {
    renderModes("edits");
    expect(screen.getByRole("radio", { name: "accept edits" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "ask" })).not.toBeChecked();
  });

  it("follows a new value", () => {
    const { rerender } = render(
      <Segmented<Mode> name="perm" label="Permissions" value="ask" options={OPTIONS} onChange={vi.fn()} />,
    );
    rerender(<Segmented<Mode> name="perm" label="Permissions" value="skip" options={OPTIONS} onChange={vi.fn()} />);
    expect(screen.getByRole("radio", { name: "skip all" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "ask" })).not.toBeChecked();
  });

  it("calls onChange with the clicked value", async () => {
    const onChange = renderModes("edits");
    await userEvent.click(screen.getByText("ask"));
    expect(onChange).toHaveBeenCalledWith("ask");
  });

  it("moves the choice with arrow keys", async () => {
    const onChange = renderModes("edits");
    await userEvent.click(screen.getByText("accept edits"));
    await userEvent.keyboard("{ArrowRight}");
    expect(onChange).toHaveBeenLastCalledWith("skip");
  });

  it("marks only the dangerous choice with data-tone", () => {
    renderModes();
    const label = (name: string) => screen.getByRole("radio", { name }).closest("label");
    expect(label("skip all")).toHaveAttribute("data-tone", "danger");
    expect(label("ask")).not.toHaveAttribute("data-tone");
    expect(label("accept edits")).not.toHaveAttribute("data-tone");
  });

  it("puts each label text in a span after its input", () => {
    renderModes();
    const input = screen.getByRole("radio", { name: "ask" });
    expect(input.nextElementSibling?.tagName).toBe("SPAN");
    expect(input.nextElementSibling).toHaveTextContent("ask");
  });
});
