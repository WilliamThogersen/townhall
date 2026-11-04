import { fireEvent, render, screen } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { supportsFieldSizing, useAutoGrow } from "./useAutoGrow";

function Area() {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState("");
  useAutoGrow(ref, value);
  return <textarea aria-label="grow" ref={ref} value={value} onChange={(event) => setValue(event.target.value)} />;
}

afterEach(() => vi.unstubAllGlobals());

describe("useAutoGrow", () => {
  it("sizes the box to its content where field-sizing is missing", () => {
    vi.stubGlobal("CSS", { supports: () => false });
    render(<Area />);
    const area = screen.getByRole("textbox");
    Object.defineProperty(area, "scrollHeight", { configurable: true, value: 64 });
    fireEvent.change(area, { target: { value: "one\ntwo\nthree" } });
    expect(area.style.height).toBe("64px");
  });

  it("leaves sizing to the browser where field-sizing works", () => {
    vi.stubGlobal("CSS", { supports: (property: string, value: string) => property === "field-sizing" && value === "content" });
    render(<Area />);
    const area = screen.getByRole("textbox");
    Object.defineProperty(area, "scrollHeight", { configurable: true, value: 64 });
    fireEvent.change(area, { target: { value: "one\ntwo" } });
    expect(area.style.height).toBe("");
  });

  it("treats a missing CSS object as no field-sizing", () => {
    vi.stubGlobal("CSS", undefined);
    expect(supportsFieldSizing()).toBe(false);
  });
});
