import { afterEach, describe, expect, it, vi } from "vitest";
import { artifactUrl, listArtifacts, parseArtifact, parseArtifacts, readArtifactText } from "./artifacts";
import { ApiError } from "./errors";
import { anArtifact } from "./fixtures";

const desktop = { __TAURI_INTERNALS__: {} };

afterEach(() => vi.unstubAllGlobals());

function stubFetch(response: Response): ReturnType<typeof vi.fn> {
  const fetchMock = vi.fn().mockResolvedValue(response);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("artifactUrl", () => {
  it("points at the same-origin api in a browser", () => {
    expect(artifactUrl("fix-login", "r1", {})).toBe("/api/tasks/fix-login/artifacts/r1");
  });

  it("uses the desktop base and encodes the names", () => {
    expect(artifactUrl("a b", "r/1", desktop)).toBe("http://127.0.0.1:7777/api/tasks/a%20b/artifacts/r%2F1");
  });
});

describe("parseArtifact", () => {
  it("accepts what the daemon sends, with or without a caption", () => {
    expect(parseArtifact(anArtifact())).toEqual(anArtifact());
    const captioned = anArtifact({ caption: "the suite" });
    expect(parseArtifact(captioned)).toEqual(captioned);
  });

  it("refuses a kind or role it does not know", () => {
    expect(() => parseArtifact({ ...anArtifact(), kind: "video" })).toThrow("artifact.kind");
    expect(() => parseArtifact({ ...anArtifact(), authorRole: "boss" })).toThrow("artifact.authorRole");
    expect(() => parseArtifact({ ...anArtifact(), size: "big" })).toThrow("artifact.size");
    expect(() => parseArtifacts({})).toThrow("artifacts is not an array");
  });
});

describe("listArtifacts", () => {
  it("reads the list of one task from the envelope", async () => {
    const body = JSON.stringify({ success: true, data: [anArtifact()], error: null });
    const fetchMock = stubFetch(new Response(body, { status: 200 }));
    await expect(listArtifacts("fix-login")).resolves.toEqual([anArtifact()]);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/tasks/fix-login/artifacts");
  });
});

describe("readArtifactText", () => {
  it("returns the text of one artifact", async () => {
    const fetchMock = stubFetch(new Response("all green", { status: 200 }));
    await expect(readArtifactText("fix-login", "r1")).resolves.toBe("all green");
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/tasks/fix-login/artifacts/r1");
  });

  it("says why it could not", async () => {
    stubFetch(new Response("{}", { status: 404 }));
    await expect(readArtifactText("fix-login", "r1")).rejects.toThrow(ApiError);
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("offline")));
    await expect(readArtifactText("fix-login", "r1")).rejects.toThrow("cannot reach the daemon");
  });
});
