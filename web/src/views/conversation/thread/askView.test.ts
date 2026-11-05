import { describe, expect, it } from "vitest";
import { answerText, askView } from "./askView";

const DIALOG = [
  "Bash: cd /Users/w/.townhall/worktrees/townhall/agent-library-view/web/…",
  "Do you want to proceed?",
  "❯ 1. Yes",
  "2. Yes, and don’t ask again for: cd *",
  "3. Yes, and switch to auto mode · auto mode handles these prompts for you",
  "4. No",
].join("\n");

describe("askView", () => {
  it("splits a permission dialog into what it runs, the question and its options", () => {
    const view = askView(DIALOG, ["1", "2", "3", "4"]);
    expect(view.context).toEqual(["Bash: cd /Users/w/.townhall/worktrees/townhall/agent-library-view/web/…"]);
    expect(view.prompt).toBe("Do you want to proceed?");
    expect(view.options.map((o) => [o.number, o.label, o.hint])).toEqual([
      ["1", "Yes", null],
      ["2", "Yes, and don’t ask again for: cd *", null],
      ["3", "Yes, and switch to auto mode", "auto mode handles these prompts for you"],
      ["4", "No", null],
    ]);
  });

  it("keeps a y/n question whole, with no options", () => {
    const view = askView("commit the fix to townhall/fix-login? (y/n)", ["y", "n"]);
    expect(view).toEqual({ context: [], prompt: "commit the fix to townhall/fix-login? (y/n)", options: [] });
  });

  it("only takes numbered lines that are real choices", () => {
    const view = askView("Steps so far:\n1. read the code\nWhich db?\n1) postgres\n2) sqlite", ["1", "2"]);
    expect(view.prompt).toBe("Which db?");
    expect(view.options.map((o) => o.label)).toEqual(["postgres", "sqlite"]);
    expect(view.context).toEqual(["Steps so far:", "1. read the code"]);
  });

  it("uses the last line as the question when none ends in a question mark", () => {
    expect(askView("waiting for input\npress enter", []).prompt).toBe("press enter");
  });
});

describe("answerText", () => {
  const view = askView(DIALOG, ["1", "2", "3", "4"]);

  it("names the option an answer picked", () => {
    expect(answerText(view, "3")).toBe("3 · Yes, and switch to auto mode");
    expect(answerText(view, " 4 ")).toBe("4 · No");
  });

  it("keeps any other answer as it was sent", () => {
    expect(answerText(view, "commit as wip")).toBe("commit as wip");
    expect(answerText(askView("ok? (y/n)", ["y", "n"]), "y")).toBe("y");
  });
});
