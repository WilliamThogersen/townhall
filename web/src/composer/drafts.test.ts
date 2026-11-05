import { beforeEach, describe, expect, it } from "vitest";
import { clearAllDrafts, clearDraft, readDraft, writeDraft } from "./drafts";

describe("drafts", () => {
  beforeEach(clearAllDrafts);

  it("keeps drafts per conversation and keys home separately", () => {
    writeDraft("a", "one");
    writeDraft(null, "home text");
    expect(readDraft("a")).toBe("one");
    expect(readDraft("b")).toBe("");
    expect(readDraft(null)).toBe("home text");
  });

  it("clears a draft, and writing empty text removes it", () => {
    writeDraft("a", "one");
    clearDraft("a");
    expect(readDraft("a")).toBe("");
    writeDraft("a", "two");
    writeDraft("a", "");
    expect(readDraft("a")).toBe("");
  });
});
