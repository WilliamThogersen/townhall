import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_ATTACHMENT_BYTES } from "../api/attachments";
import { stubObjectUrls, type FakeObjectUrls } from "./testObjectUrls";
import { attachmentFor, installFakeXhr, lastXhr, xhrs } from "./testXhr";
import { useUploads } from "./useUploads";

let urls: FakeObjectUrls;

function png(name = "shot.png", size = 10): File {
  const file = new File(["x"], name, { type: "image/png" });
  Object.defineProperty(file, "size", { value: size });
  return file;
}

const sql = () => new File(["select 1"], "schema.sql", { type: "" });

beforeEach(() => {
  installFakeXhr();
  urls = stubObjectUrls();
});

afterEach(() => {
  urls.restore();
  vi.unstubAllGlobals();
});

describe("useUploads", () => {
  it("does nothing without a conversation", () => {
    const { result } = renderHook(() => useUploads(null));
    act(() => result.current.add([png()]));
    expect(result.current.enabled).toBe(false);
    expect(result.current.items).toEqual([]);
    expect(xhrs()).toHaveLength(0);
  });

  it("uploads each file with progress and collects the finished ids", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png(), sql()]));
    expect(result.current.items.map((u) => u.status)).toEqual(["uploading", "uploading"]);
    expect(result.current.uploading).toBe(true);
    expect(result.current.items[0]?.previewUrl).toBe("blob:shot.png");
    expect(result.current.items[1]?.previewUrl).toBeNull();
    const [first, second] = xhrs();
    act(() => first?.progress(6, 10));
    expect(result.current.items[0]?.percent).toBe(60);
    await act(async () => first?.succeed(attachmentFor("a1", "shot.png")));
    await act(async () => second?.succeed(attachmentFor("a2", "schema.sql")));
    expect(result.current.uploading).toBe(false);
    expect(result.current.attachmentIds).toEqual(["a1", "a2"]);
  });

  it("refuses a file over 20 MB without sending it", () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png("screenshot-3.png", MAX_ATTACHMENT_BYTES + 1)]));
    expect(xhrs()).toHaveLength(0);
    expect(result.current.items[0]).toMatchObject({ status: "failed", retryable: false });
    expect(result.current.error).toBe("screenshot-3.png is over 20 MB");
  });

  it("marks a daemon refusal as final and a dropped connection as retryable", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png("a.png"), png("b.png")]));
    const [first, second] = xhrs();
    await act(async () => first?.reject(413, "a.png is over 20 MB"));
    await act(async () => second?.fail());
    expect(result.current.items.map((u) => u.retryable)).toEqual([false, true]);
    expect(result.current.error).toBe("cannot reach the daemon: the upload of b.png stopped");
  });

  it("retries a failed upload that can be retried", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png()]));
    await act(async () => lastXhr().fail());
    const key = result.current.items[0]?.key ?? "";
    act(() => result.current.retry(key));
    expect(xhrs()).toHaveLength(2);
    expect(result.current.items[0]).toMatchObject({ status: "uploading", error: null });
    await act(async () => lastXhr().succeed(attachmentFor("a1", "shot.png")));
    expect(result.current.attachmentIds).toEqual(["a1"]);
  });

  it("retries every retryable failure at once and leaves final ones", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png("a.png"), png("b.png"), png("big.png", MAX_ATTACHMENT_BYTES + 1)]));
    await act(async () => xhrs()[0]?.fail());
    await act(async () => xhrs()[1]?.fail());
    act(() => result.current.retryFailed());
    expect(xhrs()).toHaveLength(4);
    expect(result.current.items.map((u) => u.status)).toEqual(["uploading", "uploading", "failed"]);
  });

  it("does not retry a final failure", () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png("big.png", MAX_ATTACHMENT_BYTES + 1)]));
    act(() => result.current.retry(result.current.items[0]?.key ?? ""));
    expect(xhrs()).toHaveLength(0);
  });

  it("cancels a running upload when it is removed and frees its preview", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png()]));
    const xhr = lastXhr();
    act(() => result.current.remove(result.current.items[0]?.key ?? ""));
    expect(xhr.aborted).toBe(true);
    expect(result.current.items).toEqual([]);
    await waitFor(() => expect(urls.revoke).toHaveBeenCalledWith("blob:shot.png"));
  });

  it("clears everything after a send", async () => {
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png(), sql()]));
    await act(async () => xhrs()[0]?.succeed(attachmentFor("a1", "shot.png")));
    act(() => result.current.clear());
    expect(result.current.items).toEqual([]);
    expect(xhrs()[1]?.aborted).toBe(true);
  });

  it("starts over when the conversation changes", () => {
    const { result, rerender } = renderHook(({ id }) => useUploads(id), { initialProps: { id: "c1" } });
    act(() => result.current.add([png()]));
    rerender({ id: "c2" });
    expect(result.current.items).toEqual([]);
    expect(xhrs()[0]?.aborted).toBe(true);
    act(() => result.current.add([png()]));
    expect(lastXhr().url).toBe("/api/conversations/c2/attachments");
  });

  it("cancels running uploads and frees previews when unmounted", () => {
    const { result, unmount } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png()]));
    unmount();
    expect(lastXhr().aborted).toBe(true);
    expect(urls.revoke).toHaveBeenCalledWith("blob:shot.png");
  });

  it("works without object URLs", () => {
    urls.remove();
    const { result } = renderHook(() => useUploads("c1"));
    act(() => result.current.add([png()]));
    expect(result.current.items[0]?.previewUrl).toBeNull();
  });
});
