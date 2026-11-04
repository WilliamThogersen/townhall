import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Kbd } from "./Kbd";

describe("Kbd", () => {
  it("renders a th-kbd key", () => {
    const { container } = render(<Kbd>esc</Kbd>);
    const key = container.firstElementChild;
    expect(key?.tagName).toBe("KBD");
    expect(key).toHaveClass("th-kbd");
    expect(key).toHaveTextContent("esc");
  });
});
