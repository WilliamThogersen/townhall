import { describe, expect, it } from "vitest";
import { MAX_ATTACHMENT_BYTES } from "../api/attachments";
import { ACCEPTED_FILES, isImageFile, rejectReason } from "./validate";

function fileOf(name: string, type: string, size = 10): File {
  const file = new File(["x"], name, { type });
  Object.defineProperty(file, "size", { value: size });
  return file;
}

describe("isImageFile", () => {
  it.each(["image/png", "image/jpeg", "image/gif", "image/webp"])("treats %s as an image", (type) => {
    expect(isImageFile(fileOf("a", type))).toBe(true);
  });

  it.each(["image/svg+xml", "image/heic", "text/plain", ""])("treats %j as a file", (type) => {
    expect(isImageFile(fileOf("a", type))).toBe(false);
  });
});

describe("rejectReason", () => {
  it.each([
    ["screenshot.png", "image/png"],
    ["notes.txt", "text/plain"],
    ["auth-flow.pdf", "application/pdf"],
    ["package.json", "application/json"],
    ["query.ts", "video/mp2t"],
    ["main.rs", ""],
    ["localhost.har", ""],
    ["Dockerfile", ""],
    ["Makefile", "application/octet-stream"],
  ])("accepts %s (%j)", (name, type) => {
    expect(rejectReason(fileOf(name, type))).toBeNull();
  });

  it.each([
    ["movie.mp4", "video/mp4"],
    ["photo.heic", "image/heic"],
    ["installer.dmg", "application/x-apple-diskimage"],
    ["mystery", ""],
  ])("refuses %s as not an image, text, code or pdf", (name, type) => {
    expect(rejectReason(fileOf(name, type))).toBe(`${name} is not an image, text, code or pdf`);
  });

  it("refuses a file over 20 MB before its type", () => {
    expect(rejectReason(fileOf("screenshot-3.png", "image/png", MAX_ATTACHMENT_BYTES + 1))).toBe(
      "screenshot-3.png is over 20 MB",
    );
    expect(rejectReason(fileOf("exact.png", "image/png", MAX_ATTACHMENT_BYTES))).toBeNull();
  });

  it("refuses an empty file", () => {
    expect(rejectReason(fileOf("blank.txt", "text/plain", 0))).toBe("blank.txt is empty");
  });
});

describe("ACCEPTED_FILES", () => {
  it("lets the file picker offer images, text, code and pdf", () => {
    const accepted = ACCEPTED_FILES.split(",");
    expect(accepted).toEqual(expect.arrayContaining(["image/png", "text/*", "application/pdf", ".ts", ".rs"]));
  });
});
