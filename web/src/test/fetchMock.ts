import { vi, type Mock } from "vitest";

export type FetchMock = Mock<(url: string, init?: RequestInit) => Promise<Response>>;

export function mockFetch(status: number, data: unknown, error: string | null = null): FetchMock {
  const success = status >= 200 && status < 300;
  const fn: FetchMock = vi.fn(async () =>
    Response.json({ success, data: success ? data : null, error }, { status }),
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

export interface SentRequest {
  readonly url: string;
  readonly method: string;
  readonly body: unknown;
}

export function sent(fn: FetchMock, index = 0): SentRequest {
  const call = fn.mock.calls[index];
  if (!call) throw new Error(`no fetch call ${index}`);
  const [url, init] = call;
  const body = typeof init?.body === "string" ? (JSON.parse(init.body) as unknown) : undefined;
  return { url, method: init?.method ?? "GET", body };
}
