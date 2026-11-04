import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { attachmentFor, installFakeXhr, lastXhr, sentFile, xhrs } from "../attachments/testXhr";
import {
  attachmentUrl,
  listAttachments,
  MAX_ATTACHMENT_BYTES,
  parseAttachment,
  uploadAttachment,
} from "./attachments";
import { ApiError } from "./errors";

const shot = attachmentFor("a1", "shot.png");
const desktop = { __TAURI_INTERNALS__: {} };

function pngFile(name = "shot.png"): File {
  return new File([new Uint8Array([137, 80, 78, 71])], name, { type: "image/png" });
}

beforeEach(() => installFakeXhr());
afterEach(() => vi.unstubAllGlobals());

describe("attachmentUrl", () => {
  it("points at the same-origin api in a browser", () => {
    expect(attachmentUrl("c1", "a1", {})).toBe("/api/conversations/c1/attachments/a1");
  });

  it("uses the desktop base inside the desktop shell and encodes ids", () => {
    expect(attachmentUrl("c 1", "a/1", desktop)).toBe(
      "http://127.0.0.1:7777/api/conversations/c%201/attachments/a%2F1",
    );
  });

  it("defaults to the current window", () => {
    expect(attachmentUrl("c1", "a1")).toBe("/api/conversations/c1/attachments/a1");
  });
});

describe("parseAttachment", () => {
  it("accepts a well formed attachment", () => {
    expect(parseAttachment(shot)).toEqual(shot);
  });

  it.each([
    ["not an object", null],
    ["a missing id", { ...shot, id: 1 }],
    ["a missing name", { ...shot, name: undefined }],
    ["a bad kind", { ...shot, kind: "video" }],
    ["a bad size", { ...shot, size: "1" }],
    ["a missing mime", { ...shot, mime: null }],
    ["a missing conversation", { ...shot, conversationId: 3 }],
    ["a missing date", { ...shot, createdAt: undefined }],
  ])("rejects %s as a protocol error", (_what, value) => {
    expect(() => parseAttachment(value)).toThrow(ApiError);
  });
});

describe("listAttachments", () => {
  it("gets the conversation's attachments", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ success: true, data: [shot], error: null }), { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    expect(await listAttachments("c1")).toEqual([shot]);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/conversations/c1/attachments");
  });

  it("rejects a list that is not an array", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response(JSON.stringify({ success: true, data: {}, error: null }))),
    );
    await expect(listAttachments("c1")).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("uploadAttachment", () => {
  it("posts the file as multipart field file and resolves with the attachment", async () => {
    const file = pngFile();
    const pending = uploadAttachment("c1", file);
    const xhr = lastXhr();
    expect(xhr.method).toBe("POST");
    expect(xhr.url).toBe("/api/conversations/c1/attachments");
    expect(sentFile(xhr)?.name).toBe("shot.png");
    xhr.succeed(shot);
    await expect(pending).resolves.toEqual(shot);
  });

  it("reports progress as a whole percent", async () => {
    const onProgress = vi.fn();
    const pending = uploadAttachment("c1", pngFile(), onProgress);
    const xhr = lastXhr();
    xhr.progress(3, 5);
    xhr.progress(1, 3);
    xhr.progress(1, 0, false);
    xhr.succeed(shot);
    await pending;
    expect(onProgress.mock.calls).toEqual([[60], [33]]);
  });

  it("maps a failure envelope to an http error with the daemon's words", async () => {
    const pending = uploadAttachment("c1", pngFile());
    lastXhr().reject(413, "shot.png is over 20 MB");
    await expect(pending).rejects.toMatchObject({
      kind: "http",
      status: 413,
      message: "shot.png is over 20 MB",
    });
  });

  it("falls back to the status when the failure has no error text", async () => {
    const pending = uploadAttachment("c1", pngFile());
    lastXhr().respond(500, { success: false, data: null, error: null });
    await expect(pending).rejects.toMatchObject({ kind: "http", message: "HTTP 500" });
  });

  it("rejects a response that is not JSON", async () => {
    const pending = uploadAttachment("c1", pngFile());
    lastXhr().respond(502, "<html>bad gateway</html>");
    await expect(pending).rejects.toMatchObject({ kind: "protocol", status: 502 });
  });

  it("rejects an attachment of the wrong shape", async () => {
    const pending = uploadAttachment("c1", pngFile());
    lastXhr().succeed({ id: "a1" });
    await expect(pending).rejects.toMatchObject({ kind: "protocol" });
  });

  it("maps a dropped connection to a network error", async () => {
    const pending = uploadAttachment("c1", pngFile());
    lastXhr().fail();
    await expect(pending).rejects.toMatchObject({
      kind: "network",
      message: "cannot reach the daemon: the upload of shot.png stopped",
    });
  });

  it("aborts the request when the signal fires", async () => {
    const controller = new AbortController();
    const pending = uploadAttachment("c1", pngFile(), undefined, controller.signal);
    controller.abort();
    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(lastXhr().aborted).toBe(true);
  });

  it("never opens a request for a signal that already fired", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(uploadAttachment("c1", pngFile(), undefined, controller.signal)).rejects.toMatchObject({
      name: "AbortError",
    });
    expect(xhrs()).toHaveLength(0);
  });

  it("keeps the limit the daemon enforces", () => {
    expect(MAX_ATTACHMENT_BYTES).toBe(20 * 1024 * 1024);
  });
});
