import { describe, expect, it } from "vitest";
import { parseBlocks, parseInline, safeHref } from "./parseMarkdown";

describe("parseInline", () => {
  it("keeps plain text as one text run", () => {
    expect(parseInline("just words")).toEqual([{ kind: "text", text: "just words" }]);
  });

  it("reads inline code literally", () => {
    expect(parseInline("see `a **b**` here")).toEqual([
      { kind: "text", text: "see " },
      { kind: "code", text: "a **b**" },
      { kind: "text", text: " here" },
    ]);
  });

  it("reads bold and italic with both markers", () => {
    expect(parseInline("**bold** and __also__ then *it* and _too_")).toEqual([
      { kind: "strong", children: [{ kind: "text", text: "bold" }] },
      { kind: "text", text: " and " },
      { kind: "strong", children: [{ kind: "text", text: "also" }] },
      { kind: "text", text: " then " },
      { kind: "em", children: [{ kind: "text", text: "it" }] },
      { kind: "text", text: " and " },
      { kind: "em", children: [{ kind: "text", text: "too" }] },
    ]);
  });

  it("nests inline parts inside bold", () => {
    expect(parseInline("**fix `a.ts` now**")).toEqual([
      {
        kind: "strong",
        children: [
          { kind: "text", text: "fix " },
          { kind: "code", text: "a.ts" },
          { kind: "text", text: " now" },
        ],
      },
    ]);
  });

  it("leaves underscores inside words alone", () => {
    expect(parseInline("snake_case_name stays")).toEqual([{ kind: "text", text: "snake_case_name stays" }]);
  });

  it("leaves a lone star alone", () => {
    expect(parseInline("2 * 3 = 6")).toEqual([{ kind: "text", text: "2 * 3 = 6" }]);
  });

  it("reads links with their text", () => {
    expect(parseInline("open [the pr](https://x.test/pr/1) now")).toEqual([
      { kind: "text", text: "open " },
      { kind: "link", href: "https://x.test/pr/1", children: [{ kind: "text", text: "the pr" }] },
      { kind: "text", text: " now" },
    ]);
  });

  it("turns an unsafe link into its text", () => {
    expect(parseInline("[click](javascript:alert(1))")).toEqual([
      { kind: "text", text: "click" },
      { kind: "text", text: ")" },
    ]);
  });
});

describe("safeHref", () => {
  it("allows web and mail links only", () => {
    expect(safeHref("https://a.test")).toBe("https://a.test");
    expect(safeHref("http://a.test")).toBe("http://a.test");
    expect(safeHref("mailto:a@b.test")).toBe("mailto:a@b.test");
    expect(safeHref("javascript:alert(1)")).toBeNull();
    expect(safeHref("JavaScript:alert(1)")).toBeNull();
    expect(safeHref("data:text/html,x")).toBeNull();
    expect(safeHref("/relative")).toBeNull();
  });
});

describe("parseBlocks", () => {
  it("splits paragraphs on blank lines and keeps line breaks", () => {
    expect(parseBlocks("one\ntwo\n\nthree")).toEqual([
      { kind: "paragraph", lines: [[{ kind: "text", text: "one" }], [{ kind: "text", text: "two" }]] },
      { kind: "paragraph", lines: [[{ kind: "text", text: "three" }]] },
    ]);
  });

  it("reads fenced code verbatim with its language", () => {
    expect(parseBlocks("before\n```ts\nconst a = **1**;\n\n  b();\n```\nafter")).toEqual([
      { kind: "paragraph", lines: [[{ kind: "text", text: "before" }]] },
      { kind: "code", lang: "ts", text: "const a = **1**;\n\n  b();" },
      { kind: "paragraph", lines: [[{ kind: "text", text: "after" }]] },
    ]);
  });

  it("closes an unclosed fence at the end", () => {
    expect(parseBlocks("```\nopen")).toEqual([{ kind: "code", lang: "", text: "open" }]);
  });

  it("reads bullet lists", () => {
    expect(parseBlocks("- one\n* two\n+ three")).toEqual([
      {
        kind: "list",
        ordered: false,
        start: 1,
        items: [
          [[{ kind: "text", text: "one" }]],
          [[{ kind: "text", text: "two" }]],
          [[{ kind: "text", text: "three" }]],
        ],
      },
    ]);
  });

  it("reads numbered lists with their start and continuation lines", () => {
    expect(parseBlocks("3. first\n   still first\n4) second")).toEqual([
      {
        kind: "list",
        ordered: true,
        start: 3,
        items: [
          [[{ kind: "text", text: "first" }], [{ kind: "text", text: "still first" }]],
          [[{ kind: "text", text: "second" }]],
        ],
      },
    ]);
  });

  it("starts a new list when the kind changes", () => {
    const blocks = parseBlocks("- a\n1. b");
    expect(blocks.map((b) => (b.kind === "list" ? b.ordered : null))).toEqual([false, true]);
  });

  it("ends a list at an unindented line", () => {
    expect(parseBlocks("- a\nplain").map((b) => b.kind)).toEqual(["list", "paragraph"]);
  });

  it("shows headings as bold lines", () => {
    expect(parseBlocks("## plan")).toEqual([
      { kind: "paragraph", lines: [[{ kind: "strong", children: [{ kind: "text", text: "plan" }] }]] },
    ]);
  });

  it("gives nothing for blank text", () => {
    expect(parseBlocks("  \n\n")).toEqual([]);
  });
});
