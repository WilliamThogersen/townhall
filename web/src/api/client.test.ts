import { afterEach, describe, expect, it, vi } from "vitest";
import { createAgent, getInfo, listAgents, sendText, streamUrl } from "./client";
import { ApiError } from "./errors";

const agent = { name: "a", kind: "shell", branch: "townhall/a", worktree: "/w/a", alive: true };
const agentInfo = { ...agent, repo: "shop", state: { kind: "waiting", question: "go?" } };

function mockFetch(status: number, body: unknown) {
  const fn = vi.fn().mockResolvedValue(
    new Response(typeof body === "string" ? body : JSON.stringify(body), { status }),
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => vi.unstubAllGlobals());

describe("api client", () => {
  it("unwraps the envelope and validates agents with their repo and state", async () => {
    mockFetch(200, { success: true, data: [agentInfo], error: null });
    expect(await listAgents()).toEqual([agentInfo]);
    mockFetch(200, { success: true, data: [agent], error: null });
    await expect(listAgents()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("validates the info payload and reads the user name", async () => {
    const info = { home: "/h/.townhall", repo: "/r", repoKey: "r-0123abcd", userName: "Ada Lovelace" };
    mockFetch(200, { success: true, data: info, error: null });
    expect(await getInfo()).toEqual(info);
    mockFetch(200, { success: true, data: { ...info, userName: null }, error: null });
    expect(await getInfo()).toEqual({ ...info, userName: null });
    mockFetch(200, { success: true, data: { ...info, userName: undefined }, error: null });
    expect(await getInfo()).toEqual({ ...info, userName: null });
    mockFetch(200, { success: true, data: { ...info, userName: 4 }, error: null });
    await expect(getInfo()).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, { success: true, data: { home: "/h" }, error: null });
    await expect(getInfo()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("maps a failure envelope to an http ApiError", async () => {
    mockFetch(404, { success: false, data: null, error: "agent not found" });
    const err = await sendText("x", "hi").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ kind: "http", status: 404, message: "agent not found" });
  });

  it("rejects malformed data as a protocol error", async () => {
    mockFetch(200, { success: true, data: [{ name: 1 }], error: null });
    await expect(listAgents()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("rejects non-JSON bodies", async () => {
    mockFetch(502, "<html>bad gateway</html>");
    await expect(listAgents()).rejects.toMatchObject({ kind: "protocol", status: 502 });
  });

  it("maps fetch failures to network errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("boom")));
    await expect(listAgents()).rejects.toMatchObject({ kind: "network" });
  });

  it("POSTs JSON bodies", async () => {
    const fn = mockFetch(201, { success: true, data: agent, error: null });
    await createAgent({ name: "a", adapter: "shell", command: "cat" });
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/agents");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({ name: "a", adapter: "shell", command: "cat" });
  });

  it("builds ws urls from the page location", () => {
    expect(streamUrl("a b", { protocol: "https:", host: "h:1" } as Location)).toBe(
      "wss://h:1/api/agents/a%20b/stream",
    );
  });
});
