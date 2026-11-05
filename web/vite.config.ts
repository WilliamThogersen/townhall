/// <reference types="vitest/config" />
import { defineConfig, loadEnv, type ProxyOptions } from "vite";
import react from "@vitejs/plugin-react";

export const DAEMON_URL_ENV = "TOWNHALL_DAEMON_URL";
export const DEFAULT_DAEMON_URL = "http://127.0.0.1:7777";
export const ALLOWED_DEV_ORIGIN = "http://localhost:5173";

const LOOPBACK_HOSTNAMES = new Set(["localhost", "127.0.0.1", "[::1]"]);
const HTTP_PROTOCOLS = new Set(["http:", "https:"]);

type Env = Readonly<Record<string, string | undefined>>;

export interface ProxiedRequest {
  readonly headers: Readonly<Record<string, string | string[] | undefined>>;
}

export interface OutgoingRequest {
  setHeader(name: string, value: string): unknown;
}

type ProxyListener = (out: OutgoingRequest, req: ProxiedRequest) => void;

interface ProxyEvents {
  on(event: "proxyReq" | "proxyReqWs", listener: ProxyListener): unknown;
}

function parseUrl(text: string): URL | null {
  try {
    return new URL(text);
  } catch {
    return null;
  }
}

export function daemonUrl(env: Env): string {
  const raw = env[DAEMON_URL_ENV]?.trim();
  if (!raw) return DEFAULT_DAEMON_URL;
  const url = parseUrl(raw);
  if (url === null || !HTTP_PROTOCOLS.has(url.protocol)) {
    throw new Error(`${DAEMON_URL_ENV} must be an http url, got ${raw}`);
  }
  return url.origin;
}

export function isOwnLoopbackOrigin(origin: string, host: string): boolean {
  const own = parseUrl(`http://${host}`);
  return own !== null && LOOPBACK_HOSTNAMES.has(own.hostname) && origin === own.origin;
}

export function vouchForOwnOrigin(out: OutgoingRequest, req: ProxiedRequest): void {
  const { origin, host } = req.headers;
  if (typeof origin === "string" && typeof host === "string" && isOwnLoopbackOrigin(origin, host)) {
    out.setHeader("origin", ALLOWED_DEV_ORIGIN);
  }
}

export function daemonProxy(env: Env): ProxyOptions {
  return {
    target: daemonUrl(env),
    changeOrigin: true,
    ws: true,
    configure: (proxy) => {
      const events = proxy as unknown as ProxyEvents;
      events.on("proxyReq", vouchForOwnOrigin);
      events.on("proxyReqWs", vouchForOwnOrigin);
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": daemonProxy(loadEnv(mode, ".", "TOWNHALL_")),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: { include: [/\.css\?raw$/] },
    coverage: {
      provider: "v8",
      reporter: ["text"],
    },
  },
}));
