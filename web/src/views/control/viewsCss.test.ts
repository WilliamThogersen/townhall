import { describe, expect, it } from "vitest";
import { declarationsFor } from "../../test/cssRules";
import viewsCss from "../../styles/views.css?raw";

describe("trace css on a one column window", () => {
  it("drops the fixed trace width so the page never scrolls sideways", () => {
    expect(declarationsFor(viewsCss, ".th-trace-body .th-trace-rows")?.getPropertyValue("min-width")).toBe("0px");
    expect(declarationsFor(viewsCss, ".th-trace-body .th-span")?.getPropertyValue("grid-template-columns")).toBe(
      "var(--th-trace-label) minmax(0, 1fr)",
    );
  });
});
