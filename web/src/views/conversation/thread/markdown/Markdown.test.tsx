import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Markdown } from "./Markdown";

function renderMarkdown(text: string) {
  return render(
    <div data-testid="body">
      <Markdown text={text} />
    </div>,
  ).getByTestId("body");
}

describe("Markdown", () => {
  it("renders paragraphs with line breaks", () => {
    const body = renderMarkdown("one\ntwo\n\nthree");
    expect(body.innerHTML).toBe("<p>one<br>two</p><p>three</p>");
  });

  it("renders inline code, bold, italic and safe links", () => {
    const body = renderMarkdown("**fix** `a.ts`, *now*, see [pr](https://x.test/1)");
    expect(body.querySelector("strong")).toHaveTextContent("fix");
    expect(body.querySelector("code")).toHaveTextContent("a.ts");
    expect(body.querySelector("em")).toHaveTextContent("now");
    const link = screen.getByRole("link", { name: "pr" });
    expect(link).toHaveAttribute("href", "https://x.test/1");
    expect(link).toHaveAttribute("rel", "noreferrer noopener");
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("renders fenced code verbatim", () => {
    const body = renderMarkdown("```ts\nconst a = <b>1</b>;\n```");
    const pre = body.querySelector("pre");
    expect(pre).toHaveAttribute("data-lang", "ts");
    expect(pre?.textContent).toBe("const a = <b>1</b>;");
    expect(body.querySelector("b")).toBeNull();
  });

  it("renders lists, numbered ones from their start", () => {
    const body = renderMarkdown("- a\n- b\n\n2. c\n3. d\n\n1. e");
    expect(body.querySelectorAll("ul > li")).toHaveLength(2);
    const lists = body.querySelectorAll("ol");
    expect(lists[0]).toHaveAttribute("start", "2");
    expect(lists[1]).not.toHaveAttribute("start");
  });

  it("never renders html from the text", () => {
    const body = renderMarkdown('<img src=x onerror="alert(1)"> [x](javascript:alert(1))');
    expect(body.querySelector("img")).toBeNull();
    expect(body.querySelector("a")).toBeNull();
    expect(body).toHaveTextContent('<img src=x onerror="alert(1)">');
  });
});
