import { describe, expect, it } from "vitest";
import { markParts } from "./markParts";

const marked = (text: string, query: string) =>
  markParts(text, query).map((part) => (part.marked ? `[${part.text}]` : part.text)).join("");

describe("markParts", () => {
  it("marks every match ignoring case", () => {
    expect(marked("Checkout fails at checkout", "CHECKOUT")).toBe("[Checkout] fails at [checkout]");
  });

  it("lets a space in the query match any run of whitespace", () => {
    expect(marked("the login  redirect loop", "login redirect")).toBe("the [login  redirect] loop");
  });

  it("treats pattern characters as plain text", () => {
    expect(marked("we hit 100% (twice) of a.b", "100% (twice)")).toBe("we hit [100% (twice)] of a.b");
    expect(marked("axb", "a.b")).toBe("axb");
    expect(marked("fix-login and a/b [x] {y} ^$|\\", "fix-login a/b [x] {y} ^$|\\")).toBe(
      "fix-login and a/b [x] {y} ^$|\\",
    );
    expect(marked("see fix-login, a/b", "fix-login")).toBe("see [fix-login], a/b");
    expect(marked("[x] {y} ^$|\\ end", "[x] {y} ^$|\\")).toBe("[[x] {y} ^$|\\] end");
  });

  it("folds letters beyond ascii", () => {
    expect(marked("en god ÆBLEKAGE", "æblekage")).toBe("en god [ÆBLEKAGE]");
  });

  it("keeps the text whole for a blank query or no match", () => {
    expect(markParts("plain", "  ")).toEqual([{ text: "plain", marked: false }]);
    expect(markParts("plain", "zebra")).toEqual([{ text: "plain", marked: false }]);
  });
});
