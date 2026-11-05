import { describe, expect, it } from "vitest";
import { stripAttachmentMarkers } from "./markers";

const names = ["shot.png", "notes.txt"];

describe("stripAttachmentMarkers", () => {
  it("leaves text alone when there is nothing attached", () => {
    expect(stripAttachmentMarkers("see [Image #1]", [])).toBe("see [Image #1]");
  });

  it.each([
    ["look [Image #1] here", "look here"],
    ["[Image #1]\n[Image #2]\nwhat is this?", "what is this?"],
    ["[image 3] hi", "hi"],
    ["[Image: shot.png] hi", "hi"],
    ["[Pasted image] hi", "hi"],
  ])("removes the marker in %j", (text, expected) => {
    expect(stripAttachmentMarkers(text, names)).toBe(expected);
  });

  it("removes a line that is only an attached file's name or path", () => {
    expect(stripAttachmentMarkers("/tmp/x/shot.png\nwhy red?", names)).toBe("why red?");
    expect(stripAttachmentMarkers("shot.png", names)).toBe("");
  });

  it("keeps a line that merely mentions a file", () => {
    expect(stripAttachmentMarkers("is shot.png the right one?", names)).toBe("is shot.png the right one?");
  });

  it("keeps paragraph breaks between kept paragraphs", () => {
    expect(stripAttachmentMarkers("a\n\n[Image #1]\n\nb", names)).toBe("a\n\nb");
  });
});
