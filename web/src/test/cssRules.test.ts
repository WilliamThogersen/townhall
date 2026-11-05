import { describe, expect, it } from "vitest";
import { declarationsFor } from "./cssRules";

const CSS = "@font-face { font-family: x; } .a { color: red; } @media (max-width: 640px) { .b { flex-wrap: wrap; } }";

describe("declarationsFor", () => {
  it("finds a rule at the top and inside a media query", () => {
    expect(declarationsFor(CSS, ".a")?.getPropertyValue("color")).toBe("red");
    expect(declarationsFor(CSS, ".b")?.getPropertyValue("flex-wrap")).toBe("wrap");
  });

  it("is null for a selector the css does not have and leaves no style behind", () => {
    expect(declarationsFor(CSS, ".c")).toBeNull();
    expect(document.head.querySelector("style")).toBeNull();
  });
});
