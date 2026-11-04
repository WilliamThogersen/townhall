const POLL_MS = 300;

async function call(base, method, path, body) {
  const init = body === undefined
    ? { method }
    : { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) };
  const response = await fetch(`${base}/api${path}`, init);
  const envelope = await response.json().catch(() => null);
  if (!response.ok || !envelope?.success) {
    throw new Error(`${method} /api${path} answered ${response.status}: ${envelope?.error ?? "no envelope"}`);
  }
  return envelope.data;
}

async function upload(base, conversationId, file) {
  const form = new FormData();
  form.append("file", new Blob([file.bytes], { type: file.mime }), file.name);
  const response = await fetch(`${base}/api/conversations/${conversationId}/attachments`, { method: "POST", body: form });
  const envelope = await response.json().catch(() => null);
  if (!response.ok || !envelope?.success) {
    throw new Error(`upload of ${file.name} answered ${response.status}: ${envelope?.error ?? "no envelope"}`);
  }
  return envelope.data;
}

export function daemonApi(base) {
  return {
    get: (path) => call(base, "GET", path),
    post: (path, body) => call(base, "POST", path, body),
    put: (path, body) => call(base, "PUT", path, body),
    patch: (path, body) => call(base, "PATCH", path, body),
    upload: (conversationId, file) => upload(base, conversationId, file),
  };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function waitFor(label, check, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const found = await check().catch(() => null);
    if (found) return found;
    await sleep(POLL_MS);
  }
  throw new Error(`timed out after ${Math.round(timeoutMs / 1000)} s waiting for ${label}`);
}
