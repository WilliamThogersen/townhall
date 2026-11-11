import { httpBase } from "./base";
import { ApiError, toMessage } from "./errors";
import { parseEnvelope, type Guard } from "./guards";
import { isRecord } from "./guardKit";
import { reportQuiet } from "../log/report";

async function readJson(res: Response): Promise<unknown> {
  try {
    return await res.json();
  } catch {
    return undefined;
  }
}

export class TomlBody {
  readonly text: string;

  constructor(text: string) {
    this.text = text;
  }
}

function encode(body: unknown): { readonly type: string | null; readonly payload: BodyInit | undefined } {
  if (body === undefined) return { type: null, payload: undefined };
  if (body instanceof FormData) return { type: null, payload: body };
  if (body instanceof TomlBody) return { type: "application/toml", payload: body.text };
  return { type: "application/json", payload: JSON.stringify(body) };
}

export interface Answer {
  readonly data: unknown;
  readonly warnings: readonly string[];
}

function warningsOf(json: unknown): readonly string[] {
  const warnings = isRecord(json) ? json.warnings : undefined;
  return Array.isArray(warnings) ? warnings.filter((w): w is string => typeof w === "string") : [];
}

export async function exchange(
  method: string,
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<Answer> {
  const requestId = crypto.randomUUID();
  const quiet = path.startsWith("/logs/client");
  const fail = (err: ApiError, event: string): never => {
    if (!quiet) {
      reportQuiet("error", event, `${method} ${path.split("?")[0]}: ${err.message}`, {
        request_id: requestId,
        error_kind: err.kind,
        ...(err.status === null ? {} : { status: err.status }),
      });
    }
    throw err;
  };
  const { type, payload } = encode(body);
  let res: Response;
  try {
    res = await fetch(`${httpBase()}${path}`, {
      method,
      signal,
      headers: {
        "x-request-id": requestId,
        ...(type === null ? {} : { "content-type": type }),
      },
      body: payload,
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    return fail(new ApiError("network", `cannot reach the daemon: ${toMessage(e)}`), "api.network");
  }
  const json = await readJson(res);
  if (json === undefined) {
    return fail(new ApiError("protocol", `HTTP ${res.status}: response is not JSON`, res.status), "api.protocol");
  }
  const env = parseEnvelope(json);
  if (!res.ok || !env.success) {
    const err = new ApiError("http", env.error ?? `HTTP ${res.status}`, res.status, env.data);
    if (res.status >= 500) return fail(err, "api.http");
    throw err;
  }
  return { data: env.data, warnings: warningsOf(json) };
}

export async function request<T>(
  method: string,
  path: string,
  guard: Guard<T>,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  return guard((await exchange(method, path, body, signal)).data);
}
