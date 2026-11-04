import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { copyImage, downloadFile } from "./imageActions";
import { stubObjectUrls, type FakeObjectUrls } from "./testObjectUrls";

const PNG_TYPE = "image/png";
const JPEG_TYPE = "image/jpeg";

function serve(type: string, status = 200) {
  const fetchMock = vi.fn().mockResolvedValue(new Response("bytes", { status, headers: { "content-type": type } }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

class FakeClipboardItem {
  readonly items: Record<string, Promise<Blob>>;
  constructor(items: Record<string, Promise<Blob>>) {
    this.items = items;
  }
}

function stubClipboard() {
  const write = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("ClipboardItem", FakeClipboardItem);
  Object.defineProperty(navigator, "clipboard", { value: { write }, configurable: true });
  return write;
}

let urls: FakeObjectUrls;

beforeEach(() => {
  urls = stubObjectUrls();
});

afterEach(() => {
  urls.restore();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
});

describe("copyImage", () => {
  it("writes a png straight to the clipboard", async () => {
    serve(PNG_TYPE);
    const write = stubClipboard();
    await copyImage("/api/conversations/c1/attachments/a1");
    const [[item]] = write.mock.calls as [[FakeClipboardItem[]]];
    const written = await item[0]?.items["image/png"];
    expect(written?.type).toBe("image/png");
  });

  it("turns another image type into a png first", async () => {
    serve(JPEG_TYPE);
    const write = stubClipboard();
    const converted = new Blob(["converted"], { type: "image/png" });
    vi.stubGlobal("createImageBitmap", vi.fn().mockResolvedValue({ width: 4, height: 3 }));
    const drawImage = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage } as never);
    vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((done) => done(converted));
    await copyImage("/a.jpg");
    const [[item]] = write.mock.calls as [[FakeClipboardItem[]]];
    expect(await item[0]?.items["image/png"]).toBe(converted);
    expect(drawImage).toHaveBeenCalled();
  });

  it("fails when the image cannot be turned into a png", async () => {
    serve(JPEG_TYPE);
    vi.stubGlobal("createImageBitmap", vi.fn().mockResolvedValue({ width: 4, height: 3 }));
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((done) => done(null));
    const write = stubClipboard();
    write.mockImplementation(async (items: FakeClipboardItem[]) => items[0]?.items["image/png"]);
    await expect(copyImage("/a.jpg")).rejects.toThrow("the image could not be turned into a png");
  });

  it("refuses when this window has no image clipboard", async () => {
    await expect(copyImage("/a.png")).rejects.toThrow("this window cannot copy images");
  });

  it("fails on a download error", async () => {
    serve(PNG_TYPE, 404);
    const write = stubClipboard();
    write.mockImplementation(async (items: FakeClipboardItem[]) => items[0]?.items["image/png"]);
    await expect(copyImage("/a.png")).rejects.toThrow("HTTP 404");
  });
});

describe("downloadFile", () => {
  it("saves the bytes under the attachment's name", async () => {
    const fetchMock = serve(PNG_TYPE);
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      expect(this.download).toBe("network.png");
      expect(this.getAttribute("href")).toBe("blob:blob");
      expect(this.isConnected).toBe(true);
    });
    vi.useFakeTimers();
    await downloadFile("/api/x", "network.png");
    expect(fetchMock).toHaveBeenCalledWith("/api/x");
    expect(click).toHaveBeenCalledOnce();
    expect(document.querySelector("a[download]")).toBeNull();
    vi.runAllTimers();
    expect(urls.revoke).toHaveBeenCalledWith("blob:blob");
    vi.useRealTimers();
  });

  it("fails on a download error", async () => {
    serve(PNG_TYPE, 500);
    await expect(downloadFile("/api/x", "network.png")).rejects.toThrow("HTTP 500");
  });
});
