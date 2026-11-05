import { describe, expect, it } from "vitest";
import type { Attachment } from "../api/types";
import { attachmentFor } from "./testXhr";
import {
  doneAttachmentIds,
  isUploading,
  latestError,
  newUpload,
  uploadsReducer,
  type PendingUpload,
} from "./uploads";

const png = new File(["x"], "shot.png", { type: "image/png" });
const sql = new File(["x"], "schema.sql", { type: "" });
const shot = attachmentFor("a1", "shot.png") as Attachment;

function started(): readonly PendingUpload[] {
  return uploadsReducer([], {
    type: "added",
    uploads: [newUpload("u1", png, "blob:1", null), newUpload("u2", sql, null, null)],
  });
}

describe("newUpload", () => {
  it("starts an accepted file uploading at 0%", () => {
    expect(newUpload("u1", png, "blob:1", null)).toMatchObject({
      key: "u1",
      file: png,
      isImage: true,
      previewUrl: "blob:1",
      status: "uploading",
      percent: 0,
      attachment: null,
      error: null,
    });
  });

  it("fails a refused file for good", () => {
    expect(newUpload("u2", sql, null, "schema.sql is over 20 MB")).toMatchObject({
      isImage: false,
      status: "failed",
      error: "schema.sql is over 20 MB",
      retryable: false,
    });
  });
});

describe("uploadsReducer", () => {
  it("appends added uploads without touching the old list", () => {
    const before = started();
    const after = uploadsReducer(before, { type: "added", uploads: [newUpload("u3", png, null, null)] });
    expect(after.map((u) => u.key)).toEqual(["u1", "u2", "u3"]);
    expect(before).toHaveLength(2);
  });

  it("moves the percent of an upload that is still running", () => {
    const state = uploadsReducer(started(), { type: "progressed", key: "u1", percent: 60 });
    expect(state[0]?.percent).toBe(60);
    expect(state[1]?.percent).toBe(0);
  });

  it("finishes an upload with its attachment", () => {
    const state = uploadsReducer(started(), { type: "finished", key: "u1", attachment: shot });
    expect(state[0]).toMatchObject({ status: "done", percent: 100, attachment: shot });
  });

  it("ignores progress for an upload that is not running", () => {
    const done = uploadsReducer(started(), { type: "finished", key: "u1", attachment: shot });
    expect(uploadsReducer(done, { type: "progressed", key: "u1", percent: 10 })).toBe(done);
  });

  it("fails an upload with a reason and restarts it", () => {
    const failed = uploadsReducer(started(), {
      type: "failed",
      key: "u2",
      error: "cannot reach the daemon",
      retryable: true,
    });
    expect(failed[1]).toMatchObject({ status: "failed", error: "cannot reach the daemon", retryable: true });
    const restarted = uploadsReducer(failed, { type: "restarted", key: "u2" });
    expect(restarted[1]).toMatchObject({ status: "uploading", percent: 0, error: null });
  });

  it("removes one upload or clears them all", () => {
    expect(uploadsReducer(started(), { type: "removed", key: "u1" }).map((u) => u.key)).toEqual(["u2"]);
    expect(uploadsReducer(started(), { type: "cleared" })).toEqual([]);
  });

  it("keeps the same list when the key is unknown", () => {
    const state = started();
    expect(uploadsReducer(state, { type: "removed", key: "nope" })).toBe(state);
  });
});

describe("selectors", () => {
  it("knows when any upload is still running", () => {
    expect(isUploading(started())).toBe(true);
    const done = uploadsReducer(started(), { type: "removed", key: "u2" });
    expect(isUploading(uploadsReducer(done, { type: "finished", key: "u1", attachment: shot }))).toBe(false);
  });

  it("lists the ids of finished uploads only", () => {
    const state = uploadsReducer(started(), { type: "finished", key: "u1", attachment: shot });
    expect(doneAttachmentIds(state)).toEqual(["a1"]);
  });

  it("names the most recent failure", () => {
    const first = uploadsReducer(started(), { type: "failed", key: "u1", error: "one", retryable: true });
    const both = uploadsReducer(first, { type: "failed", key: "u2", error: "two", retryable: false });
    expect(latestError(both)).toBe("two");
    expect(latestError(started())).toBeNull();
  });
});
