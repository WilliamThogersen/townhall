import { httpBase } from "./base";
import { ApiError, toMessage } from "./errors";
import { parseEnvelope, type Guard } from "./guards";
import { reportQuiet } from "../log/report";

async function readJson(res: Response): Promise<unknown> {
  try {
    return await res.json();
  } catch {
    return undefined;
  }
}

export async function request<T>(
  method: string,
  path: string,
  guard: Guard<T>,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
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
  let res: Response;
  try {
    res = await fetch(`${httpBase()}${path}`, {
      method,
      signal,
      headers: {
        "x-request-id": requestId,
        ...(body === undefined || body instanceof FormData ? {} : { "content-type": "application/json" }),
      },
      body: body instanceof FormData || body === undefined ? body : JSON.stringify(body),
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
    const err = new ApiError("http", env.error ?? `HTTP ${res.status}`, res.status);
    if (res.status >= 500) return fail(err, "api.http");
    throw err;
  }
  return guard(env.data);
}
