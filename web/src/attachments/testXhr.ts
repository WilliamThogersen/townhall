import { vi } from "vitest";

type Listener = (() => void) | null;
type ProgressListener = ((event: Pick<ProgressEvent, "lengthComputable" | "loaded" | "total">) => void) | null;

let instances: readonly FakeXhr[] = [];

export class FakeXhr {
  method = "";
  url = "";
  body: unknown = null;
  status = 0;
  responseText = "";
  aborted = false;
  sent = false;
  onload: Listener = null;
  onerror: Listener = null;
  onabort: Listener = null;
  readonly upload: { onprogress: ProgressListener } = { onprogress: null };

  constructor() {
    instances = [...instances, this];
  }

  open(method: string, url: string): void {
    this.method = method;
    this.url = url;
  }

  send(body: unknown): void {
    this.body = body;
    this.sent = true;
  }

  abort(): void {
    this.aborted = true;
    this.onabort?.();
  }

  progress(loaded: number, total: number, lengthComputable = true): void {
    this.upload.onprogress?.({ lengthComputable, loaded, total });
  }

  respond(status: number, body: unknown): void {
    this.status = status;
    this.responseText = typeof body === "string" ? body : JSON.stringify(body);
    this.onload?.();
  }

  succeed(data: unknown): void {
    this.respond(201, { success: true, data, error: null });
  }

  reject(status: number, error: string): void {
    this.respond(status, { success: false, data: null, error });
  }

  fail(): void {
    this.onerror?.();
  }
}

export function installFakeXhr(): void {
  instances = [];
  vi.stubGlobal("XMLHttpRequest", FakeXhr);
}

export function xhrs(): readonly FakeXhr[] {
  return instances;
}

export function lastXhr(): FakeXhr {
  const last = instances[instances.length - 1];
  if (last === undefined) throw new Error("no XMLHttpRequest was opened");
  return last;
}

export function sentFile(xhr: FakeXhr): File | null {
  if (!(xhr.body instanceof FormData)) return null;
  const value = xhr.body.get("file");
  return value instanceof File ? value : null;
}

export function attachmentFor(id: string, name: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    conversationId: "c1",
    name,
    mime: name.endsWith(".png") ? "image/png" : "text/plain",
    size: 1024,
    kind: name.endsWith(".png") ? "image" : "file",
    createdAt: "2026-11-04T10:42:05.000Z",
    ...overrides,
  };
}
