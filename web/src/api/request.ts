import { httpBase } from "./base";
import { ApiError, toMessage } from "./errors";
import { parseEnvelope, type Guard } from "./guards";

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
  let res: Response;
  try {
    res = await fetch(`${httpBase()}${path}`, {
      method,
      signal,
      headers: body === undefined || body instanceof FormData ? undefined : { "content-type": "application/json" },
      body: body instanceof FormData || body === undefined ? body : JSON.stringify(body),
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new ApiError("network", `cannot reach the daemon: ${toMessage(e)}`);
  }
  const json = await readJson(res);
  if (json === undefined) {
    throw new ApiError("protocol", `HTTP ${res.status}: response is not JSON`, res.status);
  }
  const env = parseEnvelope(json);
  if (!res.ok || !env.success) {
    throw new ApiError("http", env.error ?? `HTTP ${res.status}`, res.status);
  }
  return guard(env.data);
}
