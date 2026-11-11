import { describe, expect, it } from "vitest";
import { declarationsFor } from "../../test/cssRules";
import conversationCss from "../../styles/conversation.css?raw";
import tokensCss from "../../styles/tokens.css?raw";

function token(name: string): number {
  const match = new RegExp(`--th-${name}:\\s*(\\d+)px`).exec(tokensCss);
  if (match?.[1] === undefined) throw new Error(`token --th-${name} is not a px value`);
  return Number(match[1]);
}

describe("conversation css on a narrow window", () => {
  it("lets a needs-you question shrink to its message instead of overflowing it", () => {
    expect(declarationsFor(conversationCss, ".th-ask")?.getPropertyValue("grid-template-columns")).toBe(
      "minmax(0, 1fr)",
    );
  });
});

describe("conversation css with a terminal open", () => {
  it("stacks the terminal under the chat while the two columns would not fit next to the sidebar", () => {
    const sideBySide = token("sidebar") + token("thread-min") + token("pane-min");
    const stacked = new RegExp(`@media \\(max-width: (\\d+)px\\) \\{\\s*\\.th-convo-layout\\[data-pane="true"\\] \\{`).exec(
      conversationCss,
    );
    expect(Number(stacked?.[1]) + 1).toBeGreaterThanOrEqual(sideBySide);
  });
});
