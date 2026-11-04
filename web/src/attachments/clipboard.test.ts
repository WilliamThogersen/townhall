import { describe, expect, it } from "vitest";
import { clipboardFiles, hasClipboardText } from "./clipboard";

const shot = new File(["x"], "image.png", { type: "image/png" });

function transfer(fields: { files?: readonly File[]; items?: readonly unknown[]; types?: readonly string[] }) {
  return {
    files: fields.files ?? [],
    items: fields.items ?? [],
    types: fields.types ?? [],
  } as unknown as DataTransfer;
}

describe("clipboardFiles", () => {
  it("takes the pasted files", () => {
    expect(clipboardFiles(transfer({ files: [shot] }))).toEqual([shot]);
  });

  it("falls back to file items when the file list is empty", () => {
    const items = [
      { kind: "string", getAsFile: () => null },
      { kind: "file", getAsFile: () => shot },
      { kind: "file", getAsFile: () => null },
    ];
    expect(clipboardFiles(transfer({ items }))).toEqual([shot]);
  });

  it("has nothing for a missing clipboard", () => {
    expect(clipboardFiles(null)).toEqual([]);
  });
});

describe("hasClipboardText", () => {
  it("knows when plain text came along", () => {
    expect(hasClipboardText(transfer({ types: ["Files", "text/plain"] }))).toBe(true);
    expect(hasClipboardText(transfer({ types: ["Files"] }))).toBe(false);
    expect(hasClipboardText(null)).toBe(false);
  });
});
