import { describe, expect, it } from "vitest";
import { isInTerminal } from "../../shell/globalKeys";
import { CLOSE_TERMINAL_HINT } from "./session";
import { conversationHints, TERMINAL_FOCUSED_HINT } from "./useConversationKeys";

describe("terminal key hints", () => {
  it("offers esc to close only while the terminal is not focused", () => {
    expect(conversationHints(false, true, false)).toContain(CLOSE_TERMINAL_HINT);
    expect(conversationHints(false, true, true)).not.toContain(CLOSE_TERMINAL_HINT);
    expect(conversationHints(false, true, true)).toContain(TERMINAL_FOCUSED_HINT);
    expect(conversationHints(false, false, true)).not.toContain(TERMINAL_FOCUSED_HINT);
  });

  it("knows an event target inside the live terminal", () => {
    const host = document.createElement("div");
    host.className = "th-term";
    const inner = document.createElement("textarea");
    host.append(inner);
    expect(isInTerminal(inner)).toBe(true);
    expect(isInTerminal(document.body)).toBe(false);
    expect(isInTerminal(null)).toBe(false);
  });
});
