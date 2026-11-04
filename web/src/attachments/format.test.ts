import { describe, expect, it } from "vitest";
import { extensionTag, fileExtension, formatClock, formatSize } from "./format";

describe("formatSize", () => {
  it.each([
    [0, "0 B"],
    [512, "512 B"],
    [4096, "4 KB"],
    [4300, "4.2 KB"],
    [49152, "48 KB"],
    [186368, "182 KB"],
    [1153434, "1.1 MB"],
    [20 * 1024 * 1024, "20 MB"],
  ])("%d bytes reads %s", (bytes, shown) => {
    expect(formatSize(bytes)).toBe(shown);
  });
});

describe("fileExtension", () => {
  it.each([
    ["localhost.har", "har"],
    ["Schema.SQL", "sql"],
    ["archive.tar.gz", "gz"],
    ["Makefile", ""],
    [".env", ""],
    ["trailing.", ""],
  ])("%s has extension %j", (name, ext) => {
    expect(fileExtension(name)).toBe(ext);
  });
});

describe("extensionTag", () => {
  it("shows at most four letters of the extension", () => {
    expect(extensionTag("auth-flow.pdf")).toBe("pdf");
    expect(extensionTag("notes.markdown")).toBe("mark");
  });

  it("says file when there is no extension", () => {
    expect(extensionTag("Makefile")).toBe("file");
  });
});

describe("formatClock", () => {
  it("shows the local hour and minute", () => {
    expect(formatClock(new Date(2026, 10, 4, 9, 5).toISOString())).toBe("09:05");
    expect(formatClock(new Date(2026, 10, 4, 22, 42).toISOString())).toBe("22:42");
  });

  it("shows nothing for a date it cannot read", () => {
    expect(formatClock("yesterday")).toBe("");
  });
});
