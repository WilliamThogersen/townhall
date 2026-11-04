import { randomUUID } from "node:crypto";

const CONFIG_FLAG = "--mcp-config=";

export function mcpEndpoint(args) {
  const raw = args.find((arg) => arg.startsWith(CONFIG_FLAG))?.slice(CONFIG_FLAG.length);
  if (!raw) return null;
  try {
    return JSON.parse(raw)?.mcpServers?.townhall?.url ?? null;
  } catch {
    return null;
  }
}

export async function callTool(endpoint, name, input) {
  if (!endpoint) return { ok: false, why: "no mcp endpoint" };
  const request = { jsonrpc: "2.0", id: randomUUID(), method: "tools/call", params: { name, arguments: input } };
  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(request),
    });
    const body = await response.json();
    const failed = !response.ok || body?.error || body?.result?.isError;
    return { ok: !failed, why: failed ? JSON.stringify(body?.error ?? body?.result?.content ?? body) : null };
  } catch (error) {
    return { ok: false, why: error.message };
  }
}
