import { describe, expect, it } from "vitest";
import { declarationsFor } from "../../test/cssRules";
import controlCss from "./control.css?raw";

describe("control center css", () => {
  it("lets the loop strip wrap so needs you stays in view on a narrow window", () => {
    const loop = declarationsFor(controlCss, ".th-control .th-loop");
    expect(loop?.getPropertyValue("flex-wrap")).toBe("wrap");
    expect(loop?.getPropertyValue("row-gap")).toMatch(/^var\(--th-/);
  });
});
