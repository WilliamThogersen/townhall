import { render, screen, within } from "@testing-library/react";
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
  it("renders paragraphs and keeps single line breaks", () => {
    const body = renderMarkdown("one\ntwo\n\nthree");
    const paragraphs = body.querySelectorAll("p");
    expect(paragraphs).toHaveLength(2);
    expect(paragraphs[0]?.querySelector("br")).not.toBeNull();
    expect(paragraphs[0]).toHaveTextContent("one two");
    expect(paragraphs[1]).toHaveTextContent("three");
  });

  it("renders inline code, bold, italic, strikethrough and safe links", () => {
    const body = renderMarkdown("**fix** `a.ts`, *now*, ~~old~~, see [pr](https://x.test/1)");
    expect(body.querySelector("strong")).toHaveTextContent("fix");
    expect(body.querySelector("code")).toHaveTextContent("a.ts");
    expect(body.querySelector("em")).toHaveTextContent("now");
    expect(body.querySelector("del")).toHaveTextContent("old");
    const link = screen.getByRole("link", { name: "pr" });
    expect(link).toHaveAttribute("href", "https://x.test/1");
    expect(link).toHaveAttribute("rel", "noreferrer noopener");
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("renders fenced code verbatim with its language", () => {
    const body = renderMarkdown("```ts\nconst a = <b>1</b>;\n```");
    const pre = body.querySelector("pre");
    expect(pre).toHaveAttribute("data-lang", "ts");
    expect(pre?.textContent).toBe("const a = <b>1</b>;\n");
    expect(body.querySelector("b")).toBeNull();
  });

  it("renders bullet and numbered lists, numbered ones from their start", () => {
    const body = renderMarkdown("- a\n- b\n\ntext\n\n2. c\n3. d\n\ntext\n\n1. e");
    expect(body.querySelectorAll("ul > li")).toHaveLength(2);
    const lists = body.querySelectorAll("ol");
    expect(lists[0]).toHaveAttribute("start", "2");
    expect(lists[1]).not.toHaveAttribute("start");
  });

  it("renders nested lists", () => {
    const body = renderMarkdown("- a\n  - a1\n- b");
    expect(body.querySelectorAll("ul ul > li")).toHaveLength(1);
  });

  it("renders tables in a scrollable wrapper", () => {
    const body = renderMarkdown("| Type | Model |\n|---|:---:|\n| `planner` | opus |\n| coder | sonnet |");
    const wrap = body.querySelector(".th-md-table");
    const table = wrap?.querySelector("table");
    expect(table).not.toBeNull();
    const headers = within(table as HTMLElement).getAllByRole("columnheader");
    expect(headers.map((th) => th.textContent)).toEqual(["Type", "Model"]);
    expect(within(table as HTMLElement).getAllByRole("row")).toHaveLength(3);
    expect(table?.querySelector("td code")).toHaveTextContent("planner");
  });

  it("renders headings, quotes and rules", () => {
    const body = renderMarkdown("## Plan\n\n> quoted\n\n---\n\nafter");
    expect(screen.getByRole("heading", { level: 2, name: "Plan" })).toBeInTheDocument();
    expect(body.querySelector("blockquote")).toHaveTextContent("quoted");
    expect(body.querySelector("hr")).not.toBeNull();
  });

  it("renders task lists with read-only checkboxes", () => {
    renderMarkdown("- [x] done\n- [ ] next");
    const boxes = screen.getAllByRole("checkbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0]).toBeChecked();
    expect(boxes[0]).toBeDisabled();
    expect(boxes[1]).not.toBeChecked();
  });

  it("never renders html from the text", () => {
    const body = renderMarkdown('<img src=x onerror="alert(1)"> [x](javascript:alert(1))');
    expect(body.querySelector("img")).toBeNull();
    expect(body.querySelector("a")).toBeNull();
    expect(body).toHaveTextContent('<img src=x onerror="alert(1)">');
  });

  it("allows web and mail links only", () => {
    const body = renderMarkdown("[a](mailto:x@y.test) [b](file:///etc/passwd) [c](data:text/html,x) [d](/rel)");
    const hrefs = [...body.querySelectorAll("a")].map((a) => a.getAttribute("href"));
    expect(hrefs).toEqual(["mailto:x@y.test"]);
    expect(body).toHaveTextContent("a b c d");
  });

  it("skips images", () => {
    const body = renderMarkdown("![logo](https://x.test/a.png)");
    expect(body.querySelector("img")).toBeNull();
  });
});
