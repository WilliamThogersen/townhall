import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { isTextField, useGlobalKeys } from "./globalKeys";

function Keys() {
  useGlobalKeys();
  return (
    <div>
      <input aria-label="text" />
      <input aria-label="check" type="checkbox" />
      <textarea aria-label="area" />
      <select aria-label="pick"><option>a</option></select>
      <div aria-label="editable" contentEditable suppressContentEditableWarning>
        <span>inside</span>
      </div>
      <button type="button">plain</button>
    </div>
  );
}

beforeEach(() => window.history.replaceState(null, "", "/townhall"));

describe("global keys", () => {
  it("n goes home", () => {
    render(<Keys />);
    expect(fireEvent.keyDown(document.body, { key: "n" })).toBe(false);
    expect(window.location.pathname).toBe("/");
  });

  it("/ goes to search, also when the layout needs shift for it", () => {
    render(<Keys />);
    fireEvent.keyDown(screen.getByRole("button", { name: "plain" }), { key: "/", shiftKey: true });
    expect(window.location.pathname).toBe("/search");
  });

  it.each(["text", "area", "pick", "editable"])("does nothing while focus is in %s", (label) => {
    render(<Keys />);
    expect(fireEvent.keyDown(screen.getByLabelText(label), { key: "n" })).toBe(true);
    expect(window.location.pathname).toBe("/townhall");
  });

  it("still works from a checkbox", () => {
    render(<Keys />);
    fireEvent.keyDown(screen.getByLabelText("check"), { key: "n" });
    expect(window.location.pathname).toBe("/");
  });

  it.each([
    ["meta", { metaKey: true }],
    ["ctrl", { ctrlKey: true }],
    ["alt", { altKey: true }],
  ])("ignores n with %s held", (_, init) => {
    render(<Keys />);
    fireEvent.keyDown(document.body, { key: "n", ...init });
    expect(window.location.pathname).toBe("/townhall");
  });

  it("ignores other keys and handled events", () => {
    render(<Keys />);
    fireEvent.keyDown(document.body, { key: "N", shiftKey: true });
    fireEvent.keyDown(document.body, { key: "x" });
    const handled = new KeyboardEvent("keydown", { key: "n", bubbles: true, cancelable: true });
    handled.preventDefault();
    document.body.dispatchEvent(handled);
    expect(window.location.pathname).toBe("/townhall");
  });

  it("stops listening on unmount", () => {
    const { unmount } = render(<Keys />);
    unmount();
    fireEvent.keyDown(document.body, { key: "n" });
    expect(window.location.pathname).toBe("/townhall");
  });
});

describe("isTextField", () => {
  it("treats null and plain elements as not text", () => {
    expect(isTextField(null)).toBe(false);
    expect(isTextField(window)).toBe(false);
    expect(isTextField(document.createElement("div"))).toBe(false);
  });
});
