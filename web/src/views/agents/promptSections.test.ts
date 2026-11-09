import { describe, expect, it } from "vitest";
import { parseBlocks, parsePrompt, promptStats } from "./promptSections";

describe("parsePrompt", () => {
  it("returns nothing for an empty or blank prompt", () => {
    expect(parsePrompt("")).toEqual({ intro: "", sections: [] });
    expect(parsePrompt("  \n\n ")).toEqual({ intro: "", sections: [] });
  });

  it("keeps a prompt without tags as the intro", () => {
    expect(parsePrompt("You implement the task.\nBe brief.\n")).toEqual({
      intro: "You implement the task.\nBe brief.",
      sections: [],
    });
  });

  it("splits tagged sections and keeps the text before the first tag as intro", () => {
    const parsed = parsePrompt("You are the BUILD step.\n\n<role>\nImplement.\n</role>\n\n<process>\n1. Read\n2. Write\n</process>\n");
    expect(parsed.intro).toBe("You are the BUILD step.");
    expect(parsed.sections).toEqual([
      { tag: "role", body: "Implement.", closed: true },
      { tag: "process", body: "1. Read\n2. Write", closed: true },
    ]);
  });

  it("reads a one-line section", () => {
    expect(parsePrompt("<role>Implement.</role>").sections).toEqual([{ tag: "role", body: "Implement.", closed: true }]);
  });

  it("keeps nested tags inside the body of their outer section", () => {
    const parsed = parsePrompt("<process>\n<step>\nread\n</step>\nthen write\n</process>\n<constraints>\nnone\n</constraints>");
    expect(parsed.sections.map((s) => s.tag)).toEqual(["process", "constraints"]);
    expect(parsed.sections[0]?.body).toBe("<step>\nread\n</step>\nthen write");
  });

  it("runs an unclosed section to the end and flags it", () => {
    expect(parsePrompt("<role>\nImplement.\nMore.").sections).toEqual([
      { tag: "role", body: "Implement.\nMore.", closed: false },
    ]);
  });

  it("lets an outer close tag end an unclosed nested tag", () => {
    const parsed = parsePrompt("<a>\n<b>\ntext\n</a>\n<c>\nx\n</c>");
    expect(parsed.sections.map((s) => [s.tag, s.closed])).toEqual([
      ["a", true],
      ["c", true],
    ]);
    expect(parsed.sections[0]?.body).toBe("<b>\ntext");
  });

  it("treats a stray close tag and mid-line tags as plain text", () => {
    const parsed = parsePrompt("use Array<string> here\n</oops>\n<role>\nok\n</role>");
    expect(parsed.intro).toBe("use Array<string> here\n</oops>");
    expect(parsed.sections).toHaveLength(1);
  });

  it("puts loose text between sections into an untagged section", () => {
    const parsed = parsePrompt("<a>\nx\n</a>\nloose\n<b>\ny\n</b>");
    expect(parsed.sections.map((s) => s.tag)).toEqual(["a", "", "b"]);
    expect(parsed.sections[1]?.body).toBe("loose");
  });
});

describe("parseBlocks", () => {
  it("returns no blocks for empty text", () => {
    expect(parseBlocks("")).toEqual([]);
  });

  it("makes paragraphs split on blank lines and keeps single line breaks", () => {
    expect(parseBlocks("one\ntwo\n\nthree")).toEqual([
      { kind: "paragraph", text: "one\ntwo" },
      { kind: "paragraph", text: "three" },
    ]);
  });

  it("reads numbered lists", () => {
    expect(parseBlocks("1. Read\n2) Write")).toEqual([{ kind: "list", ordered: true, items: ["Read", "Write"] }]);
  });

  it("reads dash and star lists and joins indented continuation lines", () => {
    expect(parseBlocks("- Stay inside\n  the task\n* Never skip")).toEqual([
      { kind: "list", ordered: false, items: ["Stay inside the task", "Never skip"] },
    ]);
  });

  it("separates a list from the paragraph around it", () => {
    expect(parseBlocks("Do this:\n- a\n- b\nThen rest.")).toEqual([
      { kind: "paragraph", text: "Do this:" },
      { kind: "list", ordered: false, items: ["a", "b"] },
      { kind: "paragraph", text: "Then rest." },
    ]);
  });

  it("does not treat a dash without a space as a list", () => {
    expect(parseBlocks("-no")).toEqual([{ kind: "paragraph", text: "-no" }]);
  });
});

describe("promptStats", () => {
  it("counts characters, about one token per four characters, and sections", () => {
    const text = "abcd".repeat(10) + "\n<a>\nx\n</a>";
    expect(promptStats(text)).toEqual({ chars: text.length, tokens: Math.ceil(text.length / 4), sections: 1 });
  });

  it("is zero for an empty prompt", () => {
    expect(promptStats("")).toEqual({ chars: 0, tokens: 0, sections: 0 });
  });
});
