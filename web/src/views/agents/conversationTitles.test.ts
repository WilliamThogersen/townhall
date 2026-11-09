import { describe, expect, it } from "vitest";
import { conversationLabel } from "./conversationTitles";

describe("conversationLabel", () => {
  const titles = new Map([["c13946872ad1", "usage meter"]]);

  it("uses the conversation title when it is known", () => {
    expect(conversationLabel("c13946872ad1", titles)).toBe("usage meter");
  });

  it("falls back to the id when the title is unknown or blank", () => {
    expect(conversationLabel("29a26c3cc2c1", titles)).toBe("29a26c3cc2c1");
    expect(conversationLabel("x", new Map([["x", "  "]]))).toBe("x");
  });
});
