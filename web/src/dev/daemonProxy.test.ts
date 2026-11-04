import { describe, expect, it } from "vitest";
import {
  ALLOWED_DEV_ORIGIN,
  DEFAULT_DAEMON_URL,
  daemonProxy,
  daemonUrl,
  isOwnLoopbackOrigin,
  vouchForOwnOrigin,
  type OutgoingRequest,
  type ProxiedRequest,
} from "./daemonProxy";

function outgoing(): OutgoingRequest & { readonly set: Record<string, string> } {
  const set: Record<string, string> = {};
  return {
    set,
    setHeader(name: string, value: string) {
      set[name] = value;
    },
  };
}

const request = (headers: ProxiedRequest["headers"]): ProxiedRequest => ({ headers });

describe("daemonUrl", () => {
  it("stays on the default daemon without the variable", () => {
    expect(daemonUrl({})).toBe(DEFAULT_DAEMON_URL);
    expect(daemonUrl({ TOWNHALL_DAEMON_URL: "  " })).toBe(DEFAULT_DAEMON_URL);
  });

  it("takes the origin of TOWNHALL_DAEMON_URL", () => {
    expect(daemonUrl({ TOWNHALL_DAEMON_URL: "http://127.0.0.1:7782" })).toBe("http://127.0.0.1:7782");
    expect(daemonUrl({ TOWNHALL_DAEMON_URL: " http://127.0.0.1:7782/ " })).toBe("http://127.0.0.1:7782");
  });

  it("refuses a value that is not an http url", () => {
    expect(() => daemonUrl({ TOWNHALL_DAEMON_URL: "7782" })).toThrow(/TOWNHALL_DAEMON_URL/);
    expect(() => daemonUrl({ TOWNHALL_DAEMON_URL: "ftp://127.0.0.1:7782" })).toThrow(/http url/);
  });
});

describe("isOwnLoopbackOrigin", () => {
  it("accepts a page served by this loopback dev server", () => {
    expect(isOwnLoopbackOrigin("http://localhost:5182", "localhost:5182")).toBe(true);
    expect(isOwnLoopbackOrigin("http://127.0.0.1:5182", "127.0.0.1:5182")).toBe(true);
    expect(isOwnLoopbackOrigin("http://[::1]:5182", "[::1]:5182")).toBe(true);
  });

  it("refuses other pages and other host names", () => {
    expect(isOwnLoopbackOrigin("http://localhost:3000", "localhost:5182")).toBe(false);
    expect(isOwnLoopbackOrigin("http://evil.example:5182", "evil.example:5182")).toBe(false);
    expect(isOwnLoopbackOrigin("https://localhost:5182", "localhost:5182")).toBe(false);
    expect(isOwnLoopbackOrigin("http://localhost:5182", "bad host")).toBe(false);
  });
});

describe("vouchForOwnOrigin", () => {
  it("sends the dev origin the daemon allows for this server's own pages", () => {
    const out = outgoing();
    vouchForOwnOrigin(out, request({ origin: "http://localhost:5182", host: "localhost:5182" }));
    expect(out.set).toEqual({ origin: ALLOWED_DEV_ORIGIN });
  });

  it("leaves foreign and missing origins for the daemon to judge", () => {
    const cases: ProxiedRequest["headers"][] = [
      { origin: "http://localhost:3000", host: "localhost:5182" },
      { host: "localhost:5182" },
      { origin: "http://localhost:5182" },
      { origin: ["http://localhost:5182"], host: "localhost:5182" },
    ];
    for (const headers of cases) {
      const out = outgoing();
      vouchForOwnOrigin(out, request(headers));
      expect(out.set).toEqual({});
    }
  });
});

describe("daemonProxy", () => {
  it("proxies http and websockets to the daemon and vouches on both", () => {
    const options = daemonProxy({ TOWNHALL_DAEMON_URL: "http://127.0.0.1:7882" });
    expect(options).toMatchObject({ target: "http://127.0.0.1:7882", changeOrigin: true, ws: true });
    const listeners = new Map<string, (out: OutgoingRequest, req: ProxiedRequest) => void>();
    const proxy = { on: (event: string, listener: (out: OutgoingRequest, req: ProxiedRequest) => void) => listeners.set(event, listener) };
    options.configure?.(proxy as never, options);
    expect([...listeners.keys()]).toEqual(["proxyReq", "proxyReqWs"]);
    const out = outgoing();
    listeners.get("proxyReqWs")?.(out, request({ origin: "http://localhost:5282", host: "localhost:5282" }));
    expect(out.set).toEqual({ origin: ALLOWED_DEV_ORIGIN });
  });
});
