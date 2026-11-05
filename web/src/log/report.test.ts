import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { request } from "../api/request";
import { ApiError } from "../api/errors";
import { logError, logWarn, installGlobalHandlers, resetReportsForTests } from "./report";

const guard = (v: unknown) => v as string;

function reportCalls(fetchFn: ReturnType<typeof vi.fn>) {
  return fetchFn.mock.calls.filter(([url]) => String(url).endsWith("/logs/client"));
}

function sentBatch(fetchFn: ReturnType<typeof vi.fn>): Record<string, unknown>[] {
  const calls = reportCalls(fetchFn);
  return calls.flatMap(([, init]) => JSON.parse((init as RequestInit).body as string));
}

beforeEach(() => {
  vi.useFakeTimers();
  resetReportsForTests();
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("report", () => {
  it("posts an unhandledrejection as a keepalive batch", async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(null, { status: 202 }));
    vi.stubGlobal("fetch", fetchFn);
    const remove = installGlobalHandlers(window);
    const ev = Object.assign(new Event("unhandledrejection"), { reason: new Error("boom") });
    window.dispatchEvent(ev);
    window.dispatchEvent(Object.assign(new Event("unhandledrejection"), { reason: "plain" }));
    expect(fetchFn).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1500);
    expect(reportCalls(fetchFn)).toHaveLength(1);
    const [, init] = reportCalls(fetchFn)[0] as [string, RequestInit];
    expect(init.method).toBe("POST");
    expect(init.keepalive).toBe(true);
    const batch = sentBatch(fetchFn);
    expect(batch).toHaveLength(2);
    expect(batch[0]).toMatchObject({ level: "error", event: "web.unhandled_rejection", message: "boom" });
    expect(batch[1]).toMatchObject({ event: "web.unhandled_rejection", message: "plain" });
    remove();
  });

  it("reports window errors as web.error with a truncated stack", async () => {
    const fetchFn = vi.fn().mockResolvedValue(new Response(null, { status: 202 }));
    vi.stubGlobal("fetch", fetchFn);
    const remove = installGlobalHandlers(window);
    const err = new Error("bad");
    err.stack = "x".repeat(10_000);
    window.dispatchEvent(Object.assign(new Event("error"), { message: "bad", error: err }));
    await vi.advanceTimersByTimeAsync(1500);
    const [entry] = sentBatch(fetchFn);
    expect(entry).toMatchObject({ level: "error", event: "web.error", message: "bad" });
    expect((entry.stack as string).length).toBeLessThanOrEqual(2000);
    remove();
  });

  it("a failing report neither throws nor recurses", async () => {
    const fetchFn = vi.fn().mockRejectedValue(new TypeError("offline"));
    vi.stubGlobal("fetch", fetchFn);
    const remove = installGlobalHandlers(window);
    expect(() => logError("web.x", "one")).not.toThrow();
    await vi.advanceTimersByTimeAsync(1500);
    await vi.advanceTimersByTimeAsync(5000);
    expect(reportCalls(fetchFn)).toHaveLength(1);
    // a synchronous throw from fetch is swallowed too
    vi.stubGlobal("fetch", () => {
      throw new Error("sync");
    });
    logWarn("web.y", "two");
    await expect(vi.advanceTimersByTimeAsync(1500)).resolves.not.toThrow();
    remove();
  });
});

describe("request reporting", () => {
  it("reports a 500 as api.http with status and the request id header", async () => {
    const fetchFn = vi.fn(async (url: string) =>
      String(url).endsWith("/logs/client")
        ? new Response(null, { status: 202 })
        : new Response(JSON.stringify({ success: false, data: null, error: "kaput" }), { status: 500 }),
    );
    vi.stubGlobal("fetch", fetchFn);
    await expect(request("GET", "/tasks", guard)).rejects.toBeInstanceOf(ApiError);
    const apiInit = fetchFn.mock.calls[0][1] as unknown as RequestInit;
    const id = (apiInit.headers as Record<string, string>)["x-request-id"];
    expect(id).toBeTruthy();
    await vi.advanceTimersByTimeAsync(1500);
    const batch = sentBatch(fetchFn as unknown as ReturnType<typeof vi.fn>);
    expect(batch).toHaveLength(1);
    expect(batch[0]).toMatchObject({ event: "api.http", status: 500, request_id: id, error_kind: "http" });
  });

  it("does not report 4xx, but reports network and protocol failures", async () => {
    const fetchFn = vi.fn();
    vi.stubGlobal("fetch", fetchFn);
    fetchFn.mockResolvedValueOnce(new Response(JSON.stringify({ success: false, data: null, error: "no" }), { status: 404 }));
    await expect(request("GET", "/a", guard)).rejects.toMatchObject({ kind: "http" });
    fetchFn.mockRejectedValueOnce(new TypeError("down"));
    await expect(request("GET", "/b", guard)).rejects.toMatchObject({ kind: "network" });
    fetchFn.mockResolvedValueOnce(new Response("<html>", { status: 200 }));
    await expect(request("GET", "/c", guard)).rejects.toMatchObject({ kind: "protocol" });
    fetchFn.mockResolvedValue(new Response(null, { status: 202 }));
    await vi.advanceTimersByTimeAsync(1500);
    expect(sentBatch(fetchFn).map((e) => e.event)).toEqual(["api.network", "api.protocol"]);
  });

  it("never reports a failure of the report call itself", async () => {
    const fetchFn = vi.fn().mockRejectedValue(new TypeError("down"));
    vi.stubGlobal("fetch", fetchFn);
    await expect(request("POST", "/logs/client", guard, [])).rejects.toMatchObject({ kind: "network" });
    await vi.advanceTimersByTimeAsync(5000);
    expect(fetchFn).toHaveBeenCalledTimes(1);
  });
});
